use crate::cleaner::{CleanResult, Cleaner, ScanResult};
use crate::command::CommandRunner;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::Path;

/// Thin wrapper over [`crate::cli_fallback::CliFallbackCleaner`]: this cleaner differs
/// from the shared adapter only in the cache dir it resolves at construction
/// (HF_HOME) and its CLI tool configuration, so the detect/clean skeleton
/// lives in the adapter.
pub struct HuggingFaceCleaner(crate::cli_fallback::CliFallbackCleaner);

fn config() -> crate::cli_fallback::CliFallbackConfig<'static> {
    crate::cli_fallback::CliFallbackConfig {
        tool: "huggingface-cli",
        args: &["delete-cache", "--yes"],
        recreate: true,
    }
}

impl HuggingFaceCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        // The HF_HOME value itself is validated BEFORE "hub" is joined, so a
        // root-adjacent value (e.g. HF_HOME=/) falls back to the default
        // instead of producing a joined path that passes the safety check.
        // Resolved ONCE at construction (PBI 88): the resolved value is
        // pinned on the cleaner and detect/clean never re-read the process
        // environment — the same constructor-resolution shape as the
        // generic flutter / terraform / act cleaners.
        let cache_dir = crate::safety::resolve_env_dir(
            "HF_HOME",
            "hub",
            home.join(".cache/huggingface/hub"),
            "huggingface",
        );
        Self(crate::cli_fallback::CliFallbackCleaner::with_parts(
            "huggingface",
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
            "huggingface",
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
    use crate::test_helpers::EnvGuard;
    use std::fs;
    use std::path::{Path, PathBuf};
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

    /// A reporter that records the deletion ticks, so the data-loss hazard
    /// pin can observe the delete target.
    struct RecordingReporter {
        ticks: std::sync::Mutex<Vec<PathBuf>>,
    }

    impl Default for RecordingReporter {
        fn default() -> Self {
            Self {
                ticks: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl crate::progress::ProgressReporter for RecordingReporter {
        fn show_spinner(&self) -> bool {
            false
        }
        fn progress_init(&self, _label: &str, _total: usize) {}
        fn progress_tick(&self, path: &Path, _current: usize, _size_bytes: u64) {
            self.ticks.lock().unwrap().push(path.to_path_buf());
        }
        fn progress_finish(&self) {}
    }

    #[test]
    fn detect_returns_not_found_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        let cleaner = HuggingFaceCleaner::with_cache_dir(
            tmp.path().join(".cache/huggingface/hub"),
            Box::new(SystemCommandRunner),
        );
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_returns_pruneable_when_cache_exists() {
        let tmp = TempDir::new().unwrap();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner =
            HuggingFaceCleaner::with_cache_dir(hub.clone(), Box::new(SystemCommandRunner));
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

        let cleaner = HuggingFaceCleaner::with_cache_dir(hub.clone(), Box::new(NoToolRunner));
        cleaner
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
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

        let cleaner = HuggingFaceCleaner::with_cache_dir(hub.clone(), Box::new(NoToolRunner));
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
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

        let cleaner = HuggingFaceCleaner::with_cache_dir(hub.clone(), Box::new(CliToolRunner));
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert!(result.bytes_freed > 0, "CLI path should report freed bytes");
    }

    #[test]
    fn clean_dry_run_uses_cli_when_tool_available() {
        let tmp = TempDir::new().unwrap();
        let reporter = crate::progress::VerboseProgress::new();
        let hub = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&hub).unwrap();
        fs::write(hub.join("model.bin"), b"dummy").unwrap();

        let cleaner = HuggingFaceCleaner::with_cache_dir(hub.clone(), Box::new(CliToolRunner));
        let result = cleaner
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "dry-run must report 0 freed");
        assert!(
            hub.join("model.bin").exists(),
            "dry-run must not delete files"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_hf_home_is_unsafe() {
        // The unsafe-override fallback (HF_HOME=/etc is rejected BEFORE "hub"
        // is joined, so /etc/hub is never even constructed) is pinned at the
        // resolver level in src/safety.rs. The resolved value for that case
        // is injected explicitly here: cache_dir() must return it unchanged,
        // with no process-env dependency in the test.
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join(".cache/huggingface/hub");
        let cleaner =
            HuggingFaceCleaner::with_cache_dir(default.clone(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            default,
            "unsafe HF_HOME=/etc should fall back to default"
        );
    }

    #[test]
    fn cache_dir_uses_safe_default_when_hf_home_is_tmp() {
        // The unsafe-override fallback (HF_HOME=/tmp is rejected BEFORE "hub"
        // is joined) is pinned at the resolver level in src/safety.rs. The
        // resolved value for that case is injected explicitly here.
        let tmp = TempDir::new().unwrap();
        let default = tmp.path().join(".cache/huggingface/hub");
        let cleaner =
            HuggingFaceCleaner::with_cache_dir(default.clone(), Box::new(SystemCommandRunner));
        assert_eq!(
            cleaner.cache_dir(),
            default,
            "unsafe HF_HOME=/tmp should fall back to default"
        );
    }

    #[test]
    fn cache_dir_returns_joined_path_when_hf_home_is_safe() {
        // A safe HF_HOME resolves to the override joined with "hub" — the
        // override → joined-path resolver behavior is pinned at the resolver
        // level in src/safety.rs. The resolved value is injected explicitly
        // here: cache_dir() must return it unchanged.
        let cleaner = HuggingFaceCleaner::with_cache_dir(
            PathBuf::from("/Users/u/hf/hub"),
            Box::new(SystemCommandRunner),
        );
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
        // the resolved value is the default, never the root-adjacent /hub
        // (pinned at the resolver level in src/safety.rs). The resolved
        // value is injected explicitly here.
        let tmp = TempDir::new().unwrap();
        let cleaner = HuggingFaceCleaner::with_cache_dir(
            tmp.path().join(".cache/huggingface/hub"),
            Box::new(SystemCommandRunner),
        );
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

        let cleaner =
            HuggingFaceCleaner::with_cache_dir(hub.clone(), Box::new(CliToolRunnerFailing));
        let result = cleaner.clean(false, &reporter, &crate::cleaner::CleanOptions::default());
        assert!(result.is_err(), "CLI failure should propagate as error");
        let msg = format!("{:#}", result.unwrap_err());
        assert!(
            msg.contains("exit code") || msg.contains("1"),
            "error should mention exit code: {msg}"
        );
    }

    #[test]
    fn clean_deletes_only_the_injected_cache_dir_under_ambient_hf_home() {
        // data-loss hazard pin (PBI 88): the cleaner is built through the
        // test factory with an explicitly-injected cache dir, so the delete
        // target is the injected path even with an ambient HF_HOME pointing
        // at a real cache holding model files. Before PBI 88 the per-call
        // resolver closure re-read the process environment at clean time:
        // this exact setup (the recorded Red 1 observation) deleted the
        // ambient cache and left the fixture untouched. The ambient HF_HOME
        // is a tempdir stand-in for the developer's real cache; the guard's
        // survival window is minimal and no other test reads HF_HOME, so
        // parallel overlap is neutralized by the factory structure.
        let tmp = TempDir::new().unwrap();
        let injected = tmp.path().join(".cache/huggingface/hub");
        fs::create_dir_all(&injected).unwrap();
        fs::write(injected.join("fixture-model.bin"), b"fixture").unwrap();
        let ambient = TempDir::new().unwrap();
        let ambient_hub = ambient.path().join("hub");
        fs::create_dir_all(&ambient_hub).unwrap();
        let ambient_file = ambient_hub.join("real-model.bin");
        fs::write(&ambient_file, b"real weights").unwrap();

        let reporter = RecordingReporter::default();
        let cleaner = HuggingFaceCleaner::with_cache_dir(injected.clone(), Box::new(NoToolRunner));
        assert_eq!(
            cleaner.cache_dir(),
            injected,
            "the injected cache dir is the only delete target"
        );
        // The guard is alive only around the clean: if clean() ever re-read
        // the process environment, the delete target would flip to the
        // ambient cache and the tick assertions below would fail.
        let guard = EnvGuard::set("HF_HOME", ambient.path().to_string_lossy().as_ref());
        let result = cleaner
            .clean(
                false,
                &reporter,
                &crate::cleaner::CleanOptions {
                    trash_mode: crate::trash::TrashMode::Permanent,
                    ..Default::default()
                },
            )
            .unwrap();
        drop(guard);

        assert!(
            result.bytes_freed > 0,
            "the injected fixture cache was actually deleted"
        );
        assert!(injected.exists(), "injected hub/ recreated (recreate=true)");
        assert!(
            !injected.join("fixture-model.bin").exists(),
            "injected contents gone"
        );
        let ticks = reporter.ticks.lock().unwrap();
        assert!(!ticks.is_empty(), "the deletion was observed: {ticks:?}");
        for path in ticks.iter() {
            assert_eq!(
                path, &injected,
                "the delete target must be the injected cache dir, never the ambient one"
            );
        }
        // The ambient HF_HOME cache kept every file, byte-identical.
        assert!(
            ambient_file.exists(),
            "the ambient HF_HOME cache must keep its file"
        );
        assert_eq!(
            fs::read(&ambient_file).unwrap(),
            b"real weights",
            "the ambient cache file must be byte-identical"
        );
        assert!(
            ambient_hub.exists(),
            "the ambient cache dir must survive untouched"
        );
    }
}
