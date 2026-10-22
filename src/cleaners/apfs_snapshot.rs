use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::io::IsTerminal;

pub struct ApfsSnapshotCleaner {
    pub runner: Box<dyn CommandRunner>,
    all: bool,
    free_space: Box<dyn Fn() -> Option<u64> + Send + Sync>,
}

impl ApfsSnapshotCleaner {
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        Self {
            runner,
            all: false,
            free_space: Box::new(root_free_space),
        }
    }

    pub fn with_all(mut self, all: bool) -> Self {
        self.all = all;
        self
    }

    #[cfg(test)]
    fn with_free_space(mut self, f: impl Fn() -> Option<u64> + Send + Sync + 'static) -> Self {
        self.free_space = Box::new(f);
        self
    }
}

fn freed_bytes(before: u64, after: u64) -> u64 {
    after.saturating_sub(before)
}

fn root_free_space() -> Option<u64> {
    fs2::available_space("/").ok()
}

const TM_PREFIX: &str = "com.apple.TimeMachine.";

/// Returns the `YYYY-MM-DD-HHMMSS` part of a Time Machine local snapshot name.
/// `tmutil deletelocalsnapshots` accepts only this date form.
pub fn snapshot_date(name: &str) -> Option<String> {
    let rest = name.trim().strip_prefix(TM_PREFIX)?;
    let date = rest.strip_suffix(".local").unwrap_or(rest);
    let valid = date.len() == 17
        && date.chars().enumerate().all(|(i, c)| {
            if matches!(i, 4 | 7 | 10) {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        });
    if !valid {
        return None;
    }
    // `tmutil deletelocalsnapshots` accepts a date-shaped string, not a real
    // date, so an impossible timestamp would fail only after the user picked it.
    let part = |range: std::ops::Range<usize>| date[range].parse::<u32>().ok();
    let in_range =
        |value: Option<u32>, lo: u32, hi: u32| value.is_some_and(|v| (lo..=hi).contains(&v));
    if !in_range(part(5..7), 1, 12)
        || !in_range(part(8..10), 1, 31)
        || !in_range(part(11..13), 0, 23)
        || !in_range(part(13..15), 0, 59)
        || !in_range(part(15..17), 0, 59)
    {
        return None;
    }
    Some(date.to_string())
}

pub fn parse_snapshot_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| snapshot_date(l).is_some())
        .map(|l| l.to_string())
        .collect()
}

impl Cleaner for ApfsSnapshotCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Risky
    }

    fn is_available(&self) -> bool {
        self.runner.exists("tmutil")
    }

    fn name(&self) -> &'static str {
        "apfs-snapshot"
    }

    // Snapshots are deleted on the system volume via tmutil and cannot be
    // restored from the Trash, so `--yes` must never pick this target.
    fn auto_clean_allowed(&self) -> bool {
        false
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("tmutil") {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let output = match self.runner.run("tmutil", &["listlocalsnapshots", "/"]) {
            Ok(o) if o.status.success() => o,
            // tmutil exists but the command failed/timed out: snapshots cannot
            // be measured, so report unavailable instead of `not found`.
            _ => return ScanResult::new(self.name(), ScanStatus::Unavailable),
        };
        let names = parse_snapshot_names(&String::from_utf8_lossy(&output.stdout));
        if names.is_empty() {
            return ScanResult::new(self.name(), ScanStatus::Clean);
        }
        // Try to measure /.MobileBackups if present; fall back to 0.
        let size = {
            let mb = std::path::Path::new("/.MobileBackups");
            if mb.exists() {
                crate::format::dir_size(mb)
            } else {
                0
            }
        };
        // No primary_target: ApfsSnapshotCleaner operates on system-local
        // snapshots via tmutil, not a user HOME directory.
        ScanResult::new(self.name(), ScanStatus::Pruneable(size))
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.runner.exists("tmutil") {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }
        let output = self.runner.run("tmutil", &["listlocalsnapshots", "/"])?;
        let names = parse_snapshot_names(&String::from_utf8_lossy(&output.stdout));
        if names.is_empty() {
            println!("[apfs-snapshot] no local snapshots found");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }

        eprintln!(
            "⚠  Deleting snapshots disables local Time Machine protection until the next backup."
        );

        if dry_run {
            println!("[apfs-snapshot] dry-run: {} snapshot(s) found", names.len());
            for name in &names {
                println!("  would delete: {}", name);
            }
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }

        // `--all` is an explicit opt-in, so it skips the TTY guard that exists
        // only to stop an interactive prompt from being hit in a pipeline.
        if !self.all && !std::io::stdin().is_terminal() {
            eprintln!(
                "[apfs-snapshot] not a terminal — skipping. Use `sasurahime clean apfs-snapshot` \
                 for interactive selection, or `--all` to delete everything."
            );
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }

        let selected = if self.all {
            names.clone()
        } else {
            self.interactive_select(&names)?
        };
        if selected.is_empty() {
            println!("[apfs-snapshot] nothing selected");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }

        // Snapshot sizes are not reported by tmutil, so the freed amount is the
        // free-space delta. Purgeable space is updated lazily, so this can
        // under-report right after deletion.
        let before = (self.free_space)();
        let report = self.delete_snapshots(&selected);
        // Free space also moves for unrelated I/O, so a delta is only
        // attributable to us when at least one deletion succeeded.
        let bytes_freed = if report.deleted == 0 {
            0
        } else {
            match (before, (self.free_space)()) {
                (Some(b), Some(a)) => freed_bytes(b, a),
                _ => 0,
            }
        };
        if !report.failed.is_empty() {
            let dates: Vec<String> = report.failed.iter().map(|(d, _)| d.clone()).collect();
            eprint!("{}", sudo_hint(&dates));
        }
        let skipped = report
            .failed
            .iter()
            .map(|(d, reason)| SkippedEntry {
                path: std::path::PathBuf::from(d),
                reason: reason.clone(),
            })
            .collect();
        if report.deleted == 0 {
            eprintln!("[apfs-snapshot] failed: no snapshot was deleted");
        } else {
            println!("[apfs-snapshot] done");
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed,
            uses_trash: false,
            skipped,
            deleted_paths: vec![],
        })
    }
}

pub fn sudo_hint(failed_dates: &[String]) -> String {
    let mut s = String::from(
        "[apfs-snapshot] some snapshots could not be deleted (root privileges are required). Run:\n",
    );
    for d in failed_dates {
        s.push_str(&format!("  sudo tmutil deletelocalsnapshots {d}\n"));
    }
    s
}

#[derive(Debug, Default, PartialEq)]
struct DeleteReport {
    /// (date, reason) for snapshots `tmutil` could not delete. The reason is
    /// tmutil's first stderr line, or the spawn error, so skipped entries can
    /// say more than "requires root".
    failed: Vec<(String, String)>,
    deleted: usize,
    /// Names with no parseable date. They are not deleted and the user must
    /// be told, since silently skipping would look like a successful cleanup.
    unparsed_names: Vec<String>,
}

impl ApfsSnapshotCleaner {
    fn delete_snapshots(&self, names: &[String]) -> DeleteReport {
        let mut report = DeleteReport::default();
        for name in names {
            let Some(date) = snapshot_date(name) else {
                eprintln!("[apfs-snapshot] warning: cannot parse date, not deleted: {name}");
                report.unparsed_names.push(name.clone());
                continue;
            };
            match self.runner.run("tmutil", &["deletelocalsnapshots", &date]) {
                Ok(o) if o.status.success() => {
                    println!("[apfs-snapshot] deleted: {name}");
                    report.deleted += 1;
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    let first = stderr.lines().next().unwrap_or("").trim();
                    let reason = if first.is_empty() {
                        format!("tmutil exited with {:?}", o.status.code())
                    } else {
                        first.to_string()
                    };
                    report.failed.push((date, reason));
                }
                Err(e) => report.failed.push((date, format!("{e}"))),
            }
        }
        report
    }

    fn interactive_select(&self, names: &[String]) -> Result<Vec<String>> {
        use dialoguer::MultiSelect;
        let defaults = vec![true; names.len()];
        println!("\nLocal APFS Time Machine snapshots:\n");
        let selections = MultiSelect::new()
            .items(names)
            .defaults(&defaults)
            .interact()?;
        Ok(selections.into_iter().map(|i| names[i].clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::ScanStatus;
    use std::os::unix::process::ExitStatusExt;

    fn exit_ok() -> std::process::ExitStatus {
        std::process::ExitStatus::from_raw(0)
    }

    fn ok_output(stdout: &[u8]) -> std::process::Output {
        std::process::Output {
            status: exit_ok(),
            stdout: stdout.to_vec(),
            stderr: vec![],
        }
    }

    struct NoSnapshots;
    impl CommandRunner for NoSnapshots {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            Ok(ok_output(b""))
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    struct TwoSnapshots;
    impl CommandRunner for TwoSnapshots {
        fn run(&self, _: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            if args.contains(&"listlocalsnapshots") {
                Ok(ok_output(
                    b"com.apple.TimeMachine.2026-05-10-120000.local\ncom.apple.TimeMachine.2026-05-11-120000.local\n",
                ))
            } else {
                Ok(ok_output(b""))
            }
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    struct TmutilMissing;
    impl CommandRunner for TmutilMissing {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            anyhow::bail!("failed to spawn `tmutil`: No such file or directory")
        }
        fn exists(&self, program: &str) -> bool {
            program != "tmutil"
        }
    }

    #[test]
    fn parse_empty_string_returns_empty_vec() {
        assert!(parse_snapshot_names("").is_empty());
    }

    #[test]
    fn parse_whitespace_only_returns_empty_vec() {
        assert!(parse_snapshot_names("   \n  \n").is_empty());
    }

    #[test]
    fn parse_two_snapshot_lines_returns_two_names() {
        let input = "com.apple.TimeMachine.2026-05-10-120000.local\ncom.apple.TimeMachine.2026-05-11-120000.local\n";
        let names = parse_snapshot_names(input);
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "com.apple.TimeMachine.2026-05-10-120000.local");
        assert_eq!(names[1], "com.apple.TimeMachine.2026-05-11-120000.local");
    }

    #[test]
    fn parse_trims_surrounding_whitespace() {
        let input = "  com.apple.TimeMachine.2026-05-10-120000.local  \n";
        let names = parse_snapshot_names(input);
        assert_eq!(names[0], "com.apple.TimeMachine.2026-05-10-120000.local");
    }

    #[test]
    fn detect_returns_not_found_when_tmutil_missing() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(TmutilMissing));
        assert!(matches!(cleaner.detect().status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_returns_clean_when_no_snapshots() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(NoSnapshots));
        assert!(matches!(cleaner.detect().status, ScanStatus::Clean));
    }

    #[test]
    fn detect_returns_pruneable_when_snapshots_present() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(TwoSnapshots));
        assert!(matches!(cleaner.detect().status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn detect_name_is_apfs_snapshot() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(NoSnapshots));
        assert_eq!(cleaner.detect().name, "apfs-snapshot");
    }

    #[test]
    fn clean_dry_run_returns_zero_bytes_freed() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(TwoSnapshots));
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn clean_dry_run_when_no_snapshots_returns_zero() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(NoSnapshots));
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn clean_when_tmutil_missing_returns_zero() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(TmutilMissing));
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    // ── primary_target ──────────────────────────────────────────────────────
    #[test]
    fn detect_primary_target_is_none_when_verbose() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(NoSnapshots));
        let _guard = crate::test_helpers::VerboseGuard::new();
        let result = cleaner.detect();
        // ApfsSnapshotCleaner operates on system snapshots, not a user directory
        assert!(
            result.primary_target.is_none(),
            "ApfsSnapshotCleaner has no primary target"
        );
    }

    #[test]
    fn parse_ignores_header_line() {
        let input = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-05-10-120000.local\n";
        assert_eq!(
            parse_snapshot_names(input),
            vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()]
        );
    }

    #[test]
    fn parse_header_only_returns_empty() {
        assert!(parse_snapshot_names("Snapshots for disk /:\n").is_empty());
    }

    #[test]
    fn snapshot_date_extracts_timestamp() {
        assert_eq!(
            snapshot_date("com.apple.TimeMachine.2026-05-10-120000.local"),
            Some("2026-05-10-120000".to_string())
        );
        assert_eq!(snapshot_date("Snapshots for disk /:"), None);
        assert_eq!(snapshot_date("com.apple.os.update-ABCDEF"), None);
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    type Calls = Arc<Mutex<Vec<Vec<String>>>>;

    struct Recording {
        calls: Calls,
        delete_status: i32,
    }
    impl CommandRunner for Recording {
        fn run(&self, _: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            if args.contains(&"listlocalsnapshots") {
                return Ok(ok_output(
                    b"Snapshots for disk /:\ncom.apple.TimeMachine.2026-05-10-120000.local\n",
                ));
            }
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(self.delete_status << 8),
                stdout: vec![],
                stderr: b"must be root".to_vec(),
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    struct ErrRunner;
    impl CommandRunner for ErrRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            anyhow::bail!("failed to spawn `tmutil`")
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn detect_returns_unavailable_when_tmutil_fails() {
        // tmutil exists but the command fails: snapshots cannot be measured,
        // so scan must say `unavailable`, not `not found`.
        let cleaner = ApfsSnapshotCleaner::new(Box::new(ErrRunner));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Unavailable),
            "expected Unavailable, got {:?}",
            result.status
        );
    }

    #[test]
    fn delete_uses_date_and_reports_success() {
        let calls: Calls = Arc::new(Mutex::new(vec![]));
        let runner = Recording {
            calls: Arc::clone(&calls),
            delete_status: 0,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(
            report,
            DeleteReport {
                deleted: 1,
                ..Default::default()
            }
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![vec![
                "deletelocalsnapshots".to_string(),
                "2026-05-10-120000".to_string()
            ]]
        );
    }

    #[test]
    fn delete_failure_returns_sudo_hint_list() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 1,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, "2026-05-10-120000");
        assert!(report.failed[0].1.contains("must be root"));
        assert!(report.unparsed_names.is_empty());
    }

    #[test]
    fn delete_runner_error_counts_as_failure() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(ErrRunner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, "2026-05-10-120000");
        assert!(report.failed[0].1.contains("failed to spawn"));
    }

    #[test]
    fn delete_unparseable_name_is_reported_not_silently_skipped() {
        let calls: Calls = Arc::new(Mutex::new(vec![]));
        let runner = Recording {
            calls: Arc::clone(&calls),
            delete_status: 0,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.os.update-ABCDEF".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(report.unparsed_names, names);
        assert!(report.failed.is_empty());
        assert!(calls.lock().unwrap().is_empty());
    }

    /// Free-space probe that returns the scripted values in call order.
    fn scripted_free_space(
        values: Vec<Option<u64>>,
    ) -> (impl Fn() -> Option<u64> + Send + Sync, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let values = Mutex::new(values.into_iter());
        let counter = Arc::clone(&calls);
        let probe = move || {
            counter.fetch_add(1, Ordering::SeqCst);
            values.lock().unwrap().next().flatten()
        };
        (probe, calls)
    }

    #[test]
    fn clean_all_reports_free_space_delta_as_bytes_freed() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 0,
        };
        let (probe, _) = scripted_free_space(vec![Some(100), Some(350)]);
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner))
            .with_all(true)
            .with_free_space(probe);
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 250);
    }

    #[test]
    fn clean_all_reports_zero_when_free_space_unavailable() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 0,
        };
        // Only the "after" reading failing must not be read as the whole disk being freed.
        let (probe, _) = scripted_free_space(vec![None, Some(350)]);
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner))
            .with_all(true)
            .with_free_space(probe);
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    struct Mixed;
    impl CommandRunner for Mixed {
        fn run(&self, _: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            if args.contains(&"listlocalsnapshots") {
                return Ok(ok_output(
                    b"com.apple.TimeMachine.2026-05-10-120000.local\ncom.apple.TimeMachine.2026-05-11-120000.local\n",
                ));
            }
            let code = if args.contains(&"2026-05-10-120000") {
                0
            } else {
                1
            };
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn clean_all_failed_reports_zero_bytes_and_exit_code_1() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 1,
        };
        let (probe, _) = scripted_free_space(vec![Some(100), Some(500)]);
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner))
            .with_all(true)
            .with_free_space(probe);
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.exit_code(), 1);
    }

    #[test]
    fn clean_partial_success_reports_delta_and_skips_failed() {
        let (probe, _) = scripted_free_space(vec![Some(100), Some(400)]);
        let cleaner = ApfsSnapshotCleaner::new(Box::new(Mixed))
            .with_all(true)
            .with_free_space(probe);
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 300);
        assert_eq!(result.skipped.len(), 1);
        // The date lives in the path; the reason reports what tmutil said.
        assert!(result.skipped[0].path.ends_with("2026-05-11-120000"));
        assert!(
            result.skipped[0].reason.contains("tmutil exited"),
            "{:?}",
            result.skipped[0].reason
        );
        assert_eq!(result.exit_code(), 0);
    }

    #[test]
    fn parse_mixed_input_keeps_only_time_machine_lines() {
        let out = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-05-10-120000.local\ncom.apple.os.update-ABC123\ncom.apple.TimeMachine.bogus.local\n";
        assert_eq!(
            parse_snapshot_names(out),
            vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()]
        );
    }

    #[test]
    fn clean_dry_run_does_not_measure_free_space() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 0,
        };
        let (probe, probe_calls) = scripted_free_space(vec![Some(100), Some(350)]);
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner))
            .with_all(true)
            .with_free_space(probe);
        let reporter = crate::progress::VerboseProgress::new();
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(probe_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn freed_bytes_is_free_space_delta() {
        assert_eq!(freed_bytes(100, 350), 250);
        assert_eq!(freed_bytes(350, 100), 0);
    }

    #[test]
    fn sudo_hint_lists_each_failed_date() {
        let hint = sudo_hint(&["2026-05-10-120000".to_string()]);
        assert!(hint.contains("sudo tmutil deletelocalsnapshots 2026-05-10-120000"));
    }

    #[test]
    fn sudo_hint_lists_multiple_dates_on_separate_lines() {
        let hint = sudo_hint(&[
            "2026-05-10-120000".to_string(),
            "2026-05-11-120000".to_string(),
        ]);
        assert!(hint.contains("  sudo tmutil deletelocalsnapshots 2026-05-10-120000\n"));
        assert!(hint.contains("  sudo tmutil deletelocalsnapshots 2026-05-11-120000\n"));
    }

    #[test]
    fn all_flag_deletes_without_terminal() {
        let calls: Calls = Arc::new(Mutex::new(vec![]));
        let runner = Recording {
            calls: Arc::clone(&calls),
            delete_status: 0,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner)).with_all(true);
        let reporter = crate::progress::VerboseProgress::new();
        // Non-TTY under `cargo test`: the deletion path must run, not the skip path.
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.name, "apfs-snapshot");
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                vec!["listlocalsnapshots".to_string(), "/".to_string()],
                vec![
                    "deletelocalsnapshots".to_string(),
                    "2026-05-10-120000".to_string()
                ],
            ]
        );
    }

    #[test]
    fn all_flag_dry_run_never_calls_delete() {
        let calls: Calls = Arc::new(Mutex::new(vec![]));
        let runner = Recording {
            calls: Arc::clone(&calls),
            delete_status: 0,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner)).with_all(true);
        let reporter = crate::progress::VerboseProgress::new();
        cleaner.clean(true, &reporter).unwrap();
        assert!(!calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.contains(&"deletelocalsnapshots".to_string())));
    }

    #[test]
    fn apfs_snapshot_is_never_auto_cleaned() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(NoSnapshots));
        assert!(!cleaner.auto_clean_allowed());
    }

    #[test]
    fn snapshot_date_rejects_out_of_range_parts() {
        for bad in [
            "com.apple.TimeMachine.2026-99-10-120000.local",
            "com.apple.TimeMachine.2026-12-99-120000.local",
            "com.apple.TimeMachine.2026-12-10-990000.local",
            "com.apple.TimeMachine.2026-12-10-126000.local",
            "com.apple.TimeMachine.2026-12-10-120060.local",
            "com.apple.TimeMachine.2026-00-10-120000.local",
            "com.apple.TimeMachine.2026-12-00-120000.local",
        ] {
            assert_eq!(snapshot_date(bad), None, "{bad}");
        }
    }

    #[test]
    fn snapshot_date_accepts_leap_day_and_bounds() {
        for good in [
            "com.apple.TimeMachine.2024-02-29-235959.local",
            "com.apple.TimeMachine.2026-01-01-000000.local",
            "com.apple.TimeMachine.2026-12-31-235959",
        ] {
            assert!(snapshot_date(good).is_some(), "{good}");
        }
    }

    #[test]
    fn delete_failure_keeps_reason_from_tmutil() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 1,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(report.failed.len(), 1);
        assert_eq!(report.failed[0].0, "2026-05-10-120000");
        assert!(
            report.failed[0].1.contains("must be root"),
            "{:?}",
            report.failed[0]
        );
    }

    #[test]
    fn delete_runner_error_keeps_the_error_as_reason() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(ErrRunner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert!(
            report.failed[0].1.contains("failed to spawn"),
            "{:?}",
            report.failed[0]
        );
    }

    #[test]
    fn clean_all_puts_tmutil_output_into_skipped_reason() {
        let runner = Recording {
            calls: Arc::new(Mutex::new(vec![])),
            delete_status: 1,
        };
        let (probe, _) = scripted_free_space(vec![Some(100), Some(100)]);
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner))
            .with_all(true)
            .with_free_space(probe);
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(result.skipped.len(), 1);
        assert!(
            result.skipped[0].reason.contains("must be root"),
            "{:?}",
            result.skipped[0].reason
        );
        assert_eq!(result.exit_code(), 1);
    }
}
