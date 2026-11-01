//! One prompt port for every cleaner (PBI 55, PBI 75).
//!
//! Before this module the interactive prompts were embedded in the cleaner
//! layer: dialoguer was called directly in 8 cleaner files, the TTY check
//! was hand-rolled at 9 sites with drifted wording, the live-output
//! suspension ran at only 2 of the prompt sites, and the TUI's
//! "already asked" suppression was a `SKIP_CONFIRM` global read at exactly
//! one site. This module owns all four contracts in one place: which
//! adapter a run gets (the TTY resolution), the TTY gating, the
//! live-output suspension, and the dialoguer calls.

use crate::progress::ProgressReporter;
use anyhow::{bail, Result};
use std::cell::Cell;
use std::io::{IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};

/// Set once per run by the entry point: the TUI suppresses the per-cleaner
/// confirm prompts because it already asked for confirmation before its
/// cleaning loop. Selection prompts are NOT affected (a selection is a
/// per-cleaner decision, a confirm is a per-run decision).
static CONFIRM_SUPPRESSED: AtomicBool = AtomicBool::new(false);

/// Suppress the per-cleaner confirm prompts. Call before the TUI cleaning
/// loop and restore with `set_confirm_suppressed(false)` after.
pub fn set_confirm_suppressed(suppressed: bool) {
    CONFIRM_SUPPRESSED.store(suppressed, Ordering::Relaxed);
}

/// True when the per-cleaner confirm prompts are suppressed (the TUI already
/// asked).
pub fn confirm_suppressed() -> bool {
    CONFIRM_SUPPRESSED.load(Ordering::Relaxed)
}

/// True when interactive prompts can run (a terminal is attached).
pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal()
}

/// The standard non-interactive remedy line. One owner of the wording: the
/// hand-rolled "not a terminal" guards this replaced had drifted (PBI 55),
/// and the [`NonInteractive`] port and the colima fallback refusal (PBI 69)
/// print the same line. Writer-parameterized so the wording is unit-testable.
pub fn write_not_a_terminal_remedy(out: &mut dyn Write, name: &str) -> std::io::Result<()> {
    writeln!(
        out,
        "[{name}] not a terminal - skipping. \
               Run `sasurahime clean {name}` interactively."
    )
}

/// The prompt port resolved for the current run (PBI 75): reads the
/// terminal and hands back the adapter that matches it. TTY-ness is a
/// per-process constant for a CLI run, so the resolution is ambient —
/// callers need no `CleanOptions` threading and no `clean()` signature
/// change, and the old "call `gate_interactive` first" ordering constraint
/// lives in the port's implementation instead of a doc comment.
pub struct CleanPrompt;

// The test-only port override (PBI 102): installed per thread by
// [`CleanPrompt::with_port`]. Production code never installs one, so every
// run resolves the ambient TTY port in [`CleanPrompt::for_current_run`].
thread_local! {
    static TEST_PORT_OVERRIDE: Cell<Option<&'static dyn PromptPort>> = const { Cell::new(None) };
}

impl CleanPrompt {
    /// [`TtyPrompt`] when a terminal is attached, [`NonInteractive`]
    /// otherwise. Both adapters are stateless unit structs, so the resolved
    /// port is a `&'static` trait object. A test that installed an override
    /// through [`CleanPrompt::with_port`] resolves the injected port
    /// instead; production never installs one, so the ambient TTY
    /// resolution below is the only resolution a run sees.
    pub fn for_current_run() -> &'static dyn PromptPort {
        // The test-only seam (PBI 102) is consulted first. With no override
        // installed — every production run — this is a thread-local read
        // and the ambient TTY resolution below is exactly what it always
        // was: one `is_interactive()` read per resolution.
        if let Some(port) = TEST_PORT_OVERRIDE.with(Cell::get) {
            return port;
        }
        if is_interactive() {
            &TtyPrompt
        } else {
            &NonInteractive
        }
    }

    /// The third confirm semantics (PBI 81): ask the user to proceed ONLY
    /// when a TTY is attached and the TUI has not suppressed the
    /// per-cleaner confirms. `Ok(None)` = the confirm could not be shown
    /// (no TTY, or the TUI already asked): the caller proceeds silently,
    /// and the destructive action's floor (the PBI 69 refusal gate)
    /// refuses at the deletion site instead. This is colima's semantics —
    /// `for_current_run().confirm()` cannot express it, because
    /// [`NonInteractive::confirm`] bails, which would break a non-TTY
    /// `clean colima`. The TTY resolution lives here: the cleaner names
    /// the port, never the adapter.
    ///
    /// `explanation` prints as normal output above the Confirm prompt, and
    /// only when the confirm is actually shown — a run that cannot be
    /// asked stays silent. The Confirm prompt itself stays the one short
    /// "Proceed?" line: dialoguer clears one line deep, so a wrapped
    /// prompt would double the message (the confirm-duplication fix is
    /// owned here, not per call site).
    pub fn confirm_on_tty(
        reporter: &dyn ProgressReporter,
        explanation: &str,
    ) -> Result<Option<bool>> {
        if confirm_suppressed() || !is_interactive() {
            return Ok(None);
        }
        println!("\n{explanation}");
        Ok(Some(TtyPrompt.confirm(reporter, "Proceed?")?))
    }

    /// The test-only injection seam for the select port (PBI 102): install
    /// `port` as the resolution on the CALLING THREAD and return a guard
    /// that restores the ambient TTY resolution on drop. Production code
    /// never installs an override, so the ambient resolution ([`Self::
    /// for_current_run`], the TTY-ness read per resolution) is unchanged;
    /// this seam exists so tests can drive the select paths (the
    /// `multi_select_or_skip` cleaners) with a mock port that speaks the
    /// port contract only — `Vec<usize>` picks and bools, never a dialoguer
    /// type. The override is per-thread, so parallel tests that install
    /// their own mock cannot see each other's port.
    #[doc(hidden)]
    pub fn with_port(port: &'static dyn PromptPort) -> PortOverride {
        let prev = TEST_PORT_OVERRIDE.with(|slot| slot.replace(Some(port)));
        PortOverride { prev }
    }
}

/// The guard returned by [`CleanPrompt::with_port`] (PBI 102): restores the
/// thread's previous resolution on drop, so a mock cannot leak past its
/// scope into the ambient TTY resolution.
#[doc(hidden)]
pub struct PortOverride {
    prev: Option<&'static dyn PromptPort>,
}

impl Drop for PortOverride {
    fn drop(&mut self) {
        TEST_PORT_OVERRIDE.with(|slot| slot.set(self.prev));
    }
}

/// How a cleaner asks the user questions. The port owns the TTY resolution,
/// the TTY gating, the live-output suspension, and the dialoguer calls so
/// the per-cleaner call sites cannot diverge on those contracts (the
/// dialoguer-redraw bug the confirm prompts hit is fixed once here, not per
/// call site).
pub trait PromptPort {
    /// Asks a yes/no question. TTY-only: errors when prompts are
    /// unavailable. A non-TTY run never reaches a confirm — the interactive
    /// cleaners guard with `is_interactive()` before asking — so the error
    /// is a defensive contract, not a path a user can hit.
    fn confirm(&self, reporter: &dyn ProgressReporter, prompt: &str) -> Result<bool>;

    /// Asks a multi-select question. `name` is the CLI target name the
    /// standard remedy names when prompts are unavailable.
    ///
    /// Returns `None` when prompts are unavailable (not a terminal): the
    /// port prints the standard remedy line exactly once and the caller
    /// skips its interactive path. An empty selection is `Some(vec![])`, so
    /// "not a terminal" and "nothing selected" can never print together.
    /// `defaults` pre-checks items; an empty slice leaves every item
    /// unchecked (dialoguer's behavior when `defaults` is never set).
    fn multi_select(
        &self,
        name: &str,
        reporter: &dyn ProgressReporter,
        prompt: Option<&str>,
        items: &[String],
        defaults: &[bool],
    ) -> Result<Option<Vec<usize>>>;
}

/// The real port: runs dialoguer under the reporter's live-output
/// suspension, so a live progress line never redraws under a prompt.
pub struct TtyPrompt;

impl PromptPort for TtyPrompt {
    fn confirm(&self, reporter: &dyn ProgressReporter, prompt: &str) -> Result<bool> {
        let mut answer = None;
        let mut run = || {
            answer = Some(
                dialoguer::Confirm::new()
                    .with_prompt(prompt)
                    .default(false)
                    .interact(),
            )
        };
        reporter.suspend_live_output(&mut run);
        match answer {
            Some(answer) => Ok(answer?),
            None => bail!("the confirmation prompt did not run"),
        }
    }

    fn multi_select(
        &self,
        _name: &str,
        reporter: &dyn ProgressReporter,
        prompt: Option<&str>,
        items: &[String],
        defaults: &[bool],
    ) -> Result<Option<Vec<usize>>> {
        let mut picks = None;
        let mut run = || {
            let mut select = dialoguer::MultiSelect::new();
            if let Some(prompt) = prompt {
                select = select.with_prompt(prompt);
            }
            picks = Some(select.items(items).defaults(defaults).interact());
        };
        reporter.suspend_live_output(&mut run);
        match picks {
            Some(picks) => Ok(Some(picks?)),
            None => bail!("the selection prompt did not run"),
        }
    }
}

/// The port for a non-interactive run: prompts are unavailable. Resolution
/// goes through [`CleanPrompt::for_current_run`], so a cleaner that reaches
/// this port skips its interactive path without a hand-rolled gate.
pub struct NonInteractive;

impl PromptPort for NonInteractive {
    fn confirm(&self, _reporter: &dyn ProgressReporter, _prompt: &str) -> Result<bool> {
        bail!("interactive prompts are unavailable (not a terminal)")
    }

    fn multi_select(
        &self,
        name: &str,
        _reporter: &dyn ProgressReporter,
        _prompt: Option<&str>,
        _items: &[String],
        _defaults: &[bool],
    ) -> Result<Option<Vec<usize>>> {
        // The remedy is the ONLY line this port prints: unavailable is not
        // an empty selection, so "nothing selected" never follows it.
        let _ = write_not_a_terminal_remedy(&mut std::io::stderr(), name);
        Ok(None)
    }
}

/// The one interactive-select ritual (PBI 99): a multi-select plus the two
/// skips every interactive cleaner used to repeat by hand in three drifted
/// shapes. The port owns the non-TTY resolution (PBI 75): when prompts are
/// unavailable, the port prints the standard remedy line exactly once and
/// hands back `None` — the caller skips its interactive path and never
/// interprets the `None`. An empty selection is the other skip: the
/// `[{name}] nothing selected` line prints here, exactly once, and the
/// caller skips. The two wordings live on disjoint paths, so "not a
/// terminal" and "nothing selected" can never print together, and the
/// cleaner-side duplicate outputs are gone. `Some` carries the picked
/// indices and the clean proceeds.
pub fn multi_select_or_skip(
    name: &str,
    reporter: &dyn ProgressReporter,
    prompt: Option<&str>,
    items: &[String],
    defaults: &[bool],
) -> Result<Option<Vec<usize>>> {
    // dialoguer pairs items and defaults by index, so a drifted pair would
    // silently pre-check the wrong items. Debug-only: the release build
    // compiles this out (every call site pairs them by construction).
    debug_assert_eq!(
        items.len(),
        defaults.len(),
        "multi_select items and defaults must pair up"
    );
    interpret_port_picks(
        name,
        CleanPrompt::for_current_run().multi_select(name, reporter, prompt, items, defaults)?,
        &mut std::io::stdout(),
    )
}

/// The pick interpretation of the one interactive-select ritual (PBI 99),
/// extracted from [`multi_select_or_skip`] so the three exits are
/// unit-testable without a TTY: dialoguer needs a real terminal, so the TTY
/// interaction itself is out of scope here - this seam sits after the port
/// call. `None` is the non-TTY remedy exit (the remedy is already printed by
/// the port) and stays silent; an empty selection prints the one `[{name}]`
/// line exactly once and skips; real picks carry through unchanged. The
/// stdout line goes through the `out` parameter, so its one-line shape is
/// pinned by value (the aggregation source pin greps the cleaners'
/// production regions for the literal, never this print).
fn interpret_port_picks(
    name: &str,
    picks: Option<Vec<usize>>,
    out: &mut dyn Write,
) -> Result<Option<Vec<usize>>> {
    match picks {
        // PBI 75: the port owns the non-TTY semantics — None means the
        // remedy is already printed and the caller just skips.
        None => Ok(None),
        Some(picks) if picks.is_empty() => {
            writeln!(out, "[{name}] nothing selected")?;
            Ok(None)
        }
        Some(picks) => Ok(Some(picks)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::ProgressReporter;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    /// A reporter that records how many times the live output was suspended.
    struct RecordingReporter {
        suspends: AtomicUsize,
    }
    impl RecordingReporter {
        fn new() -> Self {
            Self {
                suspends: AtomicUsize::new(0),
            }
        }
    }
    impl ProgressReporter for RecordingReporter {
        fn show_spinner(&self) -> bool {
            false
        }
        fn progress_init(&self, _msg: &str, _total: usize) {}
        fn progress_tick(&self, _path: &std::path::Path, _n: usize, _size: u64) {}
        fn progress_finish(&self) {}
        fn suspend_live_output(&self, f: &mut dyn FnMut()) {
            self.suspends.fetch_add(1, AtomicOrdering::Relaxed);
            f();
        }
    }

    #[test]
    fn tty_prompt_suspends_live_output_for_confirm() {
        // The port owns the suspension: every confirm call hides the live
        // progress line for the prompt, whichever cleaner asks (PBI 55).
        if is_interactive() {
            // dialoguer needs a real TTY; an interactive cargo test would
            // hang on the prompt.
            return;
        }
        let rep = RecordingReporter::new();
        let result = TtyPrompt.confirm(&rep, "Proceed?");
        assert!(result.is_err(), "a non-TTY interact() errors");
        assert_eq!(
            rep.suspends.load(AtomicOrdering::Relaxed),
            1,
            "the port must suspend the live output around the prompt"
        );
    }

    #[test]
    fn tty_prompt_suspends_live_output_for_multi_select() {
        if is_interactive() {
            return;
        }
        let rep = RecordingReporter::new();
        let items = vec!["a".to_string(), "b".to_string()];
        let defaults = vec![false, false];
        let result = TtyPrompt.multi_select("ollama", &rep, None, &items, &defaults);
        assert!(result.is_err(), "a non-TTY interact() errors");
        assert_eq!(
            rep.suspends.load(AtomicOrdering::Relaxed),
            1,
            "the port must suspend the live output around the prompt"
        );
    }

    #[test]
    fn multi_select_or_skip_skips_without_a_terminal() {
        // PBI 99: the one interactive-select ritual. The port owns the
        // non-TTY resolution (PBI 75): a non-TTY run gets Ok(None) without
        // hanging, the remedy is the only line the port prints, and nothing
        // is suspended (there is no dialoguer call to hide). The empty-
        // selection and picked-indices exits hand a real dialoguer call and
        // need a TTY, so they are pinned by the six cleaners' seam pins and
        // the port's own unit tests instead.
        if is_interactive() {
            // dialoguer needs a real TTY; an interactive cargo test would
            // hang on the prompt.
            return;
        }
        let rep = RecordingReporter::new();
        let items = vec!["a".to_string(), "b".to_string()];
        let picks = multi_select_or_skip("ollama", &rep, None, &items, &[false, false])
            .expect("unavailable is a None, not an error");
        assert!(picks.is_none(), "a non-TTY run must skip (Ok(None))");
        assert_eq!(
            rep.suspends.load(AtomicOrdering::Relaxed),
            0,
            "nothing to suspend: no dialoguer call happens"
        );
    }

    #[test]
    #[should_panic(expected = "must pair up")]
    fn multi_select_or_skip_panics_when_items_and_defaults_drift() {
        // #10: dialoguer pairs items and defaults by index, so a drifted
        // pair would silently pre-check the wrong items. The guard is a
        // debug_assert, so this pin only holds in the debug build that
        // cargo test uses (a release build compiles the assert out).
        let rep = RecordingReporter::new();
        let items = vec!["a".to_string(), "b".to_string()];
        let defaults = vec![false];
        let _ = multi_select_or_skip("ollama", &rep, None, &items, &defaults);
    }

    // --- PBI 102: the test injection seam for the select port ---

    /// A test port that answers a fixed selection and records that it was
    /// consulted. Stateless unit struct, same shape as the two adapters.
    struct MockSelectPort;
    static MOCK_CONSULTED: AtomicUsize = AtomicUsize::new(0);

    impl PromptPort for MockSelectPort {
        fn confirm(&self, _reporter: &dyn ProgressReporter, _prompt: &str) -> Result<bool> {
            bail!("the confirm path is not part of the select-port seam")
        }

        fn multi_select(
            &self,
            _name: &str,
            _reporter: &dyn ProgressReporter,
            _prompt: Option<&str>,
            _items: &[String],
            _defaults: &[bool],
        ) -> Result<Option<Vec<usize>>> {
            MOCK_CONSULTED.fetch_add(1, AtomicOrdering::Relaxed);
            Ok(Some(vec![0]))
        }
    }

    #[test]
    fn with_port_installs_the_mock_for_the_current_thread() {
        // PBI 102: the seam replaces the ambient TTY resolution. A test
        // process has no TTY, so the ambient resolution would always hand
        // back NonInteractive and the dialoguer select path would stay
        // untested; the injected port is what `for_current_run` resolves.
        let _port = CleanPrompt::with_port(&MockSelectPort);
        let rep = RecordingReporter::new();
        let items = vec!["a".to_string()];
        let picks = CleanPrompt::for_current_run()
            .multi_select("ollama", &rep, None, &items, &[false])
            .expect("the injected port answers, not the ambient remedy");
        assert_eq!(picks, Some(vec![0]), "the injected port must resolve");
        assert_eq!(
            MOCK_CONSULTED.load(AtomicOrdering::Relaxed),
            1,
            "the injected port must be the one consulted"
        );
    }

    #[test]
    fn with_port_restores_the_ambient_resolution_on_drop() {
        // The guard restores the ambient TTY resolution, so a mock cannot
        // leak past its scope: after the drop the resolution is ambient
        // again (NonInteractive on the non-TTY test stdin).
        {
            let _port = CleanPrompt::with_port(&MockSelectPort);
        }
        if is_interactive() {
            // dialoguer needs a real TTY; an interactive cargo test would
            // hang on the prompt.
            return;
        }
        let rep = RecordingReporter::new();
        let items = vec!["a".to_string()];
        let picks = CleanPrompt::for_current_run()
            .multi_select("ollama", &rep, None, &items, &[false])
            .expect("unavailable is a None, not an error");
        assert!(
            picks.is_none(),
            "the ambient TTY resolution must be back after the guard drops"
        );
    }

    #[test]
    fn interpret_port_picks_keeps_none_a_silent_skip() {
        // C3: the port's None is the non-TTY remedy exit - the remedy is
        // already printed by the port, so this branch adds no output and
        // stays a skip. The TTY interaction itself (dialoguer) is out of
        // scope here: this seam sits after the port call.
        let mut buf: Vec<u8> = Vec::new();
        let picks = interpret_port_picks("ollama", None, &mut buf).expect("None is not an error");
        assert!(picks.is_none(), "None must stay a skip (Ok(None))");
        assert!(buf.is_empty(), "the None exit must stay silent");
    }

    #[test]
    fn interpret_port_picks_treats_an_empty_selection_as_a_skip() {
        // C3: an empty selection is the other skip - the one `[{name}]`
        // line plus Ok(None). The writer parameter pins the one-line shape
        // by value (the aggregation source pin only greps the cleaners'
        // production regions for the literal, never this file's own print).
        let mut buf: Vec<u8> = Vec::new();
        let picks = interpret_port_picks("ollama", Some(Vec::new()), &mut buf)
            .expect("an empty selection is not an error");
        assert!(picks.is_none(), "an empty selection must skip (Ok(None))");
        assert_eq!(
            String::from_utf8(buf).unwrap(),
            "[ollama] nothing selected\n",
            "the empty-selection exit prints the one [{{name}}] line exactly once"
        );
    }

    #[test]
    fn interpret_port_picks_hands_picked_indices_through() {
        // C3: a real selection carries the picked indices unchanged so the
        // clean proceeds, with no extra output on that path.
        let mut buf: Vec<u8> = Vec::new();
        let picks = interpret_port_picks("ollama", Some(vec![0, 2]), &mut buf)
            .expect("a real selection is not an error");
        assert_eq!(picks, Some(vec![0, 2]), "picked indices must pass through");
        assert!(buf.is_empty(), "a real selection must add no output");
    }

    /// The whitespace-normalized production region of a scanned file:
    /// everything before its tests module. Test-only members marked
    /// `#[cfg(test)]` inside the production area (constructor helpers, a
    /// `cfg(test)` import) do not match the module marker, so the split
    /// lands on the real tests module and the suppressor literals that
    /// legitimately live under it are never scanned.
    fn production_region(src: &str) -> String {
        let flat = src.split_whitespace().collect::<Vec<_>>().join(" ");
        let tests_start = flat
            .find(concat!("#[cfg(test", ")] mod"))
            .unwrap_or(flat.len());
        flat[..tests_start].to_string()
    }

    #[test]
    fn cleaner_prompt_call_sites_pass_a_live_reporter_not_a_suppressor() {
        // Rebuilt around the type names (PBI 64): the old shape needle
        // `multi_select (&` fits neither the rustfmt-wrapped form
        // `multi_select(\n &X` nor the single-line form `multi_select(&X`,
        // so that assertion was always true and the suppression contract
        // went unpinned. clean() threads the LIVE reporter down from main()
        // and only tests may name the suppressor reporters, so every
        // scanned file's production region must not reference either
        // suppressor type - as a receiver, an argument, or a type. Type
        // names survive every rustfmt shape (rustfmt never wraps an
        // identifier), so the pin holds across argument reordering and
        // single-line/wrapped call forms. app_support.rs's production
        // `TtyPrompt` literal (the one direct confirm call, which passes the
        // caller's live reporter) is the legitimate exception and is NOT
        // pinned here: only suppressor references are forbidden.
        let sources = [
            (
                "cleaners/library_caches.rs",
                include_str!("cleaners/library_caches.rs"),
            ),
            (
                "cleaners/library_logs.rs",
                include_str!("cleaners/library_logs.rs"),
            ),
            (
                "cleaners/app_support.rs",
                include_str!("cleaners/app_support.rs"),
            ),
            (
                "cleaners/ios_backup.rs",
                include_str!("cleaners/ios_backup.rs"),
            ),
            (
                "cleaners/apfs_snapshot.rs",
                include_str!("cleaners/apfs_snapshot.rs"),
            ),
            ("cleaners/generic.rs", include_str!("cleaners/generic.rs")),
            ("cleaners/ollama.rs", include_str!("cleaners/ollama.rs")),
            (
                "cleaners/orphan_containers.rs",
                include_str!("cleaners/orphan_containers.rs"),
            ),
            ("prompt.rs", include_str!("prompt.rs")),
        ];
        // Assembled from pieces so this file's own source does not contain
        // the contiguous shapes it greps for. The bare name subsumes the
        // Deep-prefixed one; both are asserted so a failure names the exact
        // forbidden shapes.
        let deep_needle = concat!("Deep", "SuppressReporter");
        let bare_needle = concat!("Suppress", "Reporter");
        for (path, src) in sources {
            let production = production_region(src);
            assert!(
                !production.contains(deep_needle) && !production.contains(bare_needle),
                "{path} production code names a suppressor reporter \
                 ({deep_needle} / {bare_needle}); clean() threads the live \
                 reporter down and only tests may name the suppressors:\n{}",
                &production[..production.len().min(200)]
            );
        }
    }

    #[test]
    fn the_empty_selection_exit_and_select_routing_are_owned_by_the_shared_select_path() {
        // Consolidated form of the five migrated cleaners' seam pins
        // (PBI 99), both needles on each file's production region:
        // - the assembled empty-selection literal must NOT appear: the
        //   shared select path owns that wording, and a cleaner-side print
        //   would repeat it. prompt.rs is deliberately NOT scanned here -
        //   the shared path's own print lives in this file and would match
        //   its own needle;
        // - the assembled routing needle MUST appear: every migrated cleaner
        //   routes its selection through the shared ritual, which owns the
        //   non-TTY remedy and the empty-selection exit. The per-file dup
        //   pins' routing assertion is self-fulfilling (their own source
        //   contains the short needle), so this multi-source pin is the
        //   guard that actually detects a bypass.
        let migrated = [
            (
                "cleaners/library_caches.rs",
                include_str!("cleaners/library_caches.rs"),
            ),
            (
                "cleaners/library_logs.rs",
                include_str!("cleaners/library_logs.rs"),
            ),
            (
                "cleaners/app_support.rs",
                include_str!("cleaners/app_support.rs"),
            ),
            (
                "cleaners/ios_backup.rs",
                include_str!("cleaners/ios_backup.rs"),
            ),
            ("cleaners/ollama.rs", include_str!("cleaners/ollama.rs")),
        ];
        let print_needle = concat!("] nothing", " selected");
        let routing_needle = concat!("multi_select", "_or_skip");
        for (path, src) in migrated {
            let production = production_region(src);
            assert!(
                !production.contains(print_needle),
                "{path} prints the empty-selection line itself; the shared \
                 select path owns that wording exactly once:\n{}",
                &production[..production.len().min(200)]
            );
            assert!(
                production.contains(routing_needle),
                "{path} bypasses the shared select ritual ({routing_needle}); \
                 the non-TTY remedy and the empty-selection exit are owned \
                 there:\n{}",
                &production[..production.len().min(200)]
            );
        }
    }

    #[test]
    fn non_interactive_multi_select_is_unavailable_with_one_remedy_line() {
        // PBI 75: the port owns the non-TTY resolution. Unavailable is not
        // an empty selection: multi_select returns None (the caller skips),
        // the standard remedy is the only line the port prints, and nothing
        // is suspended (there is no dialoguer call to hide).
        let rep = RecordingReporter::new();
        let items = vec!["a".to_string(), "b".to_string()];
        let picks = NonInteractive
            .multi_select("ollama", &rep, None, &items, &[])
            .expect("unavailable is a None, not an error");
        assert!(
            picks.is_none(),
            "unavailable must be None, not an empty pick list"
        );
        assert_eq!(
            rep.suspends.load(AtomicOrdering::Relaxed),
            0,
            "nothing to suspend: no dialoguer call happens"
        );
        // The remedy wording and its one-line shape: the port writes exactly
        // this line once per unavailable multi_select.
        let mut buf: Vec<u8> = Vec::new();
        write_not_a_terminal_remedy(&mut buf, "ollama").unwrap();
        let line = String::from_utf8(buf).unwrap();
        assert!(
            line.starts_with("[ollama] not a terminal - skipping"),
            "the remedy must name the target:\n{line}"
        );
        assert_eq!(line.lines().count(), 1, "exactly one remedy line:\n{line}");
    }

    #[test]
    fn non_interactive_confirm_refuses() {
        // confirm stays TTY-only (PBI 75): a non-TTY run never reaches a
        // confirm (the interactive cleaners guard with is_interactive
        // first), so the port errors instead of answering.
        let rep = RecordingReporter::new();
        assert!(NonInteractive.confirm(&rep, "Proceed?").is_err());
        assert_eq!(rep.suspends.load(AtomicOrdering::Relaxed), 0);
    }

    #[test]
    fn confirm_suppression_round_trips() {
        set_confirm_suppressed(true);
        assert!(confirm_suppressed());
        set_confirm_suppressed(false);
        assert!(!confirm_suppressed());
    }

    #[test]
    fn confirm_on_tty_returns_none_on_a_non_tty() {
        // The third semantics (PBI 81): the confirm is asked ONLY when a
        // TTY is attached and the TUI has not suppressed it. Ok(None) = the
        // confirm could not be shown — the caller proceeds silently and
        // the destructive-action floor (the PBI 69 refusal gate) refuses
        // at the deletion site instead.
        if is_interactive() {
            // dialoguer needs a real TTY; an interactive cargo test would
            // hang on the prompt.
            return;
        }
        let rep = RecordingReporter::new();
        let answer =
            CleanPrompt::confirm_on_tty(&rep, "an explanation").expect("None is not an error");
        assert!(answer.is_none(), "a non-TTY run cannot show the confirm");
        assert_eq!(
            rep.suspends.load(AtomicOrdering::Relaxed),
            0,
            "nothing to suspend: no dialoguer call happens"
        );
    }

    #[test]
    fn confirm_on_tty_returns_none_while_the_tui_suppresses_confirms() {
        // The suppression predicate is owned by the port: a suppressed run
        // never reaches a dialoguer confirm, whichever cleaner asks. On a
        // TTY this exercises the suppression branch; on a non-TTY the
        // TTY branch answers None first and the assertion still holds.
        set_confirm_suppressed(true);
        let rep = RecordingReporter::new();
        let answer = CleanPrompt::confirm_on_tty(&rep, "an explanation").unwrap();
        set_confirm_suppressed(false);
        assert!(answer.is_none(), "a suppressed run cannot show the confirm");
        assert_eq!(
            rep.suspends.load(AtomicOrdering::Relaxed),
            0,
            "nothing to suspend: no dialoguer call happens"
        );
    }
}
