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
                let key: Vec<u32> = name
                    .split(|c: char| !c.is_ascii_digit())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if key.is_empty() {
                    return None;
                }
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
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        let old = Self::find_old_caches(&caches);
        let bytes: u64 = old.iter().map(|p| dir_size(p)).sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let caches = self.home.join(".gradle/caches");
        if !caches.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
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
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        let old = Self::find_old_caches(&dir);
        let bytes: u64 = old.iter().map(|p| dir_size(p)).sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let dir = self.home.join("Library/Caches/JetBrains");
        if !dir.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
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
        })
    }
}
