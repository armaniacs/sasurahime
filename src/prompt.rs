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

impl CleanPrompt {
    /// [`TtyPrompt`] when a terminal is attached, [`NonInteractive`]
    /// otherwise. Both adapters are stateless unit structs, so the resolved
    /// port is a `&'static` trait object.
    pub fn for_current_run() -> &'static dyn PromptPort {
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
    fn cleaner_prompt_call_sites_pass_a_live_reporter_not_a_suppressor() {
        // PBI 64: the port owns the suspension, but the suspension only
        // happens when the caller passes the LIVE reporter. Six call sites
        // used to hardcode &DeepSuppressReporter, whose suspend_live_output
        // is the no-op default, so the dialoguer redraw bug could still
        // reproduce at those cleaners. The pin: no cleaner ever passes a
        // literal reference to the port - every call site threads the live
        // reporter down from clean().
        let sources = [
            include_str!("cleaners/library_caches.rs"),
            include_str!("cleaners/library_logs.rs"),
            include_str!("cleaners/app_support.rs"),
            include_str!("cleaners/ios_backup.rs"),
            include_str!("cleaners/apfs_snapshot.rs"),
            include_str!("cleaners/generic.rs"),
            include_str!("cleaners/ollama.rs"),
            include_str!("cleaners/orphan_containers.rs"),
        ];
        for src in sources {
            // Whitespace-normalized so a wrapped argument list still matches.
            let flat = src.split_whitespace().collect::<Vec<_>>().join(" ");
            assert!(
                !flat.contains("multi_select (&") && !flat.contains("confirm (&"),
                "a prompt call site passes a literal reference (the suppressor) \
                 instead of the live reporter:\n{}",
                &flat[..flat.len().min(200)]
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
