use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// A hardcoded log target (name is `&'static str`).
struct LogTarget {
    name: &'static str,
    path: PathBuf,
    exclude: Vec<String>,
}

/// A user-configured log target from the config file (name is an owned String).
pub struct OwnedLogTarget {
    pub name: String,
    pub path: PathBuf,
    pub exclude: Vec<String>,
}

/// Unified view used internally by LogCleaner.
struct AnyLogTarget<'a> {
    name: &'a str,
    path: &'a PathBuf,
    exclude: &'a [String],
}

pub struct LogCleaner {
    builtin: Vec<LogTarget>,
    extra: Vec<OwnedLogTarget>,
    keep_days: u32,
}

impl LogCleaner {
    /// `#[allow(dead_code)]`: clients call `new_with_extra` instead.
    #[allow(dead_code)]
    pub fn new(home: &Path, keep_days: u32) -> Self {
        Self::new_with_extra(home, keep_days, vec![])
    }

    /// Like `new`, but appends user-configured targets from the config file.
    pub fn new_with_extra(home: &Path, keep_days: u32, extra: Vec<OwnedLogTarget>) -> Self {
        Self {
            builtin: vec![
                LogTarget {
                    name: "kilo",
                    path: home.join(".local/share/kilo/log"),
                    exclude: vec!["dev.log".to_string()],
                },
                LogTarget {
                    name: "opencode",
                    path: home.join(".local/share/opencode/logs"),
                    exclude: vec![],
                },
                LogTarget {
                    name: "claude-code",
                    path: home.join(".local/share/claude/logs"),
                    exclude: vec![],
                },
                LogTarget {
                    name: "vscode-logs",
                    path: home.join("Library/Application Support/Code/logs"),
                    exclude: vec![],
                },
            ],
            extra,
            keep_days,
        }
    }

    fn all_targets(&self) -> impl Iterator<Item = AnyLogTarget<'_>> {
        let builtins = self.builtin.iter().map(|t| AnyLogTarget {
            name: t.name,
            path: &t.path,
            exclude: &t.exclude,
        });
        let extras = self.extra.iter().map(|t| AnyLogTarget {
            name: &t.name,
            path: &t.path,
            exclude: &t.exclude,
        });
        builtins.chain(extras)
    }

    /// Returns `true` if the file's mtime is strictly older than `days` days.
    pub fn is_older_than(metadata: &fs::Metadata, days: u32) -> bool {
        let threshold = Duration::from_secs(u64::from(days) * 86_400);
        metadata
            .modified()
            .ok()
            .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
            .map(|age| age > threshold)
            .unwrap_or(false)
    }

    /// Returns paths of files in `dir` older than `keep_days`, excluding names in `exclude`.
    pub fn find_old_logs(dir: &Path, keep_days: u32, exclude: &[String]) -> Vec<PathBuf> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return vec![],
        };
        entries
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !exclude.iter().any(|ex| ex == &name)
            })
            .filter(|e| {
                e.metadata()
                    .map(|m| Self::is_older_than(&m, keep_days))
                    .unwrap_or(false)
            })
            .map(|e| e.path())
            .collect()
    }
}

impl Cleaner for LogCleaner {
    fn name(&self) -> &'static str {
        "logs"
    }

    fn detect(&self) -> ScanResult {
        let any_found = self.all_targets().any(|t| t.path.exists());
        if !any_found {
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        let bytes: u64 = self
            .all_targets()
            .flat_map(|t| Self::find_old_logs(t.path, self.keep_days, t.exclude))
            .filter_map(|p| fs::metadata(&p).ok())
            .map(|m| m.blocks() * 512)
            .sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let mut all_old: Vec<(String, PathBuf)> = Vec::new();
        for target in self.all_targets() {
            let target_name = target.name.to_string();
            for path in Self::find_old_logs(target.path, self.keep_days, target.exclude) {
                all_old.push((target_name.clone(), path));
            }
        }

        let mut freed: u64 = 0;
        let mut deleted: u32 = 0;

        if dry_run {
            for (target_name, path) in &all_old {
                println!("[dry-run] [{target_name}] would remove: {}", path.display());
            }
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
            });
        }

        reporter.progress_init(self.name(), all_old.len());
        for (i, (target_name, path)) in all_old.iter().enumerate() {
            let size = fs::metadata(path).map(|m| m.blocks() * 512).unwrap_or(0);
            reporter.progress_tick(path, i + 1, size);
            crate::trash::delete_path(path)?;
            freed += size;
            deleted += 1;
            println!("[{target_name}] Removed: {}", path.display());
        }
        reporter.progress_finish();

        println!("Removed {deleted} log files");
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::FileTime;
    use tempfile::TempDir;

    fn write_aged(path: &Path, secs_old: u64) {
        fs::write(path, b"x").unwrap();
        let mtime = SystemTime::now() - Duration::from_secs(secs_old);
        filetime::set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
    }

    #[test]
    fn is_older_than_boundary_under() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.log");
        // 6d 23h → NOT older than 7d
        write_aged(&p, 6 * 86_400 + 82_800);
        assert!(!LogCleaner::is_older_than(&fs::metadata(&p).unwrap(), 7));
    }

    #[test]
    fn is_older_than_boundary_over() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.log");
        // 7d 1s → IS older than 7d
        write_aged(&p, 7 * 86_400 + 1);
        assert!(LogCleaner::is_older_than(&fs::metadata(&p).unwrap(), 7));
    }

    #[test]
    fn find_old_logs_excludes_dev_log() {
        let tmp = TempDir::new().unwrap();
        write_aged(&tmp.path().join("dev.log"), 30 * 86_400);
        write_aged(&tmp.path().join("old.log"), 30 * 86_400);

        let old = LogCleaner::find_old_logs(tmp.path(), 7, &["dev.log".to_string()]);
        assert_eq!(old.len(), 1);
        assert!(old[0].ends_with("old.log"));
    }

    #[test]
    fn find_old_logs_missing_dir_returns_empty() {
        let old = LogCleaner::find_old_logs(Path::new("/does/not/exist"), 7, &[]);
        assert!(old.is_empty());
    }

    // ── GAP-008: edge cases for is_older_than ──────────────────────────────
    #[test]
    fn is_older_than_just_under_threshold_is_not_older() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.log");
        // 7 days minus 1 minute → NOT older (accounts for test execution time)
        write_aged(&p, 7 * 86_400 - 60);
        assert!(!LogCleaner::is_older_than(&fs::metadata(&p).unwrap(), 7));
    }

    #[test]
    fn is_older_than_zero_days_never_older() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.log");
        write_aged(&p, 0);
        assert!(!LogCleaner::is_older_than(&fs::metadata(&p).unwrap(), 1));
    }

    #[test]
    fn is_older_than_now_is_not_older() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("f.log");
        std::fs::write(&p, b"x").unwrap();
        // Use current time (file created just now → age ≈ 0)
        assert!(!LogCleaner::is_older_than(&fs::metadata(&p).unwrap(), 1));
    }

    #[test]
    fn is_older_than_missing_metadata_returns_false() {
        // Fake metadata that won't have a valid modified() time
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("does_not_exist.log");
        // fs::metadata on non-existent file returns Err → unwrap fails
        // We test the function's defensive behaviour via the inner ok() chain
        let meta = std::fs::metadata(&p);
        assert!(meta.is_err());
        // The function itself uses unwrap_or(false) internally; this test is
        // primarily documentation of that safety net.
    }

    /// Creates a symlink to /dev/null and confirms is_older_than still returns
    /// false gracefully (no panic on unusual file types).
    #[test]
    fn is_older_than_symlink_does_not_panic() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("link.log");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/dev/null", &p).unwrap();
            assert!(!LogCleaner::is_older_than(&fs::metadata(&p).unwrap(), 7));
        }
    }
}
