//! The "CLI tool first, fallback delete" adapter (PBI 57).
//!
//! huggingface and pre-commit differ only in their cache-dir resolution and
//! their [`CliFallbackConfig`] values, so the detect/clean skeleton lives
//! once here and the wrappers own their constructors. Extracted from the
//! cleaner-layer `generic.rs` so the adapter lives with the other
//! structural helpers instead of inside the 2000-line generic module.

use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkipCause, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use crate::safety::is_safe_delete_target;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

/// Configuration for [`clean_cli_or_fallback`].
pub struct CliFallbackConfig<'a> {
    pub tool: &'a str,
    pub args: &'a [&'a str],
    /// Whether to recreate the cache directory after fallback deletion.
    pub recreate: bool,
}

/// Shared adapter for cleaners that run a CLI tool with a fallback delete.
///
/// huggingface and pre-commit differ only in their cache-dir resolution and
/// their [`CliFallbackConfig`] values; the detect/clean skeleton used to be
/// duplicated per module. The wrappers own their constructors and delegate
/// here, so the skeleton lives once.
pub struct CliFallbackCleaner {
    name: &'static str,
    config: CliFallbackConfig<'static>,
    home: PathBuf,
    resolve_cache_dir: Box<dyn Fn(&Path) -> PathBuf + Send + Sync>,
    runner: Box<dyn CommandRunner>,
}

impl CliFallbackCleaner {
    pub(crate) fn with_parts(
        name: &'static str,
        config: CliFallbackConfig<'static>,
        home: PathBuf,
        resolve_cache_dir: Box<dyn Fn(&Path) -> PathBuf + Send + Sync>,
        runner: Box<dyn CommandRunner>,
    ) -> Self {
        Self {
            name,
            config,
            home,
            resolve_cache_dir,
            runner,
        }
    }

    pub(crate) fn cache_dir(&self) -> PathBuf {
        (self.resolve_cache_dir)(&self.home)
    }
}

impl Cleaner for CliFallbackCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Safe
    }

    fn name(&self) -> &'static str {
        self.name
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
            r = r.with_target(dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(
        &self,
        dry_run: bool,
        _reporter: &dyn ProgressReporter,
        opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        clean_cli_or_fallback(
            self.name(),
            &self.cache_dir(),
            &*self.runner,
            &self.config,
            dry_run,
            opts.trash_mode,
        )
    }
}

/// Run a CLI tool to clean a cache directory, falling back to direct deletion
/// when the tool is not installed.
///
/// This eliminates the duplicated "CLI first → fallback" pattern between
/// `huggingface.rs` and `pre_commit.rs`.
pub fn clean_cli_or_fallback(
    name: &'static str,
    dir: &Path,
    runner: &dyn CommandRunner,
    config: &CliFallbackConfig,
    dry_run: bool,
    mode: crate::trash::TrashMode,
) -> Result<CleanResult> {
    if !dir.exists() {
        return Ok(CleanResult::empty(name));
    }

    // Prefer CLI if available
    if runner.exists(config.tool) {
        if dry_run {
            println!(
                "[dry-run] [{name}] would run: {} {}",
                config.tool,
                config.args.join(" ")
            );
            let size = dir_size(dir);
            println!(
                "[dry-run] [{name}] would free: {}",
                crate::format::format_bytes(size)
            );
            return Ok(CleanResult::empty(name));
        }
        let size_before = dir_size(dir);
        let output = runner.run(config.tool, config.args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!(
                "{} failed (exit {:?}): {stderr}",
                config.tool,
                output.status.code()
            );
            anyhow::bail!(
                "{} failed with exit code {:?}",
                config.tool,
                output.status.code()
            );
        }
        println!("[{name}] ran {} {}", config.tool, config.args.join(" "));
        return Ok(CleanResult::freed(name, size_before, false));
    }

    // Fallback: delete directly
    let size = dir_size(dir);
    if dry_run {
        println!(
            "[dry-run] [{name}] would remove: {} ({})",
            dir.display(),
            crate::format::format_bytes(size)
        );
        return Ok(CleanResult::empty(name));
    }
    // Clean-time safety re-validation, same as the other deletion sinks:
    // env-var-derived dirs are only checked at construction. This is
    // validate-then-use (not an atomic guarantee): a concurrent swap in the
    // parent dir between this check and the delete remains possible.
    if !is_safe_delete_target(dir) {
        eprintln!("[{name}] warning: unsafe path, skipping: {}", dir.display());
        return Ok(CleanResult::empty(name));
    }

    let outcome = crate::trash::safe_delete(runner, dir, mode)?;
    if let Some(reason) = outcome.skip_reason {
        // The seam classified the failure as skippable: the entry was NOT
        // deleted, so nothing is in the Trash and the run must not claim
        // Trash. Only successful deletions contribute outcome.uses_trash
        // (PBI 43 seam-truth convention).
        return Ok(CleanResult {
            name,
            bytes_freed: 0,
            uses_trash: false,
            skipped: vec![SkippedEntry {
                path: dir.to_path_buf(),
                reason: SkipCause::Delete(reason),
            }],
            deleted_paths: vec![],
        });
    }
    if config.recreate {
        fs::create_dir_all(dir)?;
    }
    println!("[{name}] removed cache: {}", dir.display());
    Ok(CleanResult::freed(name, size, outcome.uses_trash))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A runner whose `run` must never be reached: the configured tool does
    /// not exist, so the fallback delete runs instead.
    struct MissingToolRunner;
    impl CommandRunner for MissingToolRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            unreachable!()
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }

    #[test]
    fn clean_cli_or_fallback_revalidates_safety_at_clean_time() {
        // A dir that is unsafe at clean time (here: HOME itself) must be
        // skipped by the fallback delete, not removed. TEST_LOCK serializes
        // the process-global HOME mutation against the other HOME-scoped
        // tests.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        std::fs::write(home.join("keep.txt"), b"data").unwrap();
        let config = CliFallbackConfig {
            tool: "definitely-not-installed-tool",
            args: &[],
            recreate: false,
        };
        let result = clean_cli_or_fallback(
            "test-fallback",
            &home,
            &crate::command::SystemCommandRunner,
            &config,
            false,
            crate::trash::TrashMode::Permanent,
        )
        .unwrap();
        assert_eq!(
            result.bytes_freed, 0,
            "an unsafe dir must not be deleted by the fallback"
        );
        assert!(
            home.join("keep.txt").exists(),
            "the unsafe dir must survive"
        );
    }

    #[test]
    fn clean_cli_or_fallback_seam_skip_never_claims_trash() {
        // PBI 43 follow-up: clean_cli_or_fallback's seam-skip return still
        // propagated outcome.uses_trash, so a trash-mode failure (entry NOT
        // deleted) claimed Trash for a zero-deletion run. The refusal comes
        // from the test-only backend override: the platform backends move
        // entries out of read-only parents on modern macOS, so a chmod alone
        // no longer forces the deterministic failure. The pinned Trash mode
        // keeps the claim out of the global pin.
        let _backend = crate::trash::TrashBackendOverrideGuard::set();
        let tmp = tempfile::TempDir::new().unwrap();
        let parent = tmp.path().join("locked");
        let dir = parent.join("cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let config = CliFallbackConfig {
            tool: "definitely-not-installed-tool",
            args: &[],
            recreate: false,
        };
        let result = clean_cli_or_fallback(
            "test-fallback",
            &dir,
            &MissingToolRunner,
            &config,
            false,
            crate::trash::TrashMode::Trash,
        )
        .unwrap();
        assert!(dir.exists(), "a seam-skipped entry must survive");
        assert_eq!(
            result.skipped.len(),
            1,
            "the seam-classified failure must be recorded as a SkippedEntry"
        );
        // Typed classification (PBI 59): the seam's skip carries the variant,
        // not prose, so a reworded message cannot flip the skip.
        assert!(
            matches!(
                &result.skipped[0].reason,
                crate::cleaner::SkipCause::Delete(crate::cleaner::DeleteError::TrashRefused(_))
            ),
            "the skip must carry the typed TrashRefused classification: {:?}",
            result.skipped[0].reason
        );
        assert!(
            !result.uses_trash,
            "a run where nothing was deleted must not claim Trash"
        );
        assert_eq!(result.bytes_freed, 0, "nothing was deleted");
    }
}
