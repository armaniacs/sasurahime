use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

struct BrowserGroup {
    label: &'static str,
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
                    label: "puppeteer/chrome",
                    parent: home.join(".cache/puppeteer/chrome"),
                },
                BrowserGroup {
                    label: "puppeteer/chrome-headless-shell",
                    parent: home.join(".cache/puppeteer/chrome-headless-shell"),
                },
                BrowserGroup {
                    label: "ms-playwright",
                    parent: home.join("Library/Caches/ms-playwright"),
                },
                BrowserGroup {
                    label: "ms-playwright-go",
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
    pub fn find_old_versions(parent: &Path) -> Vec<PathBuf> {
        let entries = match fs::read_dir(parent) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        let mut versions: Vec<(Vec<u32>, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .map(|e| (Self::version_key(&e.file_name().to_string_lossy()), e.path()))
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
            return ScanResult { name: self.name(), status: ScanStatus::NotFound };
        }
        let bytes: u64 = self
            .groups
            .iter()
            .flat_map(|g| Self::find_old_versions(&g.parent))
            .map(|p| dir_size(&p))
            .sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 { ScanStatus::Pruneable(bytes) } else { ScanStatus::Clean },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        let any_found = self.groups.iter().any(|g| g.parent.exists());
        if !any_found {
            println!("browsers: not found, skipping");
            return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
        }

        let mut freed: u64 = 0;
        for group in &self.groups {
            for path in Self::find_old_versions(&group.parent) {
                let size = dir_size(&path);
                let entry_name = path.file_name().unwrap_or_default().to_string_lossy();
                if dry_run {
                    println!("[dry-run] would remove: {}/{entry_name} ({})",
                        group.label, crate::format::format_bytes(size));
                } else {
                    fs::remove_dir_all(&path)
                        .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))?;
                    freed += size;
                    println!("Removed: {}/{entry_name}", group.label);
                }
            }
        }
        Ok(CleanResult { name: self.name(), bytes_freed: freed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let names: Vec<_> = old.iter()
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
}
