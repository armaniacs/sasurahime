use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct PreCommitCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl PreCommitCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
    }

    fn cache_dir(&self) -> PathBuf {
        // Priority: $PRE_COMMIT_HOME > $XDG_CACHE_HOME/pre-commit > ~/.cache/pre-commit
        if let Ok(dir) = std::env::var("PRE_COMMIT_HOME") {
            let p = PathBuf::from(&dir);
            if !super::generic::is_safe_delete_target(&p) {
                eprintln!(
                    "[pre-commit] WARNING: PRE_COMMIT_HOME={} points to an unsafe path, using default",
                    dir
                );
                return self.home.join(".cache/pre-commit");
            }
            return p;
        }
        if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
            let p = PathBuf::from(&xdg).join("pre-commit");
            if !super::generic::is_safe_delete_target(&p) {
                eprintln!(
                    "[pre-commit] WARNING: XDG_CACHE_HOME={} points to an unsafe path, using default",
                    xdg
                );
                return self.home.join(".cache/pre-commit");
            }
            return p;
        }
        self.home.join(".cache/pre-commit")
    }
}

impl Cleaner for PreCommitCleaner {
    fn name(&self) -> &'static str {
        "pre-commit"
    }

    fn detect(&self) -> ScanResult {
        let dir = self.cache_dir();
        if !dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let bytes = dir_size(&dir);
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.cache_dir().to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let dir = self.cache_dir();
        let config = super::generic::CliFallbackConfig {
            tool: "pre-commit",
            args: &["clean"],
            recreate: false,
        };
        super::generic::clean_cli_or_fallback(self.name(), &dir, &*self.runner, &config, dry_run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandRunner, SystemCommandRunner};
    use crate::test_helpers::EnvGuard;
    use std::fs;
    use std::process::Output;
    use tempfile::TempDir;

    /// A runner that reports no cache-cleaning tools exist, forcing the fallback path.
    struct NoToolRunner;

    impl CommandRunner for NoToolRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
            assert_eq!(
                program, "chflags",
                "NoToolRunner: unexpected program {program}"
            );
            let out = std::process::Command::new("chflags")
                .args(args)
                .output()
                .map_err(|e| anyhow::anyhow!("chflags failed: {e}"))?;
            Ok(out)
        }
        fn exists(&self, program: &str) -> bool {
            program == "chflags"
        }
    }

    /// A runner that reports pre-commit as available (simulates CLI path).
    struct CliToolRunner;

    impl CommandRunner for CliToolRunner {
        fn run(&self, program: &str, _args: &[&str]) -> Result<Output> {
            assert_eq!(
                program, "pre-commit",
                "CliToolRunner: unexpected program {program}"
            );
            use std::os::unix::process::ExitStatusExt;
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, program: &str) -> bool {
            matches!(program, "pre-commit" | "chflags")
        }
    }

    struct CliToolRunnerFailing;

    impl CommandRunner for CliToolRunnerFailing {
        fn run(&self, program: &str, _args: &[&str]) -> Result<Output> {
            assert_eq!(
                program, "pre-commit",
                "CliToolRunnerFailing: unexpected program {program}"
            );
            use std::os::unix::process::ExitStatusExt;
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(1),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, program: &str) -> bool {
            matches!(program, "pre-commit" | "chflags")
        }
    }

    #[test]
    fn detect_returns_not_found_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_returns_pruneable_when_cache_exists() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        // Use a large file to ensure dir_size() > 0 on all filesystem states
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let _home_guard = EnvGuard::set("PRE_COMMIT_HOME", cache.to_string_lossy().as_ref());

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let result = cleaner.detect();

        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn clean_dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        cleaner.clean(true, &reporter).unwrap();
        assert!(cache.exists(), "dry-run must not delete");
        assert!(
            cache.join("hook.pck").exists(),
            "dry-run must not delete files"
        );
    }

    #[test]
    fn clean_fallback_deletes_dir() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(NoToolRunner));
        let result = cleaner.clean(false, &reporter).unwrap();
        assert!(result.bytes_freed > 0);
        assert!(
            !cache.exists(),
            "cache dir should be removed in fallback mode"
        );
    }

    #[test]
    fn clean_uses_cli_when_tool_available() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(CliToolRunner));
        let result = cleaner.clean(false, &reporter).unwrap();
        assert!(result.bytes_freed > 0, "CLI path should report freed bytes");
    }

    #[test]
    fn clean_dry_run_uses_cli_when_tool_available() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(CliToolRunner));
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0, "dry-run must report 0 freed");
        assert!(
            cache.join("hook.pck").exists(),
            "dry-run must not delete files"
        );
    }

    #[test]
    fn clean_returns_error_when_cli_fails() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(CliToolRunnerFailing));
        let result = cleaner.clean(false, &reporter);
        assert!(result.is_err(), "CLI failure should propagate as error");
        let err = result.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("exit code") || msg.contains("1"),
            "error should mention exit code: {msg}"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_pre_commit_home_is_unsafe() {
        let tmp = TempDir::new().unwrap();
        let _guard = EnvGuard::set("PRE_COMMIT_HOME", "/");
        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let dir = cleaner.cache_dir();
        assert_eq!(
            dir,
            tmp.path().join(".cache/pre-commit"),
            "unsafe PRE_COMMIT_HOME=/ should fall back to default"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_xdg_cache_is_unsafe() {
        let tmp = TempDir::new().unwrap();
        let _pre_guard = EnvGuard::set("PRE_COMMIT_HOME", "");
        let _xdg_guard = EnvGuard::set("XDG_CACHE_HOME", "/etc");
        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let dir = cleaner.cache_dir();
        assert_eq!(
            dir,
            tmp.path().join(".cache/pre-commit"),
            "unsafe XDG_CACHE_HOME=/etc should fall back to default"
        );
    }

    #[test]
    fn detect_uses_xdg_cache_home_unconditionally() {
        let tmp = TempDir::new().unwrap();
        let xdg_cache = tmp.path().join("xdg-cache");
        // Do NOT create the pre-commit subdirectory under XDG_CACHE_HOME
        let _pre_guard = EnvGuard::set("PRE_COMMIT_HOME", "");
        let _xdg_guard = EnvGuard::set("XDG_CACHE_HOME", &xdg_cache.to_string_lossy());

        let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let result = cleaner.detect();

        // EnvGuard automatically restores on drop
        // With XDG_CACHE_HOME set but no pre-commit dir, should look in $XDG_CACHE_HOME/pre-commit
        // and find NotFound (not fallback to ~/.cache/pre-commit)
        assert!(matches!(result.status, ScanStatus::NotFound));
    }
}
