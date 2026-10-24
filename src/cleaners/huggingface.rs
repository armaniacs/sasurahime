use crate::cleaner::{CleanResult, Cleaner, ScanResult};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::Path;

/// Thin wrapper over [`generic::CliFallbackCleaner`]: this cleaner differs
/// from the shared adapter only in its cache-dir resolution (HF_HOME) and its
/// CLI tool configuration, so the detect/clean skeleton lives in the adapter.
pub struct HuggingFaceCleaner(super::generic::CliFallbackCleaner);

impl HuggingFaceCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self(super::generic::CliFallbackCleaner::with_parts(
            "huggingface",
            super::generic::CliFallbackConfig {
                tool: "huggingface-cli",
                args: &["delete-cache", "--yes"],
                recreate: true,
            },
            home.to_path_buf(),
            Box::new(|h: &Path| {
                // The HF_HOME value itself is validated BEFORE "hub" is joined, so a
                // root-adjacent value (e.g. HF_HOME=/) falls back to the default
                // instead of producing a joined path that passes the safety check.
                super::generic::resolve_env_dir(
                    "HF_HOME",
                    "hub",
                    h.join(".cache/huggingface/hub"),
                    "huggingface",
                )
            }),
            runner,
        ))
    }

    #[cfg(test)]
    pub(crate) fn cache_dir(&self) -> std::path::PathBuf {
        self.0.cache_dir()
    }
}

impl Cleaner for HuggingFaceCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        self.0.risk()
    }

    fn name(&self) -> &'static str {
        self.0.name()
    }

    fn detect(&self) -> ScanResult {
        self.0.detect()
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        self.0.clean(dry_run, reporter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::ScanStatus;
    use crate::cleaners::generic::resolve_env_dir;
    use crate::command::{CommandRunner, SystemCommandRunner};
    use crate::test_helpers::EnvGuard;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Output;
    use tempfile::TempDir;

    /// A runner that reports no external cache-cleaning tools exist, forcing the
    /// fallback path. Permits `chflags` since the cleaner always calls it before
    /// direct deletion.
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

    /// A runner that reports huggingface-cli as available (simulates CLI path).
    struct CliToolRunner;

    impl CommandRunner for CliToolRunner {
        fn run(&self, program: &str, _args: &[&str]) -> Result<Output> {
            assert_eq!(
                program, "huggingface-cli",
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
            matches!(program, "huggingface-cli" | "chflags")
        }
    }

    struct CliToolRunnerFailing;

    impl CommandRunner for CliToolRunnerFailing {
        fn run(&self, program: &str, _args: &[&str]) -> Result<Output> {
            assert_eq!(
                program, "huggingface-cli",
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
            matches!(program, "huggingface-cli" | "chflags")
        }
    }

    #[test]
    fn detect_returns_not_found_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_returns_pruneable_when_cache_exists() {
        let tmp = TempDir::new().unwrap();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn clean_dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(NoToolRunner));
        cleaner.clean(true, &reporter).unwrap();
        assert!(hub.exists(), "dry-run must not delete");
        assert!(
            hub.join("model.bin").exists(),
            "dry-run must not delete files"
        );
    }

    #[test]
    fn clean_fallback_deletes_dir() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(NoToolRunner));
        let result = cleaner.clean(false, &reporter).unwrap();
        assert!(result.bytes_freed > 0);
        // hub/ should be recreated (empty)
        assert!(hub.exists(), "hub/ should be recreated");
        assert!(
            !hub.join("model.bin").exists(),
            "contents should be removed"
        );
    }

    #[test]
    fn clean_uses_cli_when_tool_available() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(CliToolRunner));
        let result = cleaner.clean(false, &reporter).unwrap();
        assert!(result.bytes_freed > 0, "CLI path should report freed bytes");
    }

    #[test]
    fn clean_dry_run_uses_cli_when_tool_available() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(CliToolRunner));
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0, "dry-run must report 0 freed");
        assert!(
            hub.join("model.bin").exists(),
            "dry-run must not delete files"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_hf_home_is_unsafe() {
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join(".cache/huggingface/hub");
        // HF_HOME must still hold the unsafe value while cache_dir() runs:
        // the guard is alive until the end of the test, so the restore happens
        // after the call and the assertion.
        let _guard = EnvGuard::set("HF_HOME", "/etc");
        // The resolver validates the HF_HOME value itself BEFORE "hub" is
        // joined, so /etc/hub is never even constructed.
        assert_eq!(
            resolve_env_dir("HF_HOME", "hub", default.clone(), "huggingface"),
            default,
            "unsafe HF_HOME=/etc must fall back to the default at the resolver"
        );
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            default,
            "unsafe HF_HOME=/etc should fall back to default"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_hf_home_is_tmp() {
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join(".cache/huggingface/hub");
        // HF_HOME must still hold the unsafe value while cache_dir() runs:
        // the guard is alive until the end of the test, so the restore happens
        // after the call and the assertion.
        let _guard = EnvGuard::set("HF_HOME", "/tmp");
        // The resolver validates the HF_HOME value itself BEFORE "hub" is
        // joined.
        assert_eq!(
            resolve_env_dir("HF_HOME", "hub", default.clone(), "huggingface"),
            default,
            "unsafe HF_HOME=/tmp must fall back to the default at the resolver"
        );
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            default,
            "unsafe HF_HOME=/tmp should fall back to default"
        );
    }

    #[test]
    fn cache_dir_returns_joined_path_when_hf_home_is_safe() {
        // A normal HF_HOME is unaffected: the override value is safe, so the
        // subpath is joined and the joined path is returned unchanged.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let _guard = EnvGuard::set("HF_HOME", "/Users/u/hf");
        assert_eq!(
            resolve_env_dir(
                "HF_HOME",
                "hub",
                tmp.path().join(".cache/huggingface/hub"),
                "huggingface"
            ),
            PathBuf::from("/Users/u/hf/hub"),
            "a safe HF_HOME must be used with the subpath joined"
        );
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            PathBuf::from("/Users/u/hf/hub"),
            "a safe HF_HOME must not fall back to the default"
        );
    }

    #[test]
    fn cache_dir_falls_back_when_hf_home_is_root() {
        // THE FIX (PBI 19): cache_dir() used to join "hub" BEFORE validating,
        // so HF_HOME=/ produced /hub, which passed is_safe_delete_target and
        // made a root-adjacent directory the delete target. The HF_HOME value
        // itself must be validated BEFORE joining: HF_HOME=/ is unsafe, so
        // cache_dir() must fall back to the default instead of /hub.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let _guard = EnvGuard::set("HF_HOME", "/");
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let dir = cleaner.cache_dir();
        assert_eq!(
            dir,
            tmp.path().join(".cache/huggingface/hub"),
            "HF_HOME=/ must fall back to the default, not the root-adjacent /hub"
        );
    }

    #[test]
    fn clean_returns_error_when_cli_fails() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(CliToolRunnerFailing));
        let result = cleaner.clean(false, &reporter);
        assert!(result.is_err(), "CLI failure should propagate as error");
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("exit code") || msg.contains("1"),
            "error should mention exit code: {msg}"
        );
    }
}
