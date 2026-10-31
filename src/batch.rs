//! One batch-execution path for the duplicated "clean a batch, accumulate,
//! write history" loops. Every entry point that cleans more than one target
//! funnels through [`run_batch`]; skip policies stay at the call sites —
//! they are genuinely different per entry point.

/// One batch failure carried as a typed error: the cleaner name plus the
/// whole `anyhow::Error` (cause/context chain intact), instead of a
/// flattened `to_string()` that throws the chain away at collection time.
/// Stage 3 (single/batch unification into `report_single_target`) rebuilds
/// the single-target `Err` contract from this — see `run_batch`'s doc.
#[derive(Debug)]
pub struct BatchFailure {
    pub name: String,
    pub error: anyhow::Error,
}

/// Summary of a batch clean run: what the callers print afterwards.
pub struct BatchSummary {
    pub total_freed: u64,
    /// Per-target failures (typed error carry), reported by the caller.
    pub failures: Vec<BatchFailure>,
    /// Targets the user cancelled (cleaner name), reported by the caller as
    /// a neutral `cancelled by user` line — a cancel is a run-level
    /// decision, not an error, so it never lands in `failures`.
    pub cancelled: Vec<String>,
    /// Per-entry skips (cleaner name + "path: reason"), reported by the
    /// caller in the single-target "N file(s) skipped" shape.
    pub skipped: Vec<(String, String)>,
    /// Stage-3 carry: each successful cleaner's `CleanResult`, in loop
    /// order, so the single/batch unification can rebuild the single-target
    /// contract (a "succeeded but exit_code() == 1" result survives the
    /// summary instead of being compressed into `total_freed`). Unused for
    /// display today — a deliberate preparation, see `run_batch`'s doc.
    pub results: Vec<crate::cleaner::CleanResult>,
}

/// The ONE batch execution path: spinner per cleaner, result accumulation,
/// post-clean notes, the single history-write guard, and per-target error
/// collection. Skip policies stay at the call sites — they are genuinely
/// different per entry point.
///
/// Reporter use per branch: the ANIMATED branch binds the cleaner's progress
/// to the shared spinner (`with_spinner_result_progress`); the NON-animated
/// branch passes `reporter` straight to the cleaner. The [OK]/[FAILED]
/// status line is owned by the reporter kind on both paths: the non-animated
/// branch prints it only when `reporter.show_spinner()` (a quiet reporter —
/// deep-suppress — gets none, matching the single-target path), and the
/// animated branch is keyed on the caller's `animated` flag, which is never
/// derived from `show_spinner` (no-tick-while-prompting: a cleaner on this
/// path may still prompt, and a derived steady tick would redraw the line
/// under the prompt).
///
/// Output-line families: each family keeps its own owner on the two engines
/// (`run_clean_target` = single target, [`run_batch`] = batch) — the paths
/// share the engines, not the printing.
///
/// | # | Line family | Single target (`run_clean_target`) | Batch ([`run_batch`]) | Gate |
/// |---|---|---|---|---|
/// | 1 | status line (`[OK]`/`[FAILED]`) | `show_spinner()` branch (main.rs:893-915) | reporter kind: animated → `with_spinner_result_progress` / `show_spinner()` → `with_spinner_result` / quiet → none | `animated` is the caller's flag, never derived from `show_spinner()` |
/// | 2 | cancel line | `Ok(cancelled)`, silent | `summary.cancelled` → the caller's neutral `cancelled by user` line | `Err(CleanCancelled)` downcast branch |
/// | 3 | note line (`post_clean_note`) | inside the `show_spinner()` branch (main.rs:934-940) | `with_notes` && `reporter.show_spinner()` (unified in PBI 90) | `--deep-suppress` hides; default and `--suppress` show |
/// | 4 | freed line | per-target `freed_message`, `show_spinner()` branch only | the caller's closing `Total freed` line (`report_total_freed`) | per-target and total are separate concepts |
/// | 5 | skip lines | direct `N file(s) skipped` + `path: reason` (nameless) | `summary.skipped` → `report_batch_failures` (named shape, cleaner name included) | when `skipped` is non-empty |
/// | 6 | exit code | `exit_code() != 0` → exit 1 | always exit 0 (report only) | pinned by the PBI 78 scope fence 2 |
///
/// Stage-3 carry (deliberate preparation, NOT dead weight): `failures`
/// keeps each error as an `anyhow::Error` (cause/context chain intact) and
/// `results` keeps each successful [`CleanResult`] in loop order, so stage
/// 3 — the single/batch unification into `report_single_target`, deferred
/// above — can rebuild the single-target `Err` contract and the "succeeded
/// but exit_code() == 1" result without loss. Today the only display reader
/// is `report_batch_failures`; `results` has no display reader yet.
///
/// Stage-3 trap: do NOT let `report_single_target` display or record
/// `results[0].name` — `CleanResult::name` is a pub field whose equality
/// with the batch loop's `c.name()` is NOT pinned by any test (a non-
/// invariant), so the display and history labels must keep coming from the
/// batch's own cleaner name, the same label the branches above already use.
///
/// Option A (full unification of the two paths) stays deferred; it fires on
/// the next change that touches either engine's semantics (a new line
/// family, a gate move, or a gate condition change), since the previous
/// trigger "next new entry point" never fires naturally.
pub fn run_batch(
    cleaners: &[&dyn crate::cleaner::Cleaner],
    opts: &crate::cleaner::CleanOptions,
    reporter: &dyn crate::progress::ProgressReporter,
    dry_run: bool,
    animated: bool,
    with_notes: bool,
) -> BatchSummary {
    let mut summary = BatchSummary {
        total_freed: 0,
        failures: Vec::new(),
        cancelled: Vec::new(),
        skipped: Vec::new(),
        results: Vec::new(),
    };
    for c in cleaners {
        let msg = format!("Cleaning {}...", c.name());
        let result = if animated {
            // No steady tick while a cleaner may still prompt: the animated
            // branch is for flows that already suppressed its prompts.
            crate::progress::with_spinner_result_progress(&msg, animated, |rep| {
                c.clean(dry_run, rep, opts)
            })
        } else if reporter.show_spinner() {
            // Spinner reporter: the plain non-animated spinner draws the
            // one status line, exactly like the single-target path.
            crate::progress::with_spinner_result(&msg, || c.clean(dry_run, reporter, opts))
        } else {
            // Quiet reporter (deep-suppress): no spinner, no status line —
            // the single-target path's show_spinner()==false branch prints
            // none either.
            c.clean(dry_run, reporter, opts)
        };
        match result {
            Ok(r) => {
                summary.total_freed += r.bytes_freed;
                for s in &r.skipped {
                    summary.skipped.push((
                        c.name().to_string(),
                        format!("{}: {}", s.path.display(), s.reason),
                    ));
                }
                // Notes follow the status line's gate (PBI 90): the quiet
                // deep-suppress reporter gets no per-target note either, so
                // `clean all --deep-suppress` stays as quiet as the
                // single-target path's show_spinner()==false branch.
                if with_notes && reporter.show_spinner() {
                    let trash_mode = opts.trash_mode == crate::trash::TrashMode::Trash;
                    if let Some(note) = post_clean_note(r.bytes_freed, r.uses_trash, trash_mode) {
                        println!("{note}");
                    }
                }
                write_history_entry_if_needed(
                    c.name(),
                    dry_run,
                    r.bytes_freed,
                    r.skipped.len(),
                    r.uses_trash,
                );
                // Stage-3 carry (see the doc): the single-target result is
                // parked in the summary LAST — the skipped/history/note
                // branches above consume `r`'s fields, and pushing before
                // them would move `r` out from under those reads.
                summary.results.push(r);
            }
            Err(e) => {
                // CleanCancelled is a run-level decision, not an error: it
                // gets its own channel so the caller reports a neutral
                // `cancelled by user` line instead of `Error cleaning ...`.
                // The name comes from the loop variable `c` (CleanResult::
                // cancelled() is intentionally nameless, so CleanResult.name
                // has nothing to give); remaining targets keep running.
                if e.is::<crate::cleaner::CleanCancelled>() {
                    summary.cancelled.push(c.name().to_string());
                } else {
                    // The error itself rides in the summary (R1: no
                    // `to_string()` flattening here) so the anyhow chain
                    // survives for stage 3's Err reconstruction.
                    summary.failures.push(BatchFailure {
                        name: c.name().to_string(),
                        error: e,
                    });
                }
            }
        }
    }
    summary
}

/// Minimal audit trail / restore guidance printed after a clean, so the
/// record exists even without `--verbose`. `None` when nothing was freed.
/// `uses_trash` is the cleaner's own report: a cleaner that deletes outside
/// the Trash (tmutil, rustup, ...) must not claim its bytes can be restored.
pub fn post_clean_note(bytes_freed: u64, uses_trash: bool, trash_mode: bool) -> Option<String> {
    if bytes_freed == 0 {
        return None;
    }
    if trash_mode {
        if uses_trash {
            Some("Restore from Finder (Trash) or run `open ~/.Trash`.".to_string())
        } else {
            None
        }
    } else {
        Some("Permanently deleted: no Trash copy was kept.".to_string())
    }
}

/// The one history-record guard, shared by [`run_batch`] and the single-target
/// path: entries are recorded only for real runs that freed something.
///
/// WHY zero-byte runs record nothing (pre-existing policy, kept on purpose):
/// zero-byte runs — every deletion skipped, empty-dir removals, or CLI tools
/// reporting 0 freed (brew stdout parsing, `size_before - size_after`) — are
/// not recorded. The history table tracks recovered bytes; a deletion without
/// bytes has no row. This is the only place that decides: callers here and
/// `history::write_history_entry` below do not re-check `dry_run` or bytes.
pub fn write_history_entry_if_needed(
    label: &str,
    dry_run: bool,
    bytes_freed: u64,
    skipped_count: usize,
    uses_trash: bool,
) {
    if !dry_run && bytes_freed > 0 {
        crate::history::write_history_entry(label, bytes_freed, skipped_count, uses_trash);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::{CleanOptions, CleanResult, ScanResult, ScanStatus};
    use crate::progress::ProgressReporter;
    use anyhow::Result;
    use tempfile::TempDir;

    /// Stub cleaner that always succeeds and frees a fixed byte count.
    struct OkCleaner(&'static str);

    impl crate::cleaner::Cleaner for OkCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Ok(CleanResult::freed(self.0, 100, false))
        }
    }

    /// Stub cleaner that always fails.
    struct FailingCleaner(&'static str);

    impl crate::cleaner::Cleaner for FailingCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Err(anyhow::anyhow!("boom"))
        }
    }

    /// Stub cleaner that always cancels: the `Err(CleanCancelled)` shape a
    /// user decline returns (the CleanCancelled source itself is not touched
    /// by this PBI — a hand-made Err drives the summary instead).
    struct CancelledCleaner(&'static str);

    impl crate::cleaner::Cleaner for CancelledCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Err(anyhow::Error::from(crate::cleaner::CleanCancelled))
        }
    }

    /// Stub cleaner that succeeds with one per-entry skip recorded
    /// (seam-classified skippable failure, PBI-43 shape).
    struct SkippingCleaner(&'static str);

    impl crate::cleaner::Cleaner for SkippingCleaner {
        fn name(&self) -> &'static str {
            self.0
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new(self.0, ScanStatus::Clean)
        }
        fn clean(
            &self,
            _dry_run: bool,
            _rep: &dyn ProgressReporter,
            _opts: &CleanOptions,
        ) -> Result<CleanResult> {
            Ok(CleanResult {
                name: self.0,
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![crate::cleaner::SkippedEntry {
                    path: std::path::PathBuf::from("/tmp/locked-cache"),
                    reason: crate::cleaner::SkipCause::Delete(
                        crate::cleaner::DeleteError::TrashRefused("boom".to_string()),
                    ),
                }],
            })
        }
    }

    #[test]
    fn run_batch_carries_skipped_entries() {
        // A per-entry skippable failure must be carried by the summary so
        // every batch entry point can report it in the single-target
        // "N file(s) skipped" shape.
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(SkippingCleaner("skip-a")),
            Box::new(OkCleaner("ok-a")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            true,  // dry_run: never deletes and never writes history
            false, // animated
            false, // with_notes
        );
        assert_eq!(
            summary.skipped.len(),
            1,
            "the one skipped entry must be carried by the summary"
        );
        assert_eq!(
            summary.skipped[0].0, "skip-a",
            "the skip must name the cleaner"
        );
        assert!(
            summary.skipped[0].1.contains("locked-cache"),
            "the skip must carry the path, got: {}",
            summary.skipped[0].1
        );
        assert!(
            summary.skipped[0].1.contains("trash failed"),
            "the skip must carry the reason, got: {}",
            summary.skipped[0].1
        );
    }

    #[test]
    fn run_batch_sums_only_successful_cleans_and_collects_failures() {
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(OkCleaner("ok-a")),
            Box::new(FailingCleaner("bad")),
            Box::new(OkCleaner("ok-b")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            true,  // dry_run: never deletes and never writes history
            false, // animated
            false, // with_notes
        );
        assert_eq!(
            summary.total_freed, 200,
            "only the successful cleans sum into total_freed"
        );
        assert_eq!(summary.failures.len(), 1, "one failure for the one error");
        assert_eq!(summary.failures[0].name, "bad");
        assert!(
            summary.failures[0].error.to_string().contains("boom"),
            "the failure message must carry the error, got: {}",
            summary.failures[0].error
        );
    }

    #[test]
    fn run_batch_collects_cancelled_separately_from_failures() {
        // A mixed batch must keep cancel apart from failure: cancelled
        // target names go to `cancelled`, other errors go to `failures`,
        // and the two never mix (cancel is a run-level decision, not an
        // error — PBI 78).
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(CancelledCleaner("cancel-a")),
            Box::new(FailingCleaner("bad")),
            Box::new(OkCleaner("ok-a")),
            Box::new(CancelledCleaner("cancel-b")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            true,  // dry_run: never deletes and never writes history
            false, // animated
            false, // with_notes
        );
        assert_eq!(
            summary.cancelled,
            vec!["cancel-a".to_string(), "cancel-b".to_string()],
            "cancelled target names must be collected, in loop order"
        );
        assert_eq!(
            summary.failures.len(),
            1,
            "only the real error is a failure"
        );
        assert_eq!(summary.failures[0].name, "bad");
        assert_eq!(
            summary.total_freed, 100,
            "the successful clean still sums into total_freed"
        );
    }

    #[test]
    fn run_batch_reports_cancelled_not_failed() {
        // A cancelled cleaner must never land in failures: the caller
        // reports `{name}: cancelled by user`, never `Error cleaning ...`.
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> =
            vec![Box::new(CancelledCleaner("colima"))];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            true,  // dry_run
            false, // animated
            false, // with_notes
        );
        assert_eq!(summary.cancelled, vec!["colima".to_string()]);
        assert!(
            summary.failures.is_empty(),
            "a user cancel is not a failure, got: {:?}",
            summary.failures
        );
        assert_eq!(summary.total_freed, 0);
    }

    #[test]
    fn run_batch_animated_path_accumulates_like_the_plain_path() {
        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![
            Box::new(OkCleaner("anim")),
            Box::new(FailingCleaner("bad-anim")),
        ];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();
        let summary = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            true,
            true,  // animated: with_spinner_result_progress
            false, // with_notes
        );
        assert_eq!(summary.total_freed, 100);
        assert_eq!(summary.failures.len(), 1);
        assert_eq!(summary.failures[0].name, "bad-anim");
    }

    #[test]
    fn run_batch_status_line_is_owned_by_the_reporter_kind() {
        // The non-animated branch prints its [OK]/[FAILED] status line only
        // when reporter.show_spinner(): a quiet reporter (deep-suppress)
        // gets none, matching the single-target path's show_spinner()==
        // false branch (PBI 78). The animated branch must stay first and
        // stay keyed on the caller's animated flag — animated is never
        // derived from show_spinner (no-tick-while-prompting: a cleaner on
        // the non-animated path may still prompt, so a derived steady tick
        // would redraw the line under the prompt). indicatif exposes no
        // output-capture seam here, so this pins the source (the same style
        // as the progress.rs steady-tick pins).
        let src = include_str!("batch.rs");
        let start = src
            .find("let result = if animated {")
            .expect("run_batch's branch dispatch must be present");
        let end = src[start..]
            .find("match result {")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            body.starts_with("let result = if animated {"),
            "the animated branch must stay first and keyed on the caller's animated flag"
        );
        assert!(
            body.contains("} else if reporter.show_spinner() {"),
            "the non-animated branch must gate the status line on reporter.show_spinner() (deep-suppress gets no [OK] line)"
        );
        assert!(
            !body.contains("animated = "),
            "animated must never be derived from another flag (no-tick-while-prompting)"
        );
    }

    #[test]
    fn run_batch_writes_history_only_when_not_dry_run() {
        // HOME pointed at a tempdir makes write_history_entry observable:
        // the history file lands under $HOME/.local/share/sasurahime. The
        // swap is serialized under TEST_LOCK because the HOME variable is
        // process-global: a concurrent history probe (the zero-byte test
        // below) writes under the swapped HOME, so a racing swap sends its
        // rows into the wrong directory.
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Declared after the lock so the guard restores HOME while the lock
        // is still held, then the lock is released.
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        let history_path = home.join(".local/share/sasurahime/history.json");

        let boxed: Vec<Box<dyn crate::cleaner::Cleaner>> = vec![Box::new(OkCleaner("hist-a"))];
        let cleaners: Vec<&dyn crate::cleaner::Cleaner> =
            boxed.iter().map(|b| b.as_ref()).collect();

        // Dry run: nothing deleted, nothing recorded.
        let _ = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            true,
            false,
            false,
        );
        assert!(!history_path.exists(), "dry run must not write history");

        // Real run: one history entry for the one successful cleaner.
        let _ = run_batch(
            &cleaners,
            &CleanOptions::default(),
            &crate::progress::SuppressReporter,
            false,
            false,
            false,
        );
        let entries = crate::history::load_history(&history_path);
        assert_eq!(entries.len(), 1, "one entry for the one success");
        assert_eq!(entries[0].cleaner, "hist-a");
        assert_eq!(entries[0].freed_bytes, 100);
        assert!(!entries[0].moved_to_trash);
    }

    #[test]
    fn write_history_entry_if_needed_records_nothing_for_zero_byte_real_runs() {
        // Pins the zero-byte policy: a real run that freed 0 bytes (all
        // deletions skipped, empty dirs, a CLI tool reporting 0 freed) has no
        // history row — the history table tracks recovered bytes. The guard
        // short-circuits before any I/O, so a unique path nobody else writes
        // (no HOME swap: that races with the parallel EnvGuard test) must stay
        // absent.
        let tmp = TempDir::new().unwrap();
        let history_path = tmp.path().join(".local/share/sasurahime/history.json");

        write_history_entry_if_needed("zero-byte", false, 0, 0, false);
        assert!(
            !history_path.exists(),
            "a real zero-byte run must not be recorded"
        );
    }

    #[test]
    fn post_clean_note_shows_restore_guidance_for_trash_runs() {
        let note = post_clean_note(2048, true, true).expect("restore note");
        assert!(
            note.contains("Finder") && note.contains("~/.Trash"),
            "restore guidance must mention Finder and ~/.Trash, got: {note}"
        );
        // A cleaner that never moves to Trash prints nothing in trash mode.
        assert_eq!(post_clean_note(2048, false, true), None);
    }

    #[test]
    fn post_clean_note_keeps_a_permanent_audit_trail() {
        let note = post_clean_note(2048, true, false).expect("audit note");
        assert!(
            note.contains("Permanently deleted"),
            "permanent runs must keep a minimal audit trail, got: {note}"
        );
        assert_eq!(
            post_clean_note(2048, false, false).expect("audit note"),
            "Permanently deleted: no Trash copy was kept."
        );
    }

    #[test]
    fn post_clean_note_is_silent_when_nothing_was_freed() {
        assert_eq!(post_clean_note(0, true, true), None);
        assert_eq!(post_clean_note(0, false, false), None);
    }
}
