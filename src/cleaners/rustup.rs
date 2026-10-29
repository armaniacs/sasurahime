use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkipCause};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct RustupCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl RustupCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
    }

    /// Removes a toolchain, checking the exit code (PBI 53): a non-zero
    /// exit or a spawn error must not count as a successful removal.
    fn remove_toolchain(
        runner: &dyn CommandRunner,
        toolchain: &str,
    ) -> std::result::Result<(), String> {
        match runner.run("rustup", &["toolchain", "remove", toolchain]) {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(format!(
                "rustup toolchain remove exited with {}",
                o.status.code().unwrap_or(-1)
            )),
            Err(err) => Err(format!("{err:#}")),
        }
    }

    /// Parses `rustup toolchain list` output.
    /// Active toolchains are marked with "(default)" or "(override)".
    fn parse_toolchains(stdout: &str) -> (Vec<String>, HashSet<String>) {
        let mut all = vec![];
        let mut active = HashSet::new();
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let name = trimmed.split_whitespace().next().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let is_active = trimmed.contains("(default)") || trimmed.contains("(override)");
            all.push(name.to_string());
            if is_active {
                active.insert(name.to_string());
            }
        }
        (all, active)
    }
}

impl Cleaner for RustupCleaner {
    fn is_available(&self) -> bool {
        self.runner.exists("rustup")
    }

    fn name(&self) -> &'static str {
        "rustup"
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("rustup") {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let output = match self.runner.run("rustup", &["toolchain", "list"]) {
            Ok(o) if o.status.success() => o,
            // rustup is installed but the command failed/timed out: the target
            // is present, so report it as unavailable instead of `not found`.
            _ => {
                return ScanResult::new(self.name(), ScanStatus::Unavailable);
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (all, active) = Self::parse_toolchains(&stdout);
        let unused: Vec<_> = all
            .iter()
            .filter(|t| !active.contains(t.as_str()))
            .collect();
        let bytes: u64 = unused
            .iter()
            .map(|t| {
                let dir = self.home.join(".rustup/toolchains").join(t);
                if dir.exists() {
                    dir_size(&dir)
                } else {
                    0
                }
            })
            .sum();
        let dir_str = self
            .home
            .join(".rustup/toolchains")
            .to_string_lossy()
            .to_string();
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(dir_str);
        }
        r
    }

    fn clean(
        &self,
        dry_run: bool,
        _reporter: &dyn ProgressReporter,
        _opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        if !self.runner.exists("rustup") {
            println!("rustup: not found, skipping");
            return Ok(CleanResult::empty(self.name()));
        }
        let output = self.runner.run("rustup", &["toolchain", "list"])?;
        if !output.status.success() {
            // PBI 53: detect() reports Unavailable when this command fails;
            // clean must fail closed the same way instead of parsing output
            // from a failed run.
            eprintln!(
                "[rustup] toolchain list exited with {} - skipping",
                output.status.code().unwrap_or(-1)
            );
            return Ok(CleanResult::empty(self.name()));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (all, active) = Self::parse_toolchains(&stdout);
        let mut freed: u64 = 0;
        let mut skipped: Vec<crate::cleaner::SkippedEntry> = vec![];

        for toolchain in &all {
            if active.contains(toolchain) {
                continue;
            }
            let size = {
                let dir = self.home.join(".rustup/toolchains").join(toolchain);
                if dir.exists() {
                    dir_size(&dir)
                } else {
                    0
                }
            };
            if dry_run {
                println!("[dry-run] [rustup] would remove toolchain: {toolchain}");
            } else {
                // A failed removal must not claim the toolchain's bytes:
                // record a SkippedEntry and continue with the remaining
                // toolchains (PBI 53).
                if let Err(reason) = Self::remove_toolchain(self.runner.as_ref(), toolchain) {
                    eprintln!("[rustup] failed to remove toolchain {toolchain}: {reason}");
                    skipped.push(crate::cleaner::SkippedEntry {
                        path: self.home.join(".rustup/toolchains").join(toolchain),
                        reason: SkipCause::Reason(reason),
                    });
                    continue;
                }
                freed += size;
                println!("[rustup] removed toolchain: {toolchain}");
            }
        }
        if freed == 0 && !dry_run {
            println!("[rustup] no unused toolchains found");
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: false,
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRunner;
    use std::fs;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use tempfile::TempDir;

    /// Runner that records calls and returns controlled output.
    struct FakeRunner {
        toolchain_list: &'static str,
    }
    impl CommandRunner for FakeRunner {
        fn run(&self, _program: &str, args: &[&str]) -> anyhow::Result<Output> {
            assert_eq!(args, &["toolchain", "list"]);
            Ok(Output {
                status: std::process::ExitStatus::default(),
                stdout: self.toolchain_list.as_bytes().to_vec(),
                stderr: vec![],
            })
        }
        fn exists(&self, _program: &str) -> bool {
            true
        }
    }

    /// rustup is installed but its command fails (spawn error / timeout).
    struct FailingRunner;
    impl CommandRunner for FailingRunner {
        fn run(&self, program: &str, _: &[&str]) -> anyhow::Result<Output> {
            anyhow::bail!("failed to spawn `{program}`: command timed out")
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn detect_returns_unavailable_when_toolchain_list_fails() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(
            tmp.path()
                .join(".rustup/toolchains/stable-aarch64-apple-darwin"),
        )
        .unwrap();
        let cleaner = RustupCleaner::new(tmp.path(), Box::new(FailingRunner));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Unavailable),
            "expected Unavailable, got {:?}",
            result.status
        );
    }

    #[test]
    fn detect_measures_actual_toolchain_dirs() {
        let tmp = TempDir::new().unwrap();
        let toolchains = tmp.path().join(".rustup/toolchains");
        // Active toolchain — has a dir with files
        fs::create_dir_all(toolchains.join("stable-aarch64-apple-darwin")).unwrap();
        fs::write(
            toolchains.join("stable-aarch64-apple-darwin/rustc"),
            [0u8; 2048],
        )
        .unwrap();
        // Unused toolchain — has a dir with files
        fs::create_dir_all(toolchains.join("nightly-2026-05-01-aarch64-apple-darwin")).unwrap();
        fs::write(
            toolchains.join("nightly-2026-05-01-aarch64-apple-darwin/rustc"),
            [0u8; 4096],
        )
        .unwrap();

        let runner = FakeRunner {
            toolchain_list:
                "stable-aarch64-apple-darwin (default)\nnightly-2026-05-01-aarch64-apple-darwin\n",
        };
        let cleaner = RustupCleaner {
            home: tmp.path().to_path_buf(),
            runner: Box::new(runner),
        };
        let result = cleaner.detect();

        let expected =
            crate::format::dir_size(&toolchains.join("nightly-2026-05-01-aarch64-apple-darwin"));
        match result.status {
            ScanStatus::Pruneable(bytes) => assert_eq!(
                bytes, expected,
                "detect must report actual dir_size of unused toolchain, not a fixed estimate"
            ),
            other => panic!("expected Pruneable, got {other:?}"),
        }
    }

    #[test]
    fn detect_returns_clean_when_all_active() {
        let tmp = TempDir::new().unwrap();
        let toolchains = tmp.path().join(".rustup/toolchains");
        fs::create_dir_all(toolchains.join("stable-aarch64-apple-darwin")).unwrap();
        fs::write(
            toolchains.join("stable-aarch64-apple-darwin/rustc"),
            [0u8; 64],
        )
        .unwrap();

        let runner = FakeRunner {
            toolchain_list: "stable-aarch64-apple-darwin (default)\n",
        };
        let cleaner = RustupCleaner {
            home: tmp.path().to_path_buf(),
            runner: Box::new(runner),
        };
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Clean),
            "expected Clean, got {:#?}",
            result.status
        );
    }

    #[test]
    fn parse_toolchains_active_only() {
        let stdout =
            "stable-aarch64-apple-darwin (default)\nnightly-2026-05-01-aarch64-apple-darwin\n";
        let (all, active) = RustupCleaner::parse_toolchains(stdout);
        assert_eq!(all.len(), 2);
        assert!(active.contains("stable-aarch64-apple-darwin"));
        assert!(!active.contains("nightly-2026-05-01-aarch64-apple-darwin"));
    }

    // ── PBI 53: exit-code verification ──

    /// rustup's `toolchain list` succeeds but `toolchain remove` exits
    /// non-zero. Args-aware so the two calls can differ.
    struct RemoveFailingRunner;
    impl CommandRunner for RemoveFailingRunner {
        fn run(&self, _program: &str, args: &[&str]) -> anyhow::Result<Output> {
            let status = if args.contains(&"list") {
                std::process::ExitStatus::from_raw(0)
            } else {
                // Unix wait-status encoding: the exit code lives in bits
                // 8-15, so `from_raw(1 << 8)` is exit code 1.
                std::process::ExitStatus::from_raw(1 << 8)
            };
            Ok(Output {
                status,
                stdout: b"nightly-2026-05-01-aarch64-apple-darwin\n".to_vec(),
                stderr: vec![],
            })
        }
        fn exists(&self, _program: &str) -> bool {
            true
        }
    }

    #[test]
    fn clean_records_skipped_entry_when_toolchain_remove_fails() {
        // A non-zero `rustup toolchain remove` exit must not count as a
        // successful removal (PBI 53): no freed bytes, a SkippedEntry, and
        // no "[rustup] removed" claim.
        let tmp = TempDir::new().unwrap();
        let toolchain_dir = tmp
            .path()
            .join(".rustup/toolchains/nightly-2026-05-01-aarch64-apple-darwin");
        fs::create_dir_all(&toolchain_dir).unwrap();
        fs::write(toolchain_dir.join("rustc"), [0u8; 512]).unwrap();
        let cleaner = RustupCleaner {
            home: tmp.path().to_path_buf(),
            runner: Box::new(RemoveFailingRunner),
        };
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "a failed removal must not count");
        assert_eq!(
            result.skipped.len(),
            1,
            "the failure must be a SkippedEntry"
        );
        assert_eq!(result.skipped[0].path, toolchain_dir);
        assert!(
            toolchain_dir.exists(),
            "the failed toolchain must be left in place"
        );
    }

    /// rustup's `toolchain list` exits non-zero in clean.
    struct ListFailingRunner;
    impl CommandRunner for ListFailingRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> anyhow::Result<Output> {
            Ok(Output {
                status: std::process::ExitStatus::from_raw(1 << 8),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _program: &str) -> bool {
            true
        }
    }

    #[test]
    fn clean_fails_closed_when_toolchain_list_exits_nonzero() {
        // detect() reports Unavailable for a failed `toolchain list`; clean
        // must fail closed the same way instead of parsing a failed run.
        let tmp = TempDir::new().unwrap();
        let toolchain_dir = tmp
            .path()
            .join(".rustup/toolchains/nightly-2026-05-01-aarch64-apple-darwin");
        fs::create_dir_all(&toolchain_dir).unwrap();
        let cleaner = RustupCleaner {
            home: tmp.path().to_path_buf(),
            runner: Box::new(ListFailingRunner),
        };
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "nothing may be deleted");
        assert!(
            toolchain_dir.exists(),
            "nothing may be deleted when the list command fails"
        );
    }

    #[test]
    fn remove_toolchain_reports_nonzero_exit() {
        let runner = crate::test_helpers::MockRunner::new().with_exit_code("rustup", 1 << 8);
        let err = RustupCleaner::remove_toolchain(&runner, "nightly").unwrap_err();
        assert!(
            err.contains("exited with 1"),
            "the exit code must surface in the reason, got {err}"
        );
    }

    #[test]
    fn remove_toolchain_reports_spawn_failure() {
        let runner = crate::test_helpers::MockRunner::new().with_failure("rustup");
        let err = RustupCleaner::remove_toolchain(&runner, "nightly").unwrap_err();
        assert!(
            err.contains("failed to spawn"),
            "the spawn failure must surface in the reason, got {err}"
        );
    }
}
