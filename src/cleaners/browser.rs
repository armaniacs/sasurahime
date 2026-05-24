use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

struct BrowserGroup {
    parent: PathBuf,
}

pub struct BrowserCleaner {
    groups: Vec<BrowserGroup>,
    // Held for interface consistency; browser detection is filesystem-only.
    _runner: Box<dyn CommandRunner>,
}

impl BrowserCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            groups: vec![
                BrowserGroup {
                    parent: home.join(".cache/puppeteer/chrome"),
                },
                BrowserGroup {
                    parent: home.join(".cache/puppeteer/chrome-headless-shell"),
                },
                BrowserGroup {
                    parent: home.join("Library/Caches/ms-playwright"),
                },
                BrowserGroup {
                    parent: home.join("Library/Caches/ms-playwright-go"),
                },
            ],
            _runner: runner,
        }
    }

    /// Converts a directory name to a sortable key of numeric components.
    ///
    /// Handles both `mac_arm-131.0.6778.204` and `chromium-1208`
    /// by collecting all runs of ASCII digits.
    pub fn version_key(name: &str) -> Vec<u32> {
        name.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    }

    /// Returns paths of all version subdirectories except the one with the highest key.
    /// Returns empty if directory is missing or contains zero or one entries.
    /// Symlinks and directories whose name contains no digits are both skipped.
    pub fn find_old_versions(parent: &Path) -> Vec<PathBuf> {
        let entries = match fs::read_dir(parent) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        let mut versions: Vec<(Vec<u32>, PathBuf)> = entries
            .filter_map(|e| e.ok())
            // Skip symlinks to avoid following stale / shared links
            .filter(|e| !e.file_type().map(|t| t.is_symlink()).unwrap_or(true))
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| {
                let key = Self::version_key(&e.file_name().to_string_lossy());
                (key, e.path())
            })
            .filter(|(k, _)| !k.is_empty()) // skip unparseable directory names
            .collect();

        if versions.len() <= 1 {
            return vec![];
        }

        let max = versions.iter().map(|(k, _)| k.clone()).max().unwrap();
        versions.retain(|(k, _)| *k != max);
        versions.into_iter().map(|(_, p)| p).collect()
    }
}

impl Cleaner for BrowserCleaner {
    fn name(&self) -> &'static str {
        "browsers"
    }

    fn detect(&self) -> ScanResult {
        let any_found = self.groups.iter().any(|g| g.parent.exists());
        if !any_found {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let bytes: u64 = self
            .groups
            .iter()
            .flat_map(|g| Self::find_old_versions(&g.parent))
            .map(|p| dir_size(&p))
            .sum();
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            // Report the first browser group's parent as the primary target
            // (covers puppeteer/chrome — the most common browser cache dir).
            if let Some(group) = self.groups.first() {
                r = r.with_target(group.parent.to_string_lossy().to_string());
            }
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let any_found = self.groups.iter().any(|g| g.parent.exists());
        if !any_found {
            println!("browsers: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
            });
        }

        let mut candidates: Vec<(PathBuf, u64)> = Vec::new();
        for group in &self.groups {
            for path in Self::find_old_versions(&group.parent) {
                let size = dir_size(&path);
                candidates.push((path, size));
            }
        }

        if !dry_run && !candidates.is_empty() {
            reporter.progress_init(self.name(), candidates.len());
        }

        let mut freed: u64 = 0;
        for (i, (path, size)) in candidates.iter().enumerate() {
            let entry_name = path.file_name().unwrap_or_default().to_string_lossy();
            if dry_run {
                println!(
                    "[dry-run] would remove: {entry_name} ({})",
                    crate::format::format_bytes(*size)
                );
            } else {
                reporter.progress_tick(path, i + 1, *size);
                crate::trash::delete_path(path)?;
                freed += size;
                println!("Removed: {entry_name}");
            }
        }

        if !dry_run && !candidates.is_empty() {
            reporter.progress_finish();
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn version_key_chrome_platform_prefix() {
        let k131 = BrowserCleaner::version_key("mac_arm-131.0.6778.204");
        let k140 = BrowserCleaner::version_key("mac_arm-140.0.7339.80");
        assert!(k140 > k131, "140.x must sort higher than 131.x");
    }

    #[test]
    fn version_key_playwright_build_number() {
        let k1208 = BrowserCleaner::version_key("chromium-1208");
        let k1217 = BrowserCleaner::version_key("chromium-1217");
        assert!(k1217 > k1208);
    }

    #[test]
    fn version_key_semver() {
        let k150 = BrowserCleaner::version_key("1.50.1");
        let k157 = BrowserCleaner::version_key("1.57.0");
        assert!(k157 > k150);
    }

    #[test]
    fn find_old_versions_returns_all_but_highest() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("mac_arm-131.0.6778.204")).unwrap();
        fs::create_dir_all(tmp.path().join("mac_arm-137.0.7151.119")).unwrap();
        fs::create_dir_all(tmp.path().join("mac_arm-140.0.7339.80")).unwrap();

        let old = BrowserCleaner::find_old_versions(tmp.path());
        assert_eq!(old.len(), 2);
        let names: Vec<_> = old
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert!(names.contains(&"mac_arm-131.0.6778.204".to_string()));
        assert!(names.contains(&"mac_arm-137.0.7151.119".to_string()));
    }

    #[test]
    fn find_old_versions_single_dir_returns_empty() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("chromium-1217")).unwrap();
        assert!(BrowserCleaner::find_old_versions(tmp.path()).is_empty());
    }

    #[test]
    fn find_old_versions_missing_dir_returns_empty() {
        assert!(BrowserCleaner::find_old_versions(Path::new("/does/not/exist")).is_empty());
    }

    // ── GAP-004 / GAP-005: edge cases ──────────────────────────────────────
    #[test]
    fn version_key_empty_string_returns_empty() {
        let key = BrowserCleaner::version_key("");
        assert!(
            key.is_empty(),
            "empty dir name must produce no version components"
        );
    }

    #[test]
    fn find_old_versions_skips_unparseable_dir_name() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("chromium-1217")).unwrap();
        // A directory name with zero digits — should not cause panic
        fs::create_dir_all(tmp.path().join("nightly")).unwrap();
        // Should only see chromium-1217 (one entry → returns empty = nothing to remove)
        let old = BrowserCleaner::find_old_versions(tmp.path());
        assert!(old.is_empty());
    }

    #[test]
    fn find_old_versions_skips_symlinks() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("chromium-140-safe");
        fs::create_dir_all(&target).unwrap();
        let link = tmp.path().join("chromium-120-symlink");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        // Without the symlink guard, the link would be followed; with the guard it is skipped.
        let old = BrowserCleaner::find_old_versions(tmp.path());
        assert_eq!(old.len(), 0, "symlinked dir must be skipped");
    }

    // ── primary_target ──────────────────────────────────────────────────────
    #[test]
    fn detect_includes_primary_target_when_verbose() {
        let _guard = crate::context::TEST_LOCK.lock().unwrap();
        crate::context::set_verbose(true);
        let tmp = TempDir::new().unwrap();
        let chrome = tmp.path().join(".cache/puppeteer/chrome");
        fs::create_dir_all(chrome.join("mac_arm-131.0.6778.204")).unwrap();
        fs::create_dir_all(chrome.join("mac_arm-140.0.7339.80")).unwrap();

        let cleaner =
            BrowserCleaner::new(tmp.path(), Box::new(crate::command::SystemCommandRunner));
        let result = cleaner.detect();
        assert!(
            result.primary_target.is_some(),
            "primary_target should be set when verbose"
        );
        assert!(
            result
                .primary_target
                .as_deref()
                .unwrap()
                .contains(".cache/puppeteer/chrome"),
            "target should point to first browser group parent"
        );
        crate::context::set_verbose(false);
    }

    #[test]
    fn detect_omits_primary_target_when_not_verbose() {
        let _guard = crate::context::TEST_LOCK.lock().unwrap();
        crate::context::set_verbose(false);
        let tmp = TempDir::new().unwrap();
        let chrome = tmp.path().join(".cache/puppeteer/chrome");
        fs::create_dir_all(chrome.join("mac_arm-131.0.6778.204")).unwrap();

        let cleaner =
            BrowserCleaner::new(tmp.path(), Box::new(crate::command::SystemCommandRunner));
        let result = cleaner.detect();
        assert!(result.primary_target.is_none());
    }
}
