//! One batch-execution path for the duplicated "clean a batch, accumulate,
//! write history" loops. Every entry point that cleans more than one target
//! funnels through [`run_batch`]; skip policies stay at the call sites —
//! they are genuinely different per entry point.

/// Summary of a batch clean run: what the callers print afterwards.
pub struct BatchSummary {
    pub total_freed: u64,
    /// Per-target failures (cleaner name + error), reported by the caller.
    pub failures: Vec<(String, String)>,
    /// Per-entry skips (cleaner name + "path: reason"), reported by the
    /// caller in the single-target "N file(s) skipped" shape.
    pub skipped: Vec<(String, String)>,
}

/// The ONE batch execution path: spinner per cleaner, result accumulation,
/// post-clean notes, the single history-write guard, and per-target error
/// collection. Skip policies stay at the call sites — they are genuinely
/// different per entry point.
///
/// Reporter use per branch: the ANIMATED branch binds the cleaner's progress
/// to the shared spinner (`with_spinner_result_progress`); the NON-animated
/// branch passes `reporter` straight to the cleaner, so the caller's reporter
/// carries the progress output while the plain spinner here is only a label —
/// and enables no steady tick (no-tick-while-prompting: a cleaner on this
/// path may still prompt). `reporter` is therefore only dead as a spinner
/// source, never as the cleaner's progress sink.
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
        skipped: Vec::new(),
    };
    for c in cleaners {
        let msg = format!("Cleaning {}...", c.name());
        let result = if animated {
            // No steady tick while a cleaner may still prompt: the animated
            // branch is for flows that already suppressed its prompts.
            crate::progress::with_spinner_result_progress(&msg, animated, |rep| {
                c.clean(dry_run, rep, opts)
            })
        } else {
            crate::progress::with_spinner_result(&msg, || c.clean(dry_run, reporter, opts))
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
                if with_notes {
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
            }
            Err(e) => summary.failures.push((c.name().to_string(), e.to_string())),
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
                deleted_paths: vec![],
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
        assert_eq!(summary.failures[0].0, "bad");
        assert!(
            summary.failures[0].1.contains("boom"),
            "the failure message must carry the error, got: {}",
            summary.failures[0].1
        );
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
        assert_eq!(summary.failures[0].0, "bad-anim");
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
