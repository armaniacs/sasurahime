use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub struct GradleCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl GradleCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
    }

    fn find_old_caches(caches_dir: &Path) -> Vec<PathBuf> {
        let entries = match fs::read_dir(caches_dir) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        let mut versions: Vec<(Vec<u32>, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.starts_with(
                    |c: char| c.is_ascii_digit(),
                ) {
                    return None;
                }
                let key: Vec<u32> = name
                    .split(|c: char| !c.is_ascii_digit())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                Some((key, e.path()))
            })
            .collect();

        if versions.len() <= 1 {
            return vec![];
        }

        let max_key = versions.iter().map(|(k, _)| k.clone()).max().unwrap();
        versions.retain(|(k, _)| *k != max_key);
        versions.into_iter().map(|(_, p)| p).collect()
    }
}

impl Cleaner for GradleCleaner {
    fn name(&self) -> &'static str {
        "gradle"
    }

    fn detect(&self) -> ScanResult {
        let caches = self.home.join(".gradle/caches");
        if !caches.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let old = Self::find_old_caches(&caches);
        let bytes: u64 = old.iter().map(|p| dir_size(p)).sum();
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(
                self.home
                    .join(".gradle/caches")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        r
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let caches = self.home.join(".gradle/caches");
        if !caches.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }
        let old = Self::find_old_caches(&caches);
        let mut freed: u64 = 0;
        for path in &old {
            let size = dir_size(path);
            if dry_run {
                println!(
                    "[dry-run] [gradle] would remove: {} ({})",
                    path.display(),
                    crate::format::format_bytes(size)
                );
            } else {
                self.runner
                    .run("chflags", &["-R", "nouchg", &path.to_string_lossy()])
                    .ok();
                fs::remove_dir_all(path)?;
                freed += size;
                println!("[gradle] removed: {}", path.display());
            }
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: false,
            skipped: vec![],
        })
    }
}

pub struct JetBrainsCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl JetBrainsCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
    }

    fn find_old_caches(jetbrains_dir: &Path) -> Vec<PathBuf> {
        let entries = match fs::read_dir(jetbrains_dir) {
            Ok(e) => e,
            Err(_) => return vec![],
        };

        let mut by_ide: HashMap<String, Vec<(Vec<u32>, PathBuf)>> = HashMap::new();

        for entry in entries.filter_map(|e| e.ok()) {
            if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let ide_name: String = name
                .chars()
                .take_while(|c| c.is_ascii_alphabetic())
                .collect();
            if ide_name.is_empty() {
                continue;
            }
            let key: Vec<u32> = name[ide_name.len()..]
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|s| s.parse().ok())
                .collect();
            if key.is_empty() {
                continue;
            }
            by_ide
                .entry(ide_name)
                .or_default()
                .push((key, entry.path()));
        }

        let mut old = vec![];
        for versions in by_ide.values() {
            if versions.len() <= 1 {
                continue;
            }
            let max_key = versions.iter().map(|(k, _)| k.clone()).max().unwrap();
            for (k, p) in versions {
                if *k != max_key {
                    old.push(p.clone());
                }
            }
        }
        old
    }
}

impl Cleaner for JetBrainsCleaner {
    fn name(&self) -> &'static str {
        "jetbrains"
    }

    fn detect(&self) -> ScanResult {
        let dir = self.home.join("Library/Caches/JetBrains");
        if !dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let old = Self::find_old_caches(&dir);
        let bytes: u64 = old.iter().map(|p| dir_size(p)).sum();
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(
                self.home
                    .join("Library/Caches/JetBrains")
                    .to_string_lossy()
                    .to_string(),
            );
        }
        r
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let dir = self.home.join("Library/Caches/JetBrains");
        if !dir.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }
        let old = Self::find_old_caches(&dir);
        let mut freed: u64 = 0;
        for path in &old {
            let size = dir_size(path);
            if dry_run {
                println!(
                    "[dry-run] [jetbrains] would remove: {} ({})",
                    path.display(),
                    crate::format::format_bytes(size)
                );
            } else {
                self.runner
                    .run("chflags", &["-R", "nouchg", &path.to_string_lossy()])
                    .ok();
                fs::remove_dir_all(path)?;
                freed += size;
                println!("[jetbrains] removed: {}", path.display());
            }
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: false,
            skipped: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── GradleCleaner (M02) ──

    #[test]
    fn gradle_find_old_caches_single_version_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        let result = GradleCleaner::find_old_caches(&caches);
        assert!(result.is_empty());
    }

    #[test]
    fn gradle_find_old_caches_old_versions_returned() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("8.8.0")).unwrap();
        fs::create_dir_all(caches.join("8.10.1")).unwrap();
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        let result = GradleCleaner::find_old_caches(&caches);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.ends_with("8.8.0")));
        assert!(result.iter().any(|p| p.ends_with("8.10.1")));
    }

    #[test]
    fn gradle_find_old_caches_varying_digit_counts() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("7.0")).unwrap();
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        fs::create_dir_all(caches.join("8.12.1")).unwrap();
        let result = GradleCleaner::find_old_caches(&caches);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn gradle_find_old_caches_skip_non_version_names() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("modules-2")).unwrap();
        fs::create_dir_all(caches.join("wrapper")).unwrap();
        fs::create_dir_all(caches.join("journal-1")).unwrap();
        let result = GradleCleaner::find_old_caches(&caches);
        assert!(result.is_empty());
    }

    // ── JetBrainsCleaner (H01) ──

    #[test]
    fn jetbrains_find_old_caches_empty_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(&jb).unwrap();
        let result = JetBrainsCleaner::find_old_caches(&jb);
        assert!(result.is_empty());
    }

    #[test]
    fn jetbrains_find_old_caches_single_version_per_ide_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        let result = JetBrainsCleaner::find_old_caches(&jb);
        assert!(result.is_empty(), "single version must be kept");
    }

    #[test]
    fn jetbrains_find_old_caches_old_versions_are_returned() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        let result = JetBrainsCleaner::find_old_caches(&jb);
        assert_eq!(result.len(), 1);
        assert!(result[0].ends_with("GoLand2024.2"));
    }

    #[test]
    fn jetbrains_find_old_caches_multiple_ides_independent_retention() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        fs::create_dir_all(jb.join("IntelliJIdea2024.3")).unwrap();
        fs::create_dir_all(jb.join("IntelliJIdea2025.2")).unwrap();
        let result = JetBrainsCleaner::find_old_caches(&jb);
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|p| p.ends_with("GoLand2024.2")));
        assert!(result.iter().any(|p| p.ends_with("IntelliJIdea2024.3")));
    }

    #[test]
    fn jetbrains_find_old_caches_non_parseable_names_skipped() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("_tmp")).unwrap();
        fs::create_dir_all(jb.join(".hidden")).unwrap();
        let result = JetBrainsCleaner::find_old_caches(&jb);
        assert!(result.is_empty(), "unparseable names must be skipped");
    }
}
