use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

pub struct CustomPathCleaner {
    name: &'static str,
    path: PathBuf,
}

impl CustomPathCleaner {
    pub fn new(name: String, path: PathBuf) -> Self {
        Self {
            // SAFETY: CustomPathCleaner instances are created once per program
            // invocation in all_cleaners() and live for the full lifetime.
            // The leaked allocation (a per-cleaner name string) is intentional
            // and negligible.
            name: Box::leak(name.into_boxed_str()),
            path,
        }
    }
}

impl Cleaner for CustomPathCleaner {
    fn name(&self) -> &'static str {
        self.name
    }

    fn detect(&self) -> ScanResult {
        if !self.path.exists() {
            return ScanResult::new(self.name, ScanStatus::NotFound);
        }
        let bytes = crate::format::dir_size(&self.path);
        let mut r = ScanResult::new(
            self.name,
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.path.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let path = &self.path;
        if !crate::cleaners::generic::is_safe_delete_target(path) {
            eprintln!(
                "{}: path {:?} is not safe to delete, skipping",
                self.name, path
            );
            return Ok(CleanResult {
                name: self.name,
                bytes_freed: 0,
                uses_trash: true,
                skipped: vec![],
            });
        }
        if !self.path.exists() {
            println!("{}: not found, skipping", self.name);
            return Ok(CleanResult {
                name: self.name,
                bytes_freed: 0,
                uses_trash: true,
                skipped: vec![],
            });
        }

        let entries: Vec<fs::DirEntry> = match fs::read_dir(&self.path) {
            Ok(e) => e.filter_map(|e| e.ok()).collect(),
            Err(_) => {
                return Ok(CleanResult {
                    name: self.name,
                    bytes_freed: 0,
                    uses_trash: true,
                    skipped: vec![],
                })
            }
        };

        if !dry_run && !entries.is_empty() {
            reporter.progress_init(self.name, entries.len());
        }

        let mut freed: u64 = 0;
        for (i, entry) in entries.iter().enumerate() {
            let path = entry.path();
            let size = crate::format::dir_size(&path);
            let entry_name = path.file_name().unwrap_or_default().to_string_lossy();
            if dry_run {
                println!(
                    "[dry-run] would remove: {entry_name} ({})",
                    crate::format::format_bytes(size)
                );
            } else {
                reporter.progress_tick(&path, i + 1, size);
                let _ = std::process::Command::new("chflags")
                    .args(["-R", "nouchg"])
                    .arg(&path)
                    .status();
                crate::trash::delete_path(&path)?;
                freed += size;
                println!("Removed: {entry_name}");
            }
        }

        if !dry_run && !entries.is_empty() {
            reporter.progress_finish();
        }

        Ok(CleanResult {
            name: self.name,
            bytes_freed: freed,
            uses_trash: true,
            skipped: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::DeepSuppressReporter;
    use tempfile::TempDir;

    #[test]
    fn detect_not_found_when_missing() {
        let tmp = TempDir::new().unwrap();
        let cleaner = CustomPathCleaner::new("test".to_string(), tmp.path().join("nonexistent"));
        assert!(matches!(cleaner.detect().status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_pruneable_when_content_exists() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("my-cache");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("data.bin"), b"content").unwrap();

        let cleaner = CustomPathCleaner::new("my-cache".to_string(), cache_dir);
        assert!(matches!(cleaner.detect().status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn detect_clean_when_empty() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("empty-cache");
        fs::create_dir_all(&cache_dir).unwrap();

        let cleaner = CustomPathCleaner::new("empty-cache".to_string(), cache_dir);
        assert!(matches!(cleaner.detect().status, ScanStatus::Clean));
    }

    #[test]
    fn clean_dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("my-cache");
        fs::create_dir_all(&cache_dir).unwrap();
        let file = cache_dir.join("data.bin");
        fs::write(&file, b"content").unwrap();

        let reporter = DeepSuppressReporter;
        let cleaner = CustomPathCleaner::new("my-cache".to_string(), cache_dir.clone());
        let result = cleaner.clean(true, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert!(file.exists(), "dry-run must not delete files");
    }

    #[test]
    fn clean_deletes_contents_not_root() {
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("my-cache");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("data.bin"), b"content").unwrap();

        let reporter = DeepSuppressReporter;
        let cleaner = CustomPathCleaner::new("my-cache".to_string(), cache_dir.clone());
        cleaner.clean(false, &reporter).unwrap();

        // Root dir must still exist
        assert!(cache_dir.exists(), "root dir must survive clean");
        // Contents must be gone
        assert!(
            !cache_dir.join("data.bin").exists(),
            "contents must be deleted"
        );
    }

    #[test]
    fn clean_skips_missing_path() {
        let tmp = TempDir::new().unwrap();
        let cleaner = CustomPathCleaner::new("missing".to_string(), tmp.path().join("nonexistent"));
        let reporter = DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn clean_rejects_unsafe_path() {
        let cleaner = CustomPathCleaner::new(
            "unsafe".to_string(),
            std::path::PathBuf::from("/etc/passwd"),
        );
        let reporter = DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.bytes_freed, 0, "must not delete unsafe paths");
    }
}
