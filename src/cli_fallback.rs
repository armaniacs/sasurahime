//! The "CLI tool first, fallback delete" adapter (PBI 57).
//!
//! huggingface and pre-commit differ only in their cache-dir resolution and
//! their [`CliFallbackConfig`] values, so the detect/clean skeleton lives
//! once here and the wrappers own their constructors. Extracted from the
//! cleaner-layer `generic.rs` so the adapter lives with the other
//! structural helpers instead of inside the 2000-line generic module.

use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
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
        reporter: &dyn ProgressReporter,
        opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        clean_cli_or_fallback(
            self.name(),
            &self.cache_dir(),
            &*self.runner,
            &self.config,
            dry_run,
            opts.trash_mode,
            reporter,
        )
    }
}

/// Run a CLI tool to clean a cache directory, falling back to direct deletion
/// when the tool is not installed.
///
/// This eliminates the duplicated "CLI first → fallback" pattern between
/// `huggingface.rs` and `pre_commit.rs`. The fallback delete rides the
/// delete ledger, which owns the per-entry contracts (progress, skip
/// records, freed sums, CleanResult assembly); the recreate is ledger
/// post-processing so a recreate failure can never masquerade as a skip
/// record for an entry that was already deleted.
pub fn clean_cli_or_fallback(
    name: &'static str,
    dir: &Path,
    runner: &dyn CommandRunner,
    config: &CliFallbackConfig,
    dry_run: bool,
    mode: crate::trash::TrashMode,
    reporter: &dyn ProgressReporter,
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
    // There is no caller-side unsafe-path guard here anymore: the seam's
    // scope tripwire refuses the same paths at deletion time, and the ledger
    // records the refusal as a skipped entry (prose reason) — the skip
    // channel and the deleted-path audit trail stay consistent with every
    // other ledger user.
    let result =
        crate::trash::delete_entries(name, runner, reporter, mode, &[(dir.to_path_buf(), size)]);
    // Recreate is ledger post-processing, never closure work: it only runs
    // when every entry deleted, so a recreate failure propagates as an Err
    // instead of masquerading as a skip record for a deleted entry.
    if config.recreate && result.skipped.is_empty() {
        fs::create_dir_all(dir)?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ledger migration (PBI 77) ──

    /// A reporter that records the progress calls, so the threading of the
    /// reporter into the delete ledger is observable.
    struct RecordingReporter {
        inits: std::sync::Mutex<Vec<(String, usize)>>,
        ticks: std::sync::Mutex<Vec<(PathBuf, usize, u64)>>,
    }

    impl Default for RecordingReporter {
        fn default() -> Self {
            Self {
                inits: std::sync::Mutex::new(Vec::new()),
                ticks: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl crate::progress::ProgressReporter for RecordingReporter {
        fn show_spinner(&self) -> bool {
            false
        }
        fn progress_init(&self, label: &str, total: usize) {
            self.inits.lock().unwrap().push((label.to_string(), total));
        }
        fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64) {
            self.ticks
                .lock()
                .unwrap()
                .push((path.to_path_buf(), current, size_bytes));
        }
        fn progress_finish(&self) {}
    }

    /// A runner that reports no tools exist (forcing the fallback delete)
    /// and permits `chflags` for the seam's permanent-mode clear, which the
    /// system performs.
    struct FallbackNoToolRunner;
    impl CommandRunner for FallbackNoToolRunner {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            assert_eq!(program, "chflags", "unexpected program {program}");
            std::process::Command::new("chflags")
                .args(args)
                .output()
                .map_err(|e| anyhow::anyhow!("chflags failed: {e}"))
        }
        fn exists(&self, program: &str) -> bool {
            program == "chflags"
        }
    }

    #[test]
    fn clean_threads_the_reporter_into_the_ledger() {
        // The fallback delete rides the delete ledger: progress_init carries
        // the label and items.len() (PBI 68) and the deletion ticks. Before
        // the ledger migration the reporter had no path into the fallback
        // branch, so the progress tick was structurally absent.
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = CliFallbackCleaner::with_parts(
            "test-fallback",
            CliFallbackConfig {
                tool: "definitely-not-installed-tool",
                args: &[],
                recreate: true,
            },
            tmp.path().to_path_buf(),
            Box::new(|h: &Path| h.join("cache")),
            Box::new(FallbackNoToolRunner),
        );
        let reporter = RecordingReporter::default();
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
        let inits = reporter.inits.lock().unwrap();
        assert_eq!(
            *inits,
            vec![("test-fallback".to_string(), 1)],
            "progress_init runs once with the label and items.len()"
        );
        let ticks = reporter.ticks.lock().unwrap();
        assert_eq!(ticks.len(), 1, "one tick per deleted entry: {ticks:?}");
        assert_eq!(ticks[0].0, dir);
        assert!(result.bytes_freed > 0, "the deletion freed bytes");
        assert!(dir.exists(), "recreate=true must recreate the cache dir");
        assert!(!dir.join("data.bin").exists(), "the contents must be gone");
    }

    #[test]
    fn clean_cli_or_fallback_records_an_unsafe_dir_as_a_skipped_entry() {
        // The clean-time unsafe-path pre-check warned without recording: the
        // skip channel and the exit code could not see the refusal. The
        // seam's scope tripwire refuses the same path at deletion time and
        // the ledger records it as a SkippedEntry (prose reason) — same as
        // every other ledger user. A tempdir under /tmp never passes
        // is_safe_delete_target (the canonicalized path starts with
        // /private/tmp).
        let tmp = tempfile::TempDir::new_in("/tmp").unwrap();
        std::fs::write(tmp.path().join("keep.txt"), b"data").unwrap();
        let config = CliFallbackConfig {
            tool: "definitely-not-installed-tool",
            args: &[],
            recreate: false,
        };
        let result = clean_cli_or_fallback(
            "test-fallback",
            tmp.path(),
            &MissingToolRunner,
            &config,
            false,
            crate::trash::TrashMode::Permanent,
            &crate::progress::DeepSuppressReporter,
        )
        .unwrap();
        assert_eq!(result.bytes_freed, 0, "a refused dir frees nothing");
        assert!(
            tmp.path().join("keep.txt").exists(),
            "the unsafe dir must survive"
        );
        assert_eq!(
            result.skipped.len(),
            1,
            "the refusal must be recorded as a SkippedEntry: {:?}",
            result.skipped
        );
        assert_eq!(result.skipped[0].path, tmp.path());
        assert!(
            matches!(&result.skipped[0].reason, crate::cleaner::SkipCause::Reason(reason) if reason.contains("refusing to delete unsafe path")),
            "the seam refusal rides as prose: {:?}",
            result.skipped[0].reason
        );
        assert_eq!(
            result.exit_code(),
            1,
            "the refusal must surface through the exit code"
        );
    }

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
            &crate::progress::DeepSuppressReporter,
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
            &crate::progress::DeepSuppressReporter,
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
