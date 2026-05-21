use crate::cleaner::{CleanResult, Cleaner, ScanResult};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;

#[allow(dead_code)]
pub struct ApfsSnapshotCleaner {
    pub runner: Box<dyn CommandRunner>,
}

#[allow(dead_code)]
impl ApfsSnapshotCleaner {
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        Self { runner }
    }
}

#[allow(dead_code)]
pub fn parse_snapshot_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect()
}

impl Cleaner for ApfsSnapshotCleaner {
    fn name(&self) -> &'static str {
        "apfs-snapshot"
    }

    fn detect(&self) -> ScanResult {
        todo!()
    }

    fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        todo!()
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
}
