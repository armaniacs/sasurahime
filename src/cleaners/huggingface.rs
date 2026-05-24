use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct HuggingFaceCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl HuggingFaceCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
    }

    fn cache_dir(&self) -> PathBuf {
        match std::env::var("HF_HOME") {
            Ok(h) => {
                let p = PathBuf::from(&h).join("hub");
                if !super::generic::is_safe_delete_target(&p) {
                    eprintln!(
                        "[huggingface] WARNING: HF_HOME={} points to an unsafe path, using default",
                        h
                    );
                    self.home.join(".cache/huggingface/hub")
                } else {
                    p
                }
            }
            Err(_) => self.home.join(".cache/huggingface/hub"),
        }
    }
}

impl Cleaner for HuggingFaceCleaner {
    fn name(&self) -> &'static str {
        "huggingface"
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
            tool: "huggingface-cli",
            args: &["delete-cache", "--yes"],
            recreate: true,
        };
        super::generic::clean_cli_or_fallback(self.name(), &dir, &*self.runner, &config, dry_run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandRunner, SystemCommandRunner};
    use std::fs;
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
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("HF_HOME").ok();
        std::env::set_var("HF_HOME", "/");
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        match prev {
            Some(v) => std::env::set_var("HF_HOME", v),
            None => std::env::remove_var("HF_HOME"),
        }
        let dir = cleaner.cache_dir();
        assert_eq!(
            dir,
            tmp.path().join(".cache/huggingface/hub"),
            "unsafe HF_HOME=/ should fall back to default"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_hf_home_is_tmp() {
        let tmp = TempDir::new().unwrap();
        let prev = std::env::var("HF_HOME").ok();
        std::env::set_var("HF_HOME", "/tmp");
        let cleaner = HuggingFaceCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        match prev {
            Some(v) => std::env::set_var("HF_HOME", v),
            None => std::env::remove_var("HF_HOME"),
        }
        let dir = cleaner.cache_dir();
        assert_eq!(
            dir,
            tmp.path().join(".cache/huggingface/hub"),
            "unsafe HF_HOME=/tmp should fall back to default"
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
