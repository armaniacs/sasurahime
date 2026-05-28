use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub struct DeviceSupportCleaner {
    xcode_dev_dir: PathBuf,
    keep: u32,
    runner: Box<dyn CommandRunner>,
}

impl DeviceSupportCleaner {
    pub fn new(home: &Path, keep: u32, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            xcode_dev_dir: home.join("Library/Developer/Xcode"),
            keep,
            runner,
        }
    }

    fn scan_platforms(&self) -> Vec<(String, Vec<DeviceSupportEntry>)> {
        let mut platforms: Vec<(String, Vec<DeviceSupportEntry>)> = Vec::new();

        let entries = match fs::read_dir(&self.xcode_dev_dir) {
            Ok(e) => e,
            Err(_) => return platforms,
        };

        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(" DeviceSupport")
                || !entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
            {
                continue;
            }
            let platform = name
                .strip_suffix(" DeviceSupport")
                .unwrap_or(&name)
                .to_string();

            let versions: Vec<DeviceSupportEntry> = fs::read_dir(entry.path())
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                .filter_map(|e| {
                    let dir_name = e.file_name().to_string_lossy().to_string();
                    let major = dir_name
                        .split('.')
                        .next()
                        .and_then(|s| s.parse::<u32>().ok())?;
                    Some(DeviceSupportEntry {
                        name: dir_name,
                        path: e.path(),
                        major_version: major,
                        size: dir_size(&e.path()),
                    })
                })
                .collect();

            if !versions.is_empty() {
                platforms.push((platform, versions));
            }
        }
        platforms
    }

    fn versions_to_delete(&self) -> Vec<PathBuf> {
        let mut to_delete = Vec::new();
        for (_platform, mut versions) in self.scan_platforms() {
            versions.sort_by_key(|b| std::cmp::Reverse(b.major_version));
            for v in versions.into_iter().skip(self.keep as usize) {
                to_delete.push(v.path);
            }
        }
        to_delete
    }
}

#[derive(Debug, Clone)]
#[expect(dead_code)]
struct DeviceSupportEntry {
    name: String,
    path: PathBuf,
    major_version: u32,
    size: u64,
}

impl Cleaner for DeviceSupportCleaner {
    fn name(&self) -> &'static str {
        "device-support"
    }

    fn detect(&self) -> ScanResult {
        let to_delete = self.versions_to_delete();
        if to_delete.is_empty() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let total: u64 = to_delete.iter().map(|p| dir_size(p)).sum();
        let mut r = ScanResult::new(self.name(), ScanStatus::Pruneable(total));
        if crate::context::is_verbose() {
            r = r.with_target(self.xcode_dev_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let to_delete = self.versions_to_delete();
        if to_delete.is_empty() {
            println!("[device-support] nothing to clean");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        if dry_run {
            for p in &to_delete {
                let size = dir_size(p);
                println!(
                    "[dry-run] would remove: {} ({})",
                    p.display(),
                    crate::format::format_bytes(size)
                );
            }
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        reporter.progress_init(self.name(), to_delete.len());

        let mut freed: u64 = 0;
        let mut skipped = vec![];
        for (i, p) in to_delete.iter().enumerate() {
            let size = dir_size(p);
            reporter.progress_tick(p, i + 1, size);
            let path_str = p.to_string_lossy();
            let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
            if let Err(e) = crate::trash::delete_path(p) {
                skipped.push(SkippedEntry {
                    path: p.to_path_buf(),
                    reason: format!("{e:#}"),
                });
                eprintln!("[device-support] error removing {}: {e}", p.display());
            } else {
                freed += size;
                println!("[device-support] removed: {}", p.display());
            }
        }

        reporter.progress_finish();

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: true,
            skipped,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SystemCommandRunner;
    use tempfile::TempDir;

    #[test]
    fn keeps_highest_n_versions() {
        let tmp = TempDir::new().unwrap();
        let ios = tmp.path().join("Library/Developer/Xcode/iOS DeviceSupport");
        for v in &["14.0", "15.0", "16.0", "17.0"] {
            fs::create_dir_all(ios.join(v)).unwrap();
            fs::write(ios.join(v).join("dummy"), b"x").unwrap();
        }
        let cleaner = DeviceSupportCleaner::new(tmp.path(), 2, Box::new(SystemCommandRunner));
        let to_delete = cleaner.versions_to_delete();
        assert_eq!(to_delete.len(), 2, "4 versions - keep 2 = 2 to delete");
        assert!(to_delete.iter().any(|p| p.ends_with("14.0")));
        assert!(to_delete.iter().any(|p| p.ends_with("15.0")));
    }

    #[test]
    fn detect_includes_primary_target_when_verbose() {
        let _guard = crate::test_helpers::VerboseGuard::new();
        let tmp = TempDir::new().unwrap();
        // Need 4 versions with keep=2 so versions_to_delete() returns 2 entries
        let ios = tmp.path().join("Library/Developer/Xcode/iOS DeviceSupport");
        for v in &["14.0", "15.0", "16.0", "17.0"] {
            fs::create_dir_all(ios.join(v)).unwrap();
            fs::write(ios.join(v).join("dummy"), b"x").unwrap();
        }

        let cleaner = DeviceSupportCleaner::new(tmp.path(), 2, Box::new(SystemCommandRunner));
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
                .contains("Library/Developer/Xcode"),
            "target should point to Xcode dev directory"
        );
    }

    #[test]
    fn detect_omits_primary_target_when_not_verbose() {
        let _guard = crate::test_helpers::VerboseGuard::with_value(false);
        let tmp = TempDir::new().unwrap();
        // Again need 4 versions so versions_to_delete() is non-empty
        let ios = tmp.path().join("Library/Developer/Xcode/iOS DeviceSupport");
        for v in &["14.0", "15.0", "16.0", "17.0"] {
            fs::create_dir_all(ios.join(v)).unwrap();
            fs::write(ios.join(v).join("dummy"), b"x").unwrap();
        }

        let cleaner = DeviceSupportCleaner::new(tmp.path(), 2, Box::new(SystemCommandRunner));
        let result = cleaner.detect();
        assert!(result.primary_target.is_none());
    }
}
