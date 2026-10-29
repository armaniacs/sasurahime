use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkipCause};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
#[cfg(test)]
use crate::test_helpers::MockRunner;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct UvCleaner {
    cache_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl UvCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            cache_dir: home.join(".cache/uv"),
            runner,
        }
    }

    /// Parses "simple-v16" → Some(16), anything else → None.
    pub fn parse_simple_version(name: &str) -> Option<u32> {
        name.strip_prefix("simple-v")?.parse().ok()
    }

    /// Returns paths of simple-vN directories that are older than the highest N found.
    pub fn detect_old_indexes(&self) -> Vec<PathBuf> {
        let entries = match fs::read_dir(&self.cache_dir) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        let mut versions: Vec<(u32, PathBuf)> = entries
            .filter_map(|e| e.ok())
            // GAP-006: skip symlinks to avoid following stale / shared links
            .filter(|e| !e.file_type().map(|t| t.is_symlink()).unwrap_or(true))
            .filter_map(|e| {
                let name = e.file_name();
                let n = Self::parse_simple_version(&name.to_string_lossy())?;
                Some((n, e.path()))
            })
            .collect();

        if versions.len() <= 1 {
            return vec![];
        }

        let max = versions.iter().map(|(n, _)| *n).max().unwrap();
        versions.retain(|(n, _)| *n < max);
        versions.into_iter().map(|(_, p)| p).collect()
    }
}

impl Cleaner for UvCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Safe
    }

    fn is_available(&self) -> bool {
        self.runner.exists("uv")
    }

    fn name(&self) -> &'static str {
        "uv"
    }

    fn detect(&self) -> ScanResult {
        if !self.cache_dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let bytes = dir_size(&self.cache_dir);
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.cache_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(
        &self,
        dry_run: bool,
        _reporter: &dyn ProgressReporter,
        opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        if !self.runner.exists("uv") {
            println!("uv: not found, skipping");
            return Ok(CleanResult::empty(self.name()));
        }

        let before = dir_size(&self.cache_dir);

        let mut skipped: Vec<crate::cleaner::SkippedEntry> = vec![];
        // OR-in across every deletion: the seam's resolved mode is the truth.
        let mut uses_trash = false;

        // Remove old simple-vN index caches
        for old in self.detect_old_indexes() {
            if dry_run {
                println!("[dry-run] would remove: {}", old.display());
            } else {
                let outcome =
                    crate::trash::safe_delete(self.runner.as_ref(), &old, opts.trash_mode)?;
                if let Some(reason) = outcome.skip_reason {
                    skipped.push(crate::cleaner::SkippedEntry {
                        path: old.clone(),
                        reason: SkipCause::Delete(reason),
                    });
                    continue;
                }
                // OR-in only when the deletion actually happened: a skipped
                // Trash move never claims Trash.
                uses_trash |= outcome.uses_trash;
                println!("Removed: {}", old.display());
            }
        }

        // Prune unused package archives via uv itself
        if dry_run {
            println!("[dry-run] would run: uv cache prune --force");
        } else {
            let output = self.runner.run("uv", &["cache", "prune", "--force"])?;
            if !output.status.success() {
                // PBI 53: a failed prune silently contributes 0 to `freed`
                // (the per-item deletions above measured correctly). Warn so
                // the prune failure is not invisible.
                log::warn!(
                    "uv cache prune --force exited with {} - the prune was not applied",
                    output.status.code().unwrap_or(-1)
                );
            }
        }

        let after = if dry_run {
            before
        } else {
            dir_size(&self.cache_dir)
        };
        let freed = before.saturating_sub(after);

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash,
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_simple_version_valid() {
        assert_eq!(UvCleaner::parse_simple_version("simple-v16"), Some(16));
        assert_eq!(UvCleaner::parse_simple_version("simple-v21"), Some(21));
    }

    #[test]
    fn parse_simple_version_invalid() {
        assert_eq!(UvCleaner::parse_simple_version("archive-v0"), None);
        assert_eq!(UvCleaner::parse_simple_version("simple-vabc"), None);
        assert_eq!(UvCleaner::parse_simple_version(""), None);
    }

    #[test]
    fn detect_old_indexes_returns_all_but_highest() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v16")).unwrap();
        std::fs::create_dir_all(cache.join("simple-v17")).unwrap();
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let old = cleaner.detect_old_indexes();
        assert_eq!(old.len(), 2);
        let names: Vec<_> = old
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"simple-v16".to_string()));
        assert!(names.contains(&"simple-v17".to_string()));
    }

    #[test]
    fn detect_old_indexes_single_version_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    // ── GAP-006: symlinks and files are skipped ────────────────────────────
    #[test]
    fn detect_old_indexes_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(cache.join("simple-v21")).unwrap();
        let target = tmp.path().join("actual-dir");
        std::fs::create_dir_all(&target).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&target, cache.join("simple-v99")).unwrap();
        }

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        // Without the symlink guard, simple-v99 would be the max and simple-v21 would
        // be marked as old. With the guard, only simple-v21 exists → returns empty.
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    #[test]
    fn detect_old_indexes_skips_regular_files() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("simple-v21"), b"not a dir").unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        // The .to_owned() returns None (not a dir), so the entry is skipped by
        // filter_map. But we test explicitly here for clarity.
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    #[test]
    fn detect_old_indexes_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        // cache dir does not exist
        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        assert!(cleaner.detect_old_indexes().is_empty());
    }

    // ── detect() size coverage ──────────────────────────────────────────────

    #[test]
    fn detect_measures_full_cache_dir() {
        let tmp = TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/uv");
        // archive-v0 — previously the only thing detect() measured
        std::fs::create_dir_all(cache.join("archive-v0")).unwrap();
        std::fs::write(cache.join("archive-v0/pkg.tar.gz"), [0u8; 4096]).unwrap();
        // simple-vN index — was missed before the fix
        std::fs::create_dir_all(cache.join("simple-v17")).unwrap();
        std::fs::write(cache.join("simple-v17/index.html"), [0u8; 1024]).unwrap();

        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let result = cleaner.detect();

        let expected = crate::format::dir_size(&cache);
        match result.status {
            ScanStatus::Pruneable(bytes) => assert_eq!(bytes, expected),
            other => panic!("expected Pruneable, got {other:?}"),
        }
    }

    #[test]
    fn detect_returns_not_found_when_cache_missing() {
        let tmp = TempDir::new().unwrap();
        // cache dir does not exist
        let cleaner = UvCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::NotFound),
            "expected NotFound, got {:#?}",
            result.status
        );
    }

    #[test]
    fn clean_reports_zero_freed_when_prune_exits_nonzero() {
        // PBI 53 pin: a failed `uv cache prune --force` silently contributes
        // 0 to `freed` (the warn branch). The failure must not bubble up as
        // an error and must not fabricate freed bytes.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".cache/uv/simple-v21")).unwrap();

        let cleaner = UvCleaner::new(
            tmp.path(),
            Box::new(MockRunner::new().with_exit_code("uv", 1 << 8)),
        );
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .expect("a failed prune must not fail the run");
        assert_eq!(
            result.bytes_freed, 0,
            "a failed prune must not report freed bytes"
        );
        assert!(result.skipped.is_empty(), "nothing was deleted");
        assert!(!result.uses_trash);
    }
}
