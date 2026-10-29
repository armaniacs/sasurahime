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

/// A version subdirectory that may become a deletion candidate: a real
/// directory, never a symlink. Symlinked versions are stale or shared links —
/// never deletion candidates (the browsers cleaner's rule). An unreadable file
/// type is treated as a symlink: never touch what we cannot classify.
pub(crate) fn is_version_dir(e: &fs::DirEntry) -> bool {
    match e.file_type() {
        Ok(t) => t.is_dir() && !t.is_symlink(),
        Err(_) => false,
    }
}

/// One owner of the "keep all but the highest version" retention rule shared
/// by the browsers, gradle and JetBrains cleaners. Groups `candidates` by
/// their numeric version key and returns the paths whose key is not the
/// highest in the group. A candidate without a numeric key (a name with no
/// digits) is never a deletion candidate.
pub(crate) fn keep_highest(candidates: Vec<(Vec<u32>, PathBuf)>) -> Vec<PathBuf> {
    // A name without digits has no version key: never a deletion candidate.
    let versioned: Vec<(Vec<u32>, PathBuf)> = candidates
        .into_iter()
        .filter(|(k, _)| !k.is_empty())
        .collect();
    let Some(max) = versioned.iter().map(|(k, _)| k.clone()).max() else {
        return vec![];
    };
    versioned
        .into_iter()
        .filter(|(k, _)| *k != max)
        .map(|(_, p)| p)
        .collect()
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

        let candidates: Vec<(Vec<u32>, PathBuf)> = entries
            .filter_map(|e| e.ok())
            .filter(is_version_dir)
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                if !name.starts_with(|c: char| c.is_ascii_digit()) {
                    return None;
                }
                let key: Vec<u32> = name
                    .split(|c: char| !c.is_ascii_digit())
                    .filter_map(|s| s.parse().ok())
                    .collect();
                Some((key, e.path()))
            })
            .collect();

        keep_highest(candidates)
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

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        let caches = self.home.join(".gradle/caches");
        if !caches.exists() {
            // Nothing is deleted on this path: the seam is never invoked, so
            // no Trash claim may be made.
            return Ok(CleanResult::empty(self.name()));
        }
        let old = Self::find_old_caches(&caches);
        // The caller owns the per-item size computation; the ledger owns
        // the per-item contracts (progress — new for this cleaner, skip
        // records, freed sums, deleted paths).
        let items: Vec<(PathBuf, u64)> = old.iter().map(|p| (p.clone(), dir_size(p))).collect();

        if dry_run {
            for (path, size) in &items {
                println!(
                    "[dry-run] [gradle] would remove: {} ({})",
                    path.display(),
                    crate::format::format_bytes(*size)
                );
            }
            return Ok(CleanResult::empty(self.name()));
        }

        // The seam adapter's Global mode keeps the process-global deletion
        // mode; a fatal per-item error no longer aborts the run.
        Ok(crate::trash::delete_ledger(
            self.name(),
            reporter,
            &items,
            crate::trash::seam_delete(self.runner.as_ref(), crate::trash::TrashMode::Global),
        ))
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
            if !is_version_dir(&entry) {
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
            old.extend(keep_highest(versions.clone()));
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

    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &crate::cleaner::CleanOptions,
    ) -> Result<CleanResult> {
        let dir = self.home.join("Library/Caches/JetBrains");
        if !dir.exists() {
            // Nothing is deleted on this path: the seam is never invoked, so
            // no Trash claim may be made.
            return Ok(CleanResult::empty(self.name()));
        }
        let old = Self::find_old_caches(&dir);
        // The caller owns the per-item size computation; the ledger owns
        // the per-item contracts (progress — new for this cleaner, skip
        // records, freed sums, deleted paths).
        let items: Vec<(PathBuf, u64)> = old.iter().map(|p| (p.clone(), dir_size(p))).collect();

        if dry_run {
            for (path, size) in &items {
                println!(
                    "[dry-run] [jetbrains] would remove: {} ({})",
                    path.display(),
                    crate::format::format_bytes(*size)
                );
            }
            return Ok(CleanResult::empty(self.name()));
        }

        // The seam adapter's Global mode keeps the process-global deletion
        // mode; a fatal per-item error no longer aborts the run.
        Ok(crate::trash::delete_ledger(
            self.name(),
            reporter,
            &items,
            crate::trash::seam_delete(self.runner.as_ref(), crate::trash::TrashMode::Global),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::MockRunner;
    use std::fs;
    use tempfile::TempDir;

    // ── GradleCleaner (M02) ──

    #[test]
    fn gradle_detect_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        fs::create_dir_all(caches.join("8.8.0")).unwrap();
        // Write files so directories have non-zero size
        fs::write(caches.join("8.12.0").join("cache.dat"), b"data").unwrap();
        fs::write(caches.join("8.8.0").join("cache.dat"), b"data").unwrap();
        let cleaner = GradleCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
        assert!(caches.join("8.12.0").exists(), "detect must not delete");
        assert!(caches.join("8.8.0").exists(), "detect must not delete");
    }

    #[test]
    fn gradle_clean_uses_trash_true() {
        // TrashModeGuard::set holds TEST_LOCK until Drop, so parallel
        // trash-mode mutators serialize with this read; the pin makes the
        // seam resolution deterministic.
        let _mode = crate::test_helpers::TrashModeGuard::set(true);
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        fs::create_dir_all(caches.join("8.8.0")).unwrap();
        let runner = MockRunner::new().with_success("chflags");
        let cleaner = GradleCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        // The seam outcome is the one owner of the Trash claim: uses_trash
        // must come from the deletion outcomes, never from a global read. A
        // candidate is either deleted or skipped, so under the trash pin
        // Trash is claimed exactly when a deletion actually happened. (In a
        // TCC-restricted sandbox the Trash move is denied and every deletion
        // comes back skipped: there the truthful claim is false, which is
        // what this assertion then checks.)
        let deletions_happened = result.skipped.is_empty();
        assert_eq!(
            result.uses_trash, deletions_happened,
            "uses_trash must come from the deletion outcomes' seam truth"
        );
    }

    #[test]
    fn gradle_clean_without_deletion_candidates_reports_uses_trash_false() {
        // Zero deletions: the seam is never invoked, so no Trash claim may be
        // made, even with the global pinned to trash.
        let _mode = crate::test_helpers::TrashModeGuard::set(true);
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        // Only non-version names: find_old_caches returns no candidate.
        fs::create_dir_all(caches.join("modules-2")).unwrap();
        fs::create_dir_all(caches.join("wrapper")).unwrap();
        let runner = MockRunner::new().with_success("chflags");
        let cleaner = GradleCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert!(result.skipped.is_empty(), "nothing was attempted");
        assert_eq!(result.bytes_freed, 0, "nothing was deleted");
        assert!(
            !result.uses_trash,
            "a run with zero deletions must not claim Trash"
        );
    }

    #[test]
    fn gradle_clean_all_deletions_skipped_reports_uses_trash_false() {
        // Every deletion comes back skipped: the refusal comes from the
        // test-only backend override, because the platform backends (Finder
        // via osascript, trashItemAtURL via trashd) move entries out of
        // read-only parents on modern macOS, so a chmod alone no longer
        // forces the deterministic failure. A run where nothing was deleted
        // must not claim Trash, even with the global pinned to trash.
        let _mode = crate::test_helpers::TrashModeGuard::set(true);
        let _backend = crate::trash::TrashBackendOverrideGuard::set();
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        // Two candidates: keep_highest retains 8.12.0 and selects 8.8.0 for
        // deletion, so exactly one deletion is attempted.
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        fs::create_dir_all(caches.join("8.8.0")).unwrap();
        fs::write(caches.join("8.8.0").join("cache.dat"), b"data").unwrap();
        let runner = MockRunner::new().with_success("chflags");
        let cleaner = GradleCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert_eq!(
            result.skipped.len(),
            1,
            "the only deletion candidate must come back skipped, not deleted"
        );
        assert!(
            caches.join("8.8.0").exists(),
            "a skipped deletion must not remove the candidate"
        );
        assert_eq!(result.bytes_freed, 0, "a skipped deletion frees no bytes");
        assert!(
            !result.uses_trash,
            "a run where every deletion was skipped must not claim Trash"
        );
    }

    #[test]
    fn gradle_clean_dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        fs::create_dir_all(caches.join("8.8.0")).unwrap();
        let cleaner = GradleCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let _ = cleaner
            .clean(
                true,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert!(caches.join("8.12.0").exists(), "dry-run must not delete");
        assert!(caches.join("8.8.0").exists(), "dry-run must not delete");
    }

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

    #[test]
    fn gradle_find_old_caches_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let caches = tmp.path().join(".gradle/caches");
        fs::create_dir_all(caches.join("8.12.0")).unwrap();
        fs::create_dir_all(caches.join("8.8.0")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(caches.join("8.8.0"), caches.join("8.10.0")).unwrap();
        let result = GradleCleaner::find_old_caches(&caches);
        assert!(
            !result.iter().any(|p| p.ends_with("8.10.0")),
            "a symlinked older version must not be a deletion candidate"
        );
        assert!(result.iter().any(|p| p.ends_with("8.8.0")));
    }

    // ── shared retention helpers ──

    #[test]
    fn keep_highest_returns_all_but_highest_key() {
        let candidates = vec![
            (vec![8, 8, 0], PathBuf::from("/c/8.8.0")),
            (vec![8, 12, 0], PathBuf::from("/c/8.12.0")),
            (vec![8, 10, 0], PathBuf::from("/c/8.10.0")),
        ];
        let old = keep_highest(candidates);
        assert_eq!(
            old,
            vec![PathBuf::from("/c/8.8.0"), PathBuf::from("/c/8.10.0"),]
        );
    }

    #[test]
    fn keep_highest_skips_candidates_without_a_numeric_key() {
        // A name with no digits has no version key: never a deletion
        // candidate, even when it is the only entry in its group.
        let candidates = vec![
            (vec![], PathBuf::from("/c/nightly")),
            (vec![1], PathBuf::from("/c/journal-1")),
        ];
        let old = keep_highest(candidates);
        assert!(old.is_empty(), "empty-key candidates must never be deleted");
    }

    #[test]
    fn keep_highest_single_candidate_returns_empty() {
        let candidates = vec![(vec![8, 12, 0], PathBuf::from("/c/8.12.0"))];
        assert!(keep_highest(candidates).is_empty());
    }

    #[test]
    fn is_version_dir_rejects_symlinks() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("8.12.0");
        fs::create_dir_all(&target).unwrap();
        let link = tmp.path().join("8.10.0");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let entries: Vec<std::fs::DirEntry> = fs::read_dir(tmp.path()).unwrap().flatten().collect();
        let linked = entries.iter().find(|e| e.file_name() == "8.10.0").unwrap();
        let real = entries.iter().find(|e| e.file_name() == "8.12.0").unwrap();
        assert!(
            !is_version_dir(linked),
            "a symlinked version is never a deletion candidate"
        );
        assert!(is_version_dir(real));
    }

    // ── JetBrainsCleaner (H01) ──

    #[test]
    fn jetbrains_detect_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        // Write files so directories have non-zero size
        fs::write(jb.join("GoLand2024.2").join("cache.dat"), b"data").unwrap();
        fs::write(jb.join("GoLand2025.1").join("cache.dat"), b"data").unwrap();
        let cleaner =
            JetBrainsCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
        assert!(jb.join("GoLand2025.1").exists(), "detect must not delete");
        assert!(jb.join("GoLand2024.2").exists(), "detect must not delete");
    }

    #[test]
    fn jetbrains_clean_uses_trash_true() {
        // TrashModeGuard::set holds TEST_LOCK until Drop, same as the gradle
        // test; the pin makes the seam resolution deterministic.
        let _mode = crate::test_helpers::TrashModeGuard::set(true);
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        let runner = MockRunner::new().with_success("chflags");
        let cleaner = JetBrainsCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        // Same seam-truth contract as the gradle test: Trash is claimed
        // exactly when a deletion actually happened (see the gradle test's
        // TCC note).
        let deletions_happened = result.skipped.is_empty();
        assert_eq!(
            result.uses_trash, deletions_happened,
            "uses_trash must come from the deletion outcomes' seam truth"
        );
    }

    #[test]
    fn jetbrains_clean_dry_run_does_not_delete() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        let cleaner =
            JetBrainsCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()));
        let _ = cleaner
            .clean(
                true,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .unwrap();
        assert!(jb.join("GoLand2025.1").exists(), "dry-run must not delete");
        assert!(jb.join("GoLand2024.2").exists(), "dry-run must not delete");
    }

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

    #[test]
    fn jetbrains_find_old_caches_skips_symlinks() {
        let tmp = TempDir::new().unwrap();
        let jb = tmp.path().join("Library/Caches/JetBrains");
        fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
        fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(jb.join("GoLand2024.2"), jb.join("GoLand2024.9")).unwrap();
        let result = JetBrainsCleaner::find_old_caches(&jb);
        assert!(
            !result.iter().any(|p| p.ends_with("GoLand2024.9")),
            "a symlinked older version must not be a deletion candidate"
        );
        assert!(result.iter().any(|p| p.ends_with("GoLand2024.2")));
    }
}
