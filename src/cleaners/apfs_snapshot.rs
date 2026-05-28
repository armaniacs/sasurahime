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

pub fn parse_snapshot_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
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

        for name in &selected {
            match self
                .runner
                .run("tmutil", &["deletelocalsnapshot", "/", name])
            {
                Ok(_) => println!("[apfs-snapshot] deleted: {}", name),
                Err(e) => log::error!("[apfs-snapshot] error deleting {}: {e}", name),
            }
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

impl ApfsSnapshotCleaner {
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
}
