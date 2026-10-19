use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::io::IsTerminal;

pub struct ApfsSnapshotCleaner {
    pub runner: Box<dyn CommandRunner>,
}

impl ApfsSnapshotCleaner {
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        Self { runner }
    }
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
    valid.then(|| date.to_string())
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
    fn is_available(&self) -> bool {
        self.runner.exists("tmutil")
    }

    fn name(&self) -> &'static str {
        "apfs-snapshot"
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("tmutil") {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let output = match self.runner.run("tmutil", &["listlocalsnapshots", "/"]) {
            Ok(o) => o,
            Err(_) => return ScanResult::new(self.name(), ScanStatus::NotFound),
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

        if !std::io::stdin().is_terminal() {
            eprintln!(
                "[apfs-snapshot] not a terminal — skipping. Use `sasurahime clean apfs-snapshot` \
                 for interactive selection."
            );
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }

        let selected = self.interactive_select(&names)?;
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

        let report = self.delete_snapshots(&selected);
        if !report.failed_dates.is_empty() {
            eprint!("{}", sudo_hint(&report.failed_dates));
        }
        // Snapshot size cannot be measured after deletion; report 0.
        println!("[apfs-snapshot] done");
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: 0,
            uses_trash: false,
            skipped: vec![],
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
    /// Dates `tmutil` could not delete; these get the sudo hint.
    failed_dates: Vec<String>,
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
                Ok(o) if o.status.success() => println!("[apfs-snapshot] deleted: {name}"),
                Ok(_) | Err(_) => report.failed_dates.push(date),
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
    fn delete_uses_date_and_reports_success() {
        let calls: Calls = Arc::new(Mutex::new(vec![]));
        let runner = Recording {
            calls: Arc::clone(&calls),
            delete_status: 0,
        };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(report, DeleteReport::default());
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
        assert_eq!(report.failed_dates, vec!["2026-05-10-120000".to_string()]);
        assert!(report.unparsed_names.is_empty());
    }

    #[test]
    fn delete_runner_error_counts_as_failure() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(ErrRunner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let report = cleaner.delete_snapshots(&names);
        assert_eq!(report.failed_dates, vec!["2026-05-10-120000".to_string()]);
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
        assert!(report.failed_dates.is_empty());
        assert!(calls.lock().unwrap().is_empty());
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
}
