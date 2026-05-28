use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub path: PathBuf,
    pub size: u64,
    #[expect(dead_code)]
    pub last_modified: SystemTime,
    pub reasons: Vec<DeleteReason>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeleteReason {
    Oversized { bytes: u64 },
    Stale { days: u64 },
}

pub const SKIP_DIRS: &[&str] = &["CrashReporter", "DiagnosticReports"];
pub const DEFAULT_KEEP_DAYS: u64 = 90;
pub const DEFAULT_SIZE_THRESHOLD: u64 = 100 * 1024 * 1024;

pub struct LibraryLogsCleaner {
    pub home: PathBuf,
    pub keep_days: u64,
    pub size_threshold: u64,
    pub runner: Box<dyn CommandRunner>,
}

fn entry_last_modified(path: &Path) -> Option<SystemTime> {
    let meta = fs::symlink_metadata(path).ok()?;
    let mtime = meta.modified().ok()?;
    let now = SystemTime::now();
    Some(if mtime > now { now } else { mtime })
}

fn classify_entry(
    _path: &Path,
    size: u64,
    last_modified: SystemTime,
    keep_days: u64,
    size_threshold: u64,
) -> Vec<DeleteReason> {
    let mut reasons = Vec::new();
    if size > size_threshold {
        reasons.push(DeleteReason::Oversized { bytes: size });
    }
    if let Ok(diff) = SystemTime::now().duration_since(last_modified) {
        let days = diff.as_secs() / 86400;
        if days >= keep_days {
            reasons.push(DeleteReason::Stale { days });
        }
    }
    reasons
}

fn scan_logs(dir: &Path, keep_days: u64, size_threshold: u64) -> Vec<LogEntry> {
    let dir = if dir.exists() { dir } else { return vec![] };
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let mut entries = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if file_name.starts_with('.') {
            continue;
        }
        if SKIP_DIRS.contains(&file_name.as_str()) {
            continue;
        }
        let size = dir_size(&path);
        if size == 0 {
            continue;
        }
        let last_modified = match entry_last_modified(&path) {
            Some(t) => t,
            None => continue,
        };
        let reasons = classify_entry(&path, size, last_modified, keep_days, size_threshold);
        if reasons.is_empty() {
            continue;
        }
        entries.push(LogEntry {
            path,
            size,
            last_modified,
            reasons,
        });
    }
    entries
}

// ── Constructor ──

impl LibraryLogsCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            keep_days: DEFAULT_KEEP_DAYS,
            size_threshold: DEFAULT_SIZE_THRESHOLD,
            runner,
        }
    }

    fn logs_dir(&self) -> PathBuf {
        self.home.join("Library/Logs")
    }

    pub(crate) fn scan(&self) -> Vec<LogEntry> {
        scan_logs(&self.logs_dir(), self.keep_days, self.size_threshold)
    }
}

// ── Cleaner trait ──

impl Cleaner for LibraryLogsCleaner {
    fn name(&self) -> &'static str {
        "library-logs"
    }

    fn detect(&self) -> ScanResult {
        let dir = self.logs_dir();
        if !dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let entries = self.scan();
        let total: u64 = entries.iter().map(|e| e.size).sum();
        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.logs_dir().to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let dir = self.logs_dir();
        if !dir.exists() {
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        let entries = self.scan();
        if entries.is_empty() {
            println!("[library-logs] nothing to clean");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        if dry_run {
            println!(
                "[library-logs] dry-run: {} cleanable entries",
                entries.len()
            );
            for e in &entries {
                let tags: Vec<String> = e
                    .reasons
                    .iter()
                    .map(|r| match r {
                        DeleteReason::Oversized { bytes } => {
                            format!("large {}", crate::format::format_bytes(*bytes))
                        }
                        DeleteReason::Stale { days } => format!("stale {}d", days),
                    })
                    .collect();
                println!(
                    "  would remove: {}  [{}]",
                    e.path.display(),
                    tags.join(", ")
                );
            }
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        let selected = self.interactive_select(&entries)?;

        if selected.is_empty() {
            println!("[library-logs] nothing selected");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        let mut total_freed: u64 = 0;
        reporter.progress_init(self.name(), selected.len());
        for (i, entry) in selected.iter().enumerate() {
            reporter.progress_tick(&entry.path, i + 1, entry.size);
            let path_str = entry.path.to_string_lossy();
            let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
            if let Err(e) = crate::trash::delete_path(&entry.path) {
                eprintln!(
                    "[library-logs] error removing {}: {e}",
                    entry.path.display()
                );
            } else {
                let size = entry.size;
                total_freed += size;
                println!(
                    "[library-logs] removed: {} (freed {})",
                    entry.path.display(),
                    crate::format::format_bytes(size)
                );
            }
        }
        reporter.progress_finish();

        println!(
            "[library-logs] total freed: {}",
            crate::format::format_bytes(total_freed)
        );
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: total_freed,
            uses_trash: true,
            skipped: vec![],
        })
    }
}

// ── Clean all (for --all flag, includes chflags) ──

impl LibraryLogsCleaner {
    pub(crate) fn clean_all(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
    ) -> Result<CleanResult> {
        let entries = self.scan();
        if entries.is_empty() {
            println!("[library-logs] nothing to clean");
            return Ok(CleanResult {
                name: "library-logs",
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }
        if dry_run {
            println!(
                "[library-logs] dry-run: {} cleanable entries",
                entries.len()
            );
            for e in &entries {
                let tags: Vec<String> = e
                    .reasons
                    .iter()
                    .map(|r| match r {
                        DeleteReason::Oversized { bytes } => {
                            format!("large {}", crate::format::format_bytes(*bytes))
                        }
                        DeleteReason::Stale { days } => format!("stale {}d", days),
                    })
                    .collect();
                println!(
                    "  would remove: {}  [{}]",
                    e.path.display(),
                    tags.join(", ")
                );
            }
            return Ok(CleanResult {
                name: "library-logs",
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }
        let mut total_freed: u64 = 0;
        reporter.progress_init("library-logs", entries.len());
        for (i, entry) in entries.iter().enumerate() {
            reporter.progress_tick(&entry.path, i + 1, entry.size);
            let path_str = entry.path.to_string_lossy();
            let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
            if let Err(e) = crate::trash::delete_path(&entry.path) {
                eprintln!(
                    "[library-logs] error removing {}: {e}",
                    entry.path.display()
                );
            } else {
                total_freed += entry.size;
                println!(
                    "[library-logs] removed: {} (freed {})",
                    entry.path.display(),
                    crate::format::format_bytes(entry.size)
                );
            }
        }
        reporter.progress_finish();
        Ok(CleanResult {
            name: "library-logs",
            bytes_freed: total_freed,
            uses_trash: true,
            skipped: vec![],
        })
    }
}

// ── Interactive selection ──

impl LibraryLogsCleaner {
    fn interactive_select(&self, entries: &[LogEntry]) -> Result<Vec<LogEntry>> {
        use dialoguer::MultiSelect;

        let items: Vec<String> = entries
            .iter()
            .map(|e| {
                let name = e.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let tags: Vec<String> = e
                    .reasons
                    .iter()
                    .map(|r| match r {
                        DeleteReason::Oversized { bytes } => {
                            format!("large {}", crate::format::format_bytes(*bytes))
                        }
                        DeleteReason::Stale { days } => format!("stale {}d", days),
                    })
                    .collect();
                format!(
                    "{:<24}  {}  [{}]",
                    name,
                    crate::format::format_bytes(e.size),
                    tags.join(", ")
                )
            })
            .collect();

        let defaults: Vec<bool> = vec![true; entries.len()];

        println!("\nCleanable log directories in ~/Library/Logs/:\n");
        let selections = MultiSelect::new()
            .items(&items)
            .defaults(&defaults)
            .interact()?;

        Ok(selections.into_iter().map(|i| entries[i].clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use filetime::{set_file_mtime, FileTime};
    use std::fs;
    use std::time::Duration;
    use tempfile::TempDir;

    fn logs_dir(tmp: &TempDir) -> PathBuf {
        let d = tmp.path().join("Library/Logs");
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn oversized_entry_triggers_large_reason() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        let f = logs.join("crash.log");
        fs::write(&f, b"xx").unwrap();
        let results = scan_logs(&logs, DEFAULT_KEEP_DAYS, 1);
        let entry = results.iter().find(|e| e.path == f).unwrap();
        assert!(entry
            .reasons
            .iter()
            .any(|r| matches!(r, DeleteReason::Oversized { .. })));
    }

    #[test]
    fn stale_entry_triggers_stale_reason() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        let f = logs.join("old.log");
        fs::write(&f, b"xx").unwrap();
        let past = SystemTime::now() - Duration::from_secs(200 * 86400);
        set_file_mtime(&f, FileTime::from_system_time(past)).unwrap();
        let results = scan_logs(&logs, 90, u64::MAX);
        let entry = results.iter().find(|e| e.path == f).unwrap();
        assert!(entry
            .reasons
            .iter()
            .any(|r| matches!(r, DeleteReason::Stale { days: 200 })));
    }

    #[test]
    fn entry_under_threshold_has_no_reason() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        fs::write(logs.join("small.log"), b"tiny").unwrap();
        let results = scan_logs(&logs, 9999, u64::MAX);
        assert!(results.is_empty());
    }

    #[test]
    fn skip_dirs_excluded() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        for name in &["CrashReporter", "DiagnosticReports"] {
            let d = logs.join(name);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("crash.log"), b"xx").unwrap();
        }
        let results = scan_logs(&logs, DEFAULT_KEEP_DAYS, 1);
        assert!(results.is_empty());
    }

    #[test]
    fn dot_entries_excluded() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        fs::write(logs.join(".hidden"), b"data").unwrap();
        let results = scan_logs(&logs, DEFAULT_KEEP_DAYS, 1);
        assert!(results.is_empty());
    }

    #[test]
    fn future_mtime_does_not_trigger_stale() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        let f = logs.join("future.log");
        fs::write(&f, b"xx").unwrap();
        let future = SystemTime::now() + Duration::from_secs(3600);
        set_file_mtime(&f, FileTime::from_system_time(future)).unwrap();
        let results = scan_logs(&logs, 1, u64::MAX);
        assert!(results.is_empty(), "future mtime must not trigger stale");
    }

    #[test]
    fn classify_entry_oversized_only() {
        let reasons = classify_entry(Path::new("/tmp"), 200, SystemTime::now(), 90, 100);
        assert!(reasons.contains(&DeleteReason::Oversized { bytes: 200 }));
        assert!(!reasons
            .iter()
            .any(|r| matches!(r, DeleteReason::Stale { .. })));
    }

    #[test]
    fn classify_entry_stale_only() {
        let old = SystemTime::now() - Duration::from_secs(100 * 86400);
        let reasons = classify_entry(Path::new("/tmp"), 1, old, 90, u64::MAX);
        assert!(reasons
            .iter()
            .any(|r| matches!(r, DeleteReason::Stale { days: 100 })));
    }

    #[test]
    fn classify_entry_no_reasons() {
        let reasons = classify_entry(Path::new("/tmp"), 1, SystemTime::now(), 90, u64::MAX);
        assert!(reasons.is_empty());
    }

    #[test]
    fn scan_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let results = scan_logs(tmp.path().join("nonexistent").as_ref(), 90, 100);
        assert!(results.is_empty());
    }

    #[test]
    fn zero_size_entry_skipped() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        let f = logs.join("empty.log");
        fs::write(&f, b"").unwrap();
        let results = scan_logs(&logs, 1, 1);
        assert!(results.is_empty(), "zero-size entry should be skipped");
    }

    #[test]
    fn clean_all_processes_all_entries_without_selection() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        fs::write(logs.join("big.log"), b"x".repeat(200)).unwrap();
        fs::write(logs.join("old.log"), b"y".repeat(200)).unwrap();
        let past = SystemTime::now() - Duration::from_secs(200 * 86400);
        set_file_mtime(&logs.join("old.log"), FileTime::from_system_time(past)).unwrap();

        let runner = crate::test_helpers::MockRunner::new()
            .with_success("chflags");
        let cleaner = LibraryLogsCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner.clean_all(false, &crate::progress::DeepSuppressReporter).unwrap();
        assert!(result.bytes_freed > 0, "clean_all should process all entries");
        assert!(result.uses_trash, "LibraryLogsCleaner should use trash");
    }

    #[test]
    fn clean_via_trait_processes_entries_with_interactive() {
        let tmp = TempDir::new().unwrap();
        let logs = logs_dir(&tmp);
        fs::write(logs.join("big.log"), b"x".repeat(200)).unwrap();
        let runner = crate::test_helpers::MockRunner::new()
            .with_success("chflags");
        let cleaner = LibraryLogsCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner.clean(true, &crate::progress::DeepSuppressReporter).unwrap();
        // dry run should succeed without interactive prompt
        assert_eq!(result.bytes_freed, 0);
    }
}
