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

    // --- PBI 74: the selection matrix ---

    /// The expected outcome of one matrix cell.
    #[derive(Debug, PartialEq, Eq)]
    enum Expected {
        /// (index, bytes) in run.
        Run(u64),
        /// (index, reason) in skipped.
        Skipped(SkipReason),
        /// In neither list.
        Neither,
    }

    /// The matrix's policy variants (Yes is constructed per cell with the
    /// cell's scan status).
    #[derive(Debug, Clone, Copy)]
    enum Variant {
        Auto,
        All,
        Yes,
    }

    /// The one expectation table for the matrix. The [policy].present entry
    /// gate is upstream of select (arm-owned, PBI 74), so the include_manual
    /// opt-in present/absent is the only policy input select sees.
    fn expected(
        variant: Variant,
        risk: Risk,
        include_manual: bool,
        status: &ScanStatus,
    ) -> Expected {
        match variant {
            // Auto classifies by risk alone: scan status is irrelevant (the
            // registry-driven flow scans nothing).
            Variant::Auto => match risk {
                Risk::Safe => Expected::Run(0),
                Risk::Moderate if include_manual => Expected::Run(0),
                Risk::Moderate => Expected::Skipped(SkipReason::NeedsOptIn(Risk::Moderate)),
                Risk::Risky => Expected::Skipped(SkipReason::NeverUnattended(Risk::Risky)),
                Risk::Manual => Expected::Skipped(SkipReason::NeverUnattended(Risk::Manual)),
            },
            // All releases EVERY non-Safe risk with the opt-in.
            Variant::All => {
                if !include_manual && risk != Risk::Safe {
                    Expected::Skipped(SkipReason::NeedsOptIn(risk))
                } else {
                    Expected::Run(0)
                }
            }
            // Yes is scan-driven: only Pruneable candidates exist, and the
            // opt-in cannot change the outcome (--yes has no flag).
            Variant::Yes => match status {
                ScanStatus::Pruneable(bytes) => {
                    if risk == Risk::Safe {
                        Expected::Run(*bytes)
                    } else if *bytes > 0 {
                        Expected::Skipped(SkipReason::NeedsExplicitSelection(risk))
                    } else {
                        Expected::Neither
                    }
                }
                _ => Expected::Neither,
            },
        }
    }

    fn selection_for(
        variant: Variant,
        risk: Risk,
        include_manual: bool,
        status: &ScanStatus,
    ) -> UnattendedSelection {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![Box::new(StubCleaner {
            name: "t",
            risk,
            status: status.clone(),
        })];
        let opts = CleanOptions {
            include_manual,
            ..Default::default()
        };
        let policy = match variant {
            Variant::Auto => UnattendedPolicy::Auto,
            Variant::All => UnattendedPolicy::All,
            Variant::Yes => UnattendedPolicy::Yes {
                results: vec![ScanResult::new("t", status.clone())],
            },
        };
        policy.select(&cleaners, &opts)
    }

    #[test]
    fn selection_matrix_pins_the_three_policies() {
        // The 3 x 4 x 2 x 2 matrix: policy variant x Risk x include_manual
        // present/absent x Pruneable/Unknown scan status. Every cell of the
        // unattended decision is pinned here, so a mis-integration of the
        // three flows fails in a unit test before it can reach a real run.
        for variant in [Variant::Auto, Variant::All, Variant::Yes] {
            for risk in [Risk::Safe, Risk::Moderate, Risk::Risky, Risk::Manual] {
                for include_manual in [false, true] {
                    for status in [ScanStatus::Pruneable(4096), ScanStatus::Unknown] {
                        let selection = selection_for(variant, risk, include_manual, &status);
                        let cell = format!(
                            "{variant:?} risk {risk:?} include_manual {include_manual} status {status:?}"
                        );
                        match expected(variant, risk, include_manual, &status) {
                            Expected::Run(bytes) => {
                                assert_eq!(
                                    selection.run,
                                    vec![(0usize, bytes)],
                                    "{cell}: must run"
                                );
                                assert!(
                                    selection.skipped.is_empty(),
                                    "{cell}: must not skip, got {:?}",
                                    selection.skipped
                                );
                            }
                            Expected::Skipped(reason) => {
                                assert!(
                                    selection.run.is_empty(),
                                    "{cell}: must not run, got {:?}",
                                    selection.run
                                );
                                assert_eq!(
                                    selection.skipped,
                                    vec![(0usize, reason)],
                                    "{cell}: skip reason"
                                );
                            }
                            Expected::Neither => {
                                assert!(
                                    selection.run.is_empty(),
                                    "{cell}: must not run, got {:?}",
                                    selection.run
                                );
                                assert!(
                                    selection.skipped.is_empty(),
                                    "{cell}: must not report, got {:?}",
                                    selection.skipped
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn auto_never_runs_risky_or_manual_even_with_include_manual() {
        // BDD: Risky は auto で決して走らない ([policy] include_manual =
        // true). No opt-in can unlock either risk in auto; only Moderate is
        // freed. This spells out include_manual's two meanings: in All it
        // releases every non-Safe risk, in auto only Moderate.
        for risk in [Risk::Risky, Risk::Manual] {
            let selection = selection_for(Variant::Auto, risk, true, &ScanStatus::Pruneable(4096));
            assert_eq!(
                selection.skipped,
                vec![(0, SkipReason::NeverUnattended(risk))],
                "no opt-in can unlock {risk:?} in auto"
            );
        }
    }

    #[test]
    fn all_releases_every_non_safe_risk_with_include_manual() {
        // BDD: all は include_manual で Risky を解放する. The same opt-in
        // that frees only Moderate in auto frees every non-Safe risk here.
        for risk in [Risk::Moderate, Risk::Risky, Risk::Manual] {
            let selection = selection_for(Variant::All, risk, true, &ScanStatus::Pruneable(4096));
            assert!(
                selection.skipped.is_empty(),
                "{risk:?} must run with the opt-in, got {:?}",
                selection.skipped
            );
            assert_eq!(selection.run, vec![(0, 0)]);
        }
    }

    #[test]
    fn yes_excludes_unknown_targets() {
        // BDD: yes は scan 駆動で Unknown を除外する. A CLI-delegating
        // cleaner that detect cannot measure is Unknown: it must not run,
        // and it has nothing to report.
        for risk in [Risk::Safe, Risk::Moderate, Risk::Risky, Risk::Manual] {
            let selection = selection_for(Variant::Yes, risk, false, &ScanStatus::Unknown);
            assert!(
                selection.run.is_empty(),
                "Unknown {risk:?} must not run, got {:?}",
                selection.run
            );
            assert!(
                selection.skipped.is_empty(),
                "Unknown {risk:?} has nothing to report, got {:?}",
                selection.skipped
            );
        }
    }

    #[test]
    fn yes_has_no_opt_in() {
        // The --yes flow has no --include-manual flag: the scan results, not
        // an opt-in, decide. Both opt-in settings classify identically.
        for risk in [Risk::Safe, Risk::Moderate, Risk::Risky, Risk::Manual] {
            let without = selection_for(Variant::Yes, risk, false, &ScanStatus::Pruneable(4096));
            let with = selection_for(Variant::Yes, risk, true, &ScanStatus::Pruneable(4096));
            assert_eq!(without, with, "include_manual must not change --yes");
        }
    }

    #[test]
    fn yes_runs_a_pruneable_zero_safe_target_and_reports_nothing_for_others() {
        // Pruneable(0) with auto_clean_allowed still runs (an empty cache
        // still cleans); with bytes == 0 a non-Safe target lands in NEITHER
        // list — the explicit-selection notice only names targets that have
        // something to reclaim.
        let safe = selection_for(Variant::Yes, Risk::Safe, false, &ScanStatus::Pruneable(0));
        assert_eq!(safe.run, vec![(0, 0)]);
        let moderate = selection_for(
            Variant::Yes,
            Risk::Moderate,
            false,
            &ScanStatus::Pruneable(0),
        );
        assert!(moderate.run.is_empty());
        assert!(moderate.skipped.is_empty());
    }

    #[test]
    fn select_preserves_registry_order_in_both_lists() {
        // detect_all preserves registration order and must not sort;
        // select must not either, so the rendered notices keep the
        // registry's order.
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(StubCleaner::pruneable("a", Risk::Moderate, 100)),
            Box::new(StubCleaner::pruneable("b", Risk::Safe, 200)),
            Box::new(StubCleaner::pruneable("c", Risk::Risky, 300)),
            Box::new(StubCleaner::pruneable("d", Risk::Safe, 400)),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        let selection =
            UnattendedPolicy::Yes { results }.select(&cleaners, &CleanOptions::default());
        assert_eq!(selection.run, vec![(1, 200), (3, 400)]);
        assert_eq!(
            selection.skipped,
            vec![
                (0, SkipReason::NeedsExplicitSelection(Risk::Moderate)),
                (2, SkipReason::NeedsExplicitSelection(Risk::Risky)),
            ]
        );
    }
}
