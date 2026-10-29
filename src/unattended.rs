//! The one owner of the unattended selection decision (PBI 74): which
//! cleaners an unattended run (`auto`, `clean all`, `--yes`) may execute.
//! Three entry points used to hand-write intentionally different filters
//! with no name and no type; [`UnattendedPolicy`] names the three policies
//! and [`UnattendedPolicy::select`] is the single place that classifies
//! cleaners into run/skipped.
//!
//! # The three policies
//!
//! * [`UnattendedPolicy::Auto`] — `sasurahime auto`, the `[policy]` apply.
//!   Registry-driven: every cleaner is classified by risk alone, nothing is
//!   scanned first. `Risky` (destructive/irreversible) and `Manual`
//!   (review-first, whole-dir removals) never run unattended
//!   ([`SkipReason::NeverUnattended`]) — no opt-in can unlock them;
//!   `Moderate` runs only when the `[policy] include_manual` opt-in is set
//!   ([`SkipReason::NeedsOptIn`]); `Safe` always runs.
//! * [`UnattendedPolicy::All`] — `sasurahime clean all`. Registry-driven.
//!   The `--include-manual` opt-in unlocks EVERY non-Safe risk (`Moderate`,
//!   `Risky` and `Manual` — unlike auto, where it frees `Moderate` only);
//!   without it they are skipped ([`SkipReason::NeedsOptIn`]).
//! * [`UnattendedPolicy::Yes`] — `sasurahime --yes`. Scan-driven: the
//!   variant carries the `detect_all` results of the same `cleaners` slice
//!   the selection runs over, so the scan asymmetry is typed — Auto/All
//!   have no results to consult and cannot grow a scan-premised filter.
//!   Only `Pruneable` targets with `auto_clean_allowed()` run (an empty
//!   `Pruneable(0)` cache still runs); a `Pruneable` target with bytes to
//!   reclaim that is not auto-clean-allowed is
//!   [`SkipReason::NeedsExplicitSelection`]; `Unknown` (and every other
//!   non-Pruneable status) is excluded from both lists.
//!
//! # What deliberately does NOT live here
//!
//! * Rendering: the wording ("never runs unattended", "set [policy]
//!   include_manual = true to include", "needs --include-manual", "that
//!   need an explicit selection") and the stdout/stderr choice are
//!   E2E-pinned facts of each call site; [`UnattendedPolicy::select`] is a
//!   pure decision so it stays unit-testable.
//! * The `auto` entry gate (`config.policy.present`): it refuses the run
//!   before any selection happens and stays at the arm — an entry gate is
//!   not a selection input.
//! * The TTY runtime facts: `gate_skipped_names` (targets whose clean gates
//!   on an interactive prompt) stays with the `--yes` flow — a per-run
//!   observation, not a policy input, and decoupled from PBI 75's port.
//! * `clean caches` is out of scope: that arm runs its seven `Moderate`
//!   generic caches unconditionally with no policy filter, by design.

use crate::cleaner::{CleanOptions, Cleaner, Risk, ScanResult, ScanStatus};

/// Which unattended entry point is selecting cleaners (PBI 74). The three
/// entry points used to hand-write intentionally different filters; the
/// type names the policies so the differences are pinned in one place
/// instead of in three comment-and-loop pairs.
pub enum UnattendedPolicy {
    /// `sasurahime auto`: the `[policy]` apply. Registry-driven — classified
    /// by risk alone, nothing scanned. The `config.policy.present` entry
    /// gate is checked by the arm before this policy runs.
    Auto,
    /// `sasurahime clean all`: the explicit full clean. Registry-driven.
    All,
    /// `sasurahime --yes`: scan-driven. `results` are the `detect_all`
    /// results of the same `cleaners` slice the selection runs over. Only
    /// `Yes` carries them, so the scan-driven flow cannot be mistaken for
    /// the registry-driven ones.
    Yes { results: Vec<ScanResult> },
}

/// One run entry: the cleaner's registry index plus the scanned bytes the
/// unattended run reports before it cleans. The scan-driven
/// [`UnattendedPolicy::Yes`] carries the target's `Pruneable` bytes (the
/// permanent-mode confirmation sums them); the registry-driven flows scan
/// nothing up front and carry 0 — the batch discovers the actual work at
/// clean time.
pub type RunEntry = (usize, u64);

/// Why an unattended run skipped a target. Each variant carries the risk
/// the skipped target reports, so every call site renders it without
/// re-deriving the classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// No opt-in can ever unlock it in this flow: auto's `Risky`
    /// (destructive/irreversible) and `Manual` (review-first, whole-dir
    /// removals) targets stay skipped even with `include_manual`.
    NeverUnattended(Risk),
    /// A flag opt-in unlocks it: `[policy] include_manual` in auto
    /// (`Moderate` only) or `--include-manual` in all (every non-Safe
    /// risk). The rendering differs per arm; the decision is shared.
    NeedsOptIn(Risk),
    /// Only an explicit per-target selection (the TUI or
    /// `sasurahime clean <target>`) runs it: the `--yes` leftovers that
    /// have bytes to reclaim but are not auto-clean-allowed.
    NeedsExplicitSelection(Risk),
}

impl SkipReason {
    /// The risk the skipped target reports; each call site renders it.
    pub fn risk(self) -> Risk {
        match self {
            SkipReason::NeverUnattended(risk)
            | SkipReason::NeedsOptIn(risk)
            | SkipReason::NeedsExplicitSelection(risk) => risk,
        }
    }
}

/// The classified outcome of one unattended selection: which cleaner
/// indices run (in registry order) and which are skipped, each with its
/// typed reason. Pure data — rendering stays at the call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnattendedSelection {
    /// Cleaner indices to run, in registry order, each with the scanned
    /// pruneable bytes (0 for the registry-driven flows, which scan
    /// nothing up front).
    pub run: Vec<RunEntry>,
    /// Cleaner indices skipped, in registry order, each with the typed
    /// reason it was left out.
    pub skipped: Vec<(usize, SkipReason)>,
}

impl UnattendedPolicy {
    /// The single owner of the unattended run/skip classification (PBI 74).
    /// Pure decision: no I/O, no rendering, no TTY facts — the call sites
    /// render [`UnattendedSelection::skipped`] with their own E2E-pinned
    /// wording.
    pub fn select(
        &self,
        cleaners: &[Box<dyn Cleaner>],
        opts: &CleanOptions,
    ) -> UnattendedSelection {
        let mut run: Vec<RunEntry> = Vec::new();
        let mut skipped: Vec<(usize, SkipReason)> = Vec::new();
        match self {
            UnattendedPolicy::Auto => {
                // The auto arm's classification, moved verbatim: Risky and
                // Manual never run unattended even with include_manual
                // (whole-dir removals must never happen without a human);
                // Moderate honors the [policy] include_manual opt-in; only
                // Safe runs without it.
                for (i, c) in cleaners.iter().enumerate() {
                    let risk = c.risk();
                    if matches!(risk, Risk::Risky | Risk::Manual) {
                        skipped.push((i, SkipReason::NeverUnattended(risk)));
                    } else if !c.auto_clean_allowed()
                        && !(opts.include_manual && risk == Risk::Moderate)
                    {
                        skipped.push((i, SkipReason::NeedsOptIn(risk)));
                    } else {
                        run.push((i, 0));
                    }
                }
            }
            UnattendedPolicy::All => {
                // The clean-all arm's classification, moved verbatim: the
                // user picked every target on purpose, so --include-manual
                // unlocks EVERY non-Safe risk (Moderate, Risky, Manual).
                for (i, c) in cleaners.iter().enumerate() {
                    if !opts.include_manual && !c.auto_clean_allowed() {
                        skipped.push((i, SkipReason::NeedsOptIn(c.risk())));
                    } else {
                        run.push((i, 0));
                    }
                }
            }
            UnattendedPolicy::Yes { results } => {
                debug_assert_eq!(
                    results.len(),
                    cleaners.len(),
                    "Yes results must be the detect_all output of the same cleaners slice"
                );
                // The run_auto classification, moved verbatim: scan-driven,
                // so only Pruneable targets are candidates.
                // auto_clean_allowed() runs (Pruneable(0) included — an
                // empty cache still cleans); a Pruneable target with bytes
                // that is not auto-clean-allowed needs an explicit
                // selection; Unknown and every other status is excluded
                // from both lists (nothing to clean, nothing to report).
                for (i, r) in results.iter().enumerate() {
                    if let ScanStatus::Pruneable(bytes) = &r.status {
                        if cleaners[i].auto_clean_allowed() {
                            run.push((i, *bytes));
                        } else if *bytes > 0 {
                            skipped
                                .push((i, SkipReason::NeedsExplicitSelection(cleaners[i].risk())));
                        }
                    }
                }
            }
        }
        UnattendedSelection { run, skipped }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::CleanResult;
    use crate::progress::ProgressReporter;
    use anyhow::Result;

    /// A test cleaner with a fixed scan status and risk. `select` is a pure
    /// decision, so clean() panics if it is ever called.
    struct StubCleaner {
        name: &'static str,
        risk: Risk,
        status: ScanStatus,
    }

    impl StubCleaner {
        fn pruneable(name: &'static str, risk: Risk, bytes: u64) -> Self {
            Self {
                name,
                risk,
                status: ScanStatus::Pruneable(bytes),
            }
        }
    }

    impl Cleaner for StubCleaner {
        fn name(&self) -> &'static str {
            self.name
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.name, self.status.clone())
        }
        fn clean(
            &self,
            _dry_run: bool,
            _reporter: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            panic!("select is a pure decision and must never clean")
        }
        fn risk(&self) -> Risk {
            self.risk
        }
    }

    #[test]
    fn yes_skips_non_auto_clean_targets_with_an_explicit_selection_reason() {
        // Port of the old auto_clean_indices pin: a target the registry's
        // ManualOnly wrapper keeps out of unattended runs (risk passthrough,
        // auto_clean_allowed false) is skipped with NeedsExplicitSelection;
        // the Safe brew target runs.
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(StubCleaner::pruneable("brew", Risk::Safe, 2000)),
            Box::new(crate::cleaners::manual_only::ManualOnly::new(Box::new(
                StubCleaner::pruneable("docker", Risk::Safe, 3000),
            ))),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        let opts = CleanOptions::default();
        let selection = UnattendedPolicy::Yes { results }.select(&cleaners, &opts);
        assert_eq!(selection.run, vec![(0, 2000)]);
        assert_eq!(
            selection.skipped,
            vec![(1, SkipReason::NeedsExplicitSelection(Risk::Safe))]
        );
    }

    #[test]
    fn yes_excludes_unavailable_targets_from_both_lists() {
        // Port of the old auto_clean_indices pin: an Unavailable target must
        // never be picked by --yes, and has nothing to report either.
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(StubCleaner::pruneable("brew", Risk::Safe, 2000)),
            Box::new(StubCleaner {
                name: "mise",
                risk: Risk::Moderate,
                status: ScanStatus::Unavailable,
            }),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        let opts = CleanOptions::default();
        let selection = UnattendedPolicy::Yes { results }.select(&cleaners, &opts);
        assert_eq!(selection.run, vec![(0, 2000)]);
        assert!(selection.skipped.is_empty());
    }
}
