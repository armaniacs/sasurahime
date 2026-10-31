use crate::cleaner::{CleanResult, Cleaner, ScanResult};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::Path;

/// Thin wrapper over [`crate::cli_fallback::CliFallbackCleaner`]: this cleaner differs
/// from the shared adapter only in the cache dir it resolves at construction
/// (PRE_COMMIT_HOME / XDG_CACHE_HOME) and its CLI tool configuration, so the
/// detect/clean skeleton lives in the adapter.
pub struct PreCommitCleaner(crate::cli_fallback::CliFallbackCleaner);

fn config() -> crate::cli_fallback::CliFallbackConfig<'static> {
    crate::cli_fallback::CliFallbackConfig {
        tool: "pre-commit",
        args: &["clean"],
        recreate: false,
    }
}

impl PreCommitCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        // Priority: $PRE_COMMIT_HOME > $XDG_CACHE_HOME/pre-commit > ~/.cache/pre-commit
        // Each override value is validated BEFORE any subpath is joined.
        // Resolved ONCE at construction (PBI 88): the resolved value is
        // pinned on the cleaner and detect/clean never re-read the process
        // environment — the same constructor-resolution shape as the
        // generic flutter / terraform / act cleaners.
        let cache_dir = if std::env::var("PRE_COMMIT_HOME").is_ok() {
            crate::safety::resolve_env_dir(
                "PRE_COMMIT_HOME",
                "",
                home.join(".cache/pre-commit"),
                "pre-commit",
            )
        } else {
            crate::safety::resolve_env_dir(
                "XDG_CACHE_HOME",
                "pre-commit",
                home.join(".cache/pre-commit"),
                "pre-commit",
            )
        };
        Self(crate::cli_fallback::CliFallbackCleaner::with_parts(
            "pre-commit",
            config(),
            cache_dir,
            runner,
        ))
    }

    /// Test-only factory: builds the cleaner with an explicitly-resolved
    /// cache dir, so unit tests never depend on the process environment.
    #[cfg(test)]
    fn with_cache_dir(cache_dir: std::path::PathBuf, runner: Box<dyn CommandRunner>) -> Self {
        Self(crate::cli_fallback::CliFallbackCleaner::with_parts(
            "pre-commit",
            config(),
            cache_dir,
            runner,
        ))
    }

    #[cfg(test)]
    pub(crate) fn cache_dir(&self) -> std::path::PathBuf {
        self.0.cache_dir()
    }
}

impl Cleaner for PreCommitCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        self.0.risk()
    }

    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn detect(&self) -> ScanResult {
        self.0.detect()
    }

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        self.0.clean(dry_run, reporter, opts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::ScanStatus;
    use crate::command::{CommandRunner, SystemCommandRunner};
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
        let cleaner = PreCommitCleaner::with_cache_dir(
            tmp.path().join(".cache/pre-commit"),
            Box::new(SystemCommandRunner),
        );
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

        let cleaner =
            PreCommitCleaner::with_cache_dir(cache.clone(), Box::new(SystemCommandRunner));
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

        let cleaner =
            PreCommitCleaner::with_cache_dir(cache.clone(), Box::new(SystemCommandRunner));
        cleaner
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
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

        let cleaner = PreCommitCleaner::with_cache_dir(cache.clone(), Box::new(NoToolRunner));
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
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

        let cleaner = PreCommitCleaner::with_cache_dir(cache.clone(), Box::new(CliToolRunner));
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert!(result.bytes_freed > 0, "CLI path should report freed bytes");
    }

    #[test]
    fn clean_dry_run_uses_cli_when_tool_available() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let cache = tmp.path().join(".cache/pre-commit");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

        let cleaner = PreCommitCleaner::with_cache_dir(cache.clone(), Box::new(CliToolRunner));
        let result = cleaner
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
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

        let cleaner =
            PreCommitCleaner::with_cache_dir(cache.clone(), Box::new(CliToolRunnerFailing));
        let result = cleaner.clean(false, &reporter, &crate::cleaner::CleanOptions::default());
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
        // The unsafe-override fallback (PRE_COMMIT_HOME=/ is rejected BEFORE
        // any subpath is joined) is pinned at the resolver level in
        // src/safety.rs. The resolved value for that case is injected
        // explicitly here: cache_dir() must return it unchanged, with no
        // process-env dependency in the test.
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join(".cache/pre-commit");
        let cleaner =
            PreCommitCleaner::with_cache_dir(default.clone(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            default,
            "unsafe PRE_COMMIT_HOME=/ should fall back to default"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_xdg_cache_is_unsafe() {
        // The set-but-empty PRE_COMMIT_HOME branch and the XDG-override
        // fallback (an empty PRE_COMMIT_HOME is unsafe at the resolver, and
        // XDG_CACHE_HOME=/etc is rejected BEFORE "pre-commit" is joined) are
        // pinned at the resolver level in src/safety.rs, and the production
        // constructor keeps the `PRE_COMMIT_HOME` set (even empty) branch.
        // The resolved value for that case is injected explicitly here.
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join(".cache/pre-commit");
        let cleaner =
            PreCommitCleaner::with_cache_dir(default.clone(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            default,
            "unsafe XDG_CACHE_HOME=/etc should fall back to default"
        );
    }

    #[test]
    fn detect_uses_xdg_cache_home_unconditionally() {
        // The resolved value for the XDG-override case is
        // $XDG_CACHE_HOME/pre-commit; it is injected explicitly here. The
        // cleaner must look in that resolved dir and find NotFound (not fall
        // back to the default ~/.cache/pre-commit).
        let tmp = TempDir::new().unwrap();
        let xdg_cache = tmp.path().join("xdg-cache");
        let cleaner = PreCommitCleaner::with_cache_dir(
            xdg_cache.join("pre-commit"),
            Box::new(SystemCommandRunner),
        );
        let result = cleaner.detect();

        assert!(matches!(result.status, ScanStatus::NotFound));
    }
}
