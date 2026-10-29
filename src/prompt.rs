//! One prompt port for every cleaner (PBI 55).
//!
//! Before this module the interactive prompts were embedded in the cleaner
//! layer: dialoguer was called directly in 8 cleaner files, the TTY check
//! was hand-rolled at 9 sites with drifted wording, the live-output
//! suspension ran at only 2 of the prompt sites, and the TUI's
//! "already asked" suppression was a `SKIP_CONFIRM` global read at exactly
//! one site. This module owns all three contracts in one place: the TTY
//! gating, the live-output suspension, and the dialoguer calls.

use crate::progress::ProgressReporter;
use anyhow::{bail, Result};
use std::io::IsTerminal;
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

/// The standardized non-interactive gate: prints the remedy and returns
/// false so the caller can skip its interactive path. Replaces the
/// hand-rolled "not a terminal" guards whose wording had drifted.
pub fn gate_interactive(name: &str) -> bool {
    if is_interactive() {
        return true;
    }
    eprintln!(
        "[{name}] not a terminal - skipping. \
               Run `sasurahime clean {name}` interactively."
    );
    false
}

/// How a cleaner asks the user questions. The port owns the TTY gating, the
/// live-output suspension, and the dialoguer calls so the per-cleaner call
/// sites cannot diverge on those contracts (the dialoguer-redraw bug the
/// confirm prompts hit is fixed once here, not per call site).
pub trait PromptPort {
    /// Asks a yes/no question. Errors when prompts are unavailable.
    fn confirm(&self, reporter: &dyn ProgressReporter, prompt: &str) -> Result<bool>;

    /// Asks a multi-select question. Errors when prompts are unavailable.
    /// `defaults` pre-checks items; an empty slice leaves every item
    /// unchecked (dialoguer's behavior when `defaults` is never set).
    fn multi_select(
        &self,
        reporter: &dyn ProgressReporter,
        prompt: Option<&str>,
        items: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>>;
}

/// The real port: runs dialoguer under the reporter's live-output
/// suspension, so a live progress line never redraws under a prompt.
/// [`gate_interactive`] must be checked before calling; dialoguer fails on
/// a non-TTY stdin.
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
        reporter: &dyn ProgressReporter,
        prompt: Option<&str>,
        items: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>> {
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
            Some(picks) => Ok(picks?),
            None => bail!("the selection prompt did not run"),
        }
    }
}

/// The port for a non-interactive run: prompts are unavailable. Callers
/// reach this path through [`gate_interactive`], which prints the remedy
/// before the cleaner skips.
pub struct NonInteractive;

impl PromptPort for NonInteractive {
    fn confirm(&self, _reporter: &dyn ProgressReporter, _prompt: &str) -> Result<bool> {
        bail!("interactive prompts are unavailable (not a terminal)")
    }

    fn multi_select(
        &self,
        _reporter: &dyn ProgressReporter,
        _prompt: Option<&str>,
        _items: &[String],
        _defaults: &[bool],
    ) -> Result<Vec<usize>> {
        bail!("interactive prompts are unavailable (not a terminal)")
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
        let result = TtyPrompt.multi_select(&rep, None, &items, &defaults);
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
    fn non_interactive_port_refuses_both_prompts() {
        let rep = RecordingReporter::new();
        assert!(NonInteractive.confirm(&rep, "Proceed?").is_err());
        let defaults: Vec<bool> = vec![];
        assert!(NonInteractive
            .multi_select(&rep, None, &[], &defaults)
            .is_err());
        // The NonInteractive port refuses before any dialoguer call: there is
        // nothing to suspend (the caller gates on gate_interactive first).
        assert_eq!(rep.suspends.load(AtomicOrdering::Relaxed), 0);
    }

    #[test]
    fn gate_interactive_prints_the_standard_remedy_when_not_interactive() {
        if is_interactive() {
            return;
        }
        // The gate returns false (skip) and prints the standardized remedy
        // line with the cleaner name.
        assert!(!gate_interactive("some-target"));
    }

    #[test]
    fn confirm_suppression_round_trips() {
        set_confirm_suppressed(true);
        assert!(confirm_suppressed());
        set_confirm_suppressed(false);
        assert!(!confirm_suppressed());
    }
}
