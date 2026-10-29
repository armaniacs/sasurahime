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
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Manual
    }

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

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        let path = &self.path;
        if !crate::safety::is_safe_delete_target(path) {
            eprintln!(
                "{}: path {:?} is not safe to delete, skipping",
                self.name, path
            );
            return Ok(CleanResult::empty(self.name));
        }
        if !self.path.exists() {
            println!("{}: not found, skipping", self.name);
            return Ok(CleanResult::empty(self.name));
        }

        let entries: Vec<fs::DirEntry> = match fs::read_dir(&self.path) {
            Ok(e) => e.filter_map(|e| e.ok()).collect(),
            Err(_) => return Ok(CleanResult::empty(self.name)),
        };

        // The caller owns the per-item size computation; the ledger owns
        // the per-item contracts (progress, skip records, freed sums,
        // deleted paths, CleanResult assembly).
        let items: Vec<(PathBuf, u64)> = entries
            .iter()
            .map(|e| {
                let p = e.path();
                (p.clone(), crate::format::dir_size(&p))
            })
            .collect();

        if dry_run {
            for (path, size) in &items {
                let entry_name = path.file_name().unwrap_or_default().to_string_lossy();
                println!(
                    "[dry-run] would remove: {entry_name} ({})",
                    crate::format::format_bytes(*size)
                );
            }
            return Ok(CleanResult::empty(self.name));
        }

        // The cleaner has no runner field: the seam only needs one for the
        // permanent-mode uchg clear, which the system runner performs. The
        // ledger's Global mode keeps the process-global deletion mode.
        Ok(crate::trash::delete_ledger(
            self.name,
            reporter,
            &items,
            crate::trash::seam_delete(
                &crate::command::SystemCommandRunner,
                crate::trash::TrashMode::Global,
            ),
        ))
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
        let result = cleaner
            .clean(true, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert!(file.exists(), "dry-run must not delete files");
    }

    #[test]
    fn clean_deletes_contents_not_root() {
        // Pin permanent deletion: the Global compat delegator resolves to the
        // default trash mode otherwise, whose backend is TCC-blocked in this
        // sandbox (bytes_freed 0). The test must pin the mode it needs instead
        // of depending on ambient global state (PBI 46 guard scoping removed
        // the accidental leaks this test used to rely on).
        let _mode = crate::test_helpers::TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let cache_dir = tmp.path().join("my-cache");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("data.bin"), b"content").unwrap();

        let reporter = DeepSuppressReporter;
        let cleaner = CustomPathCleaner::new("my-cache".to_string(), cache_dir.clone());
        cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();

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
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
    }

    #[test]
    fn clean_rejects_unsafe_path() {
        let cleaner = CustomPathCleaner::new(
            "unsafe".to_string(),
            std::path::PathBuf::from("/etc/passwd"),
        );
        let reporter = DeepSuppressReporter;
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "must not delete unsafe paths");
    }
}
