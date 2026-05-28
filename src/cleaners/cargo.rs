use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct CargoCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    target_cache: std::sync::OnceLock<Vec<(PathBuf, u64)>>,
}

impl CargoCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            target_cache: std::sync::OnceLock::new(),
        }
    }

    /// Returns the cached target dirs, computing them on first call.
    fn get_target_dirs(&self) -> &Vec<(PathBuf, u64)> {
        self.target_cache
            .get_or_init(|| Self::find_target_dirs(&self.home))
    }

    fn find_target_dirs(home: &Path) -> Vec<(PathBuf, u64)> {
        let mut targets = vec![];
        for entry in walkdir::WalkDir::new(home)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let fname = entry.file_name().to_string_lossy();
            if fname == "target" && entry.file_type().is_dir() {
                let path = entry.path();
                if path.components().any(|c| c.as_os_str() == ".cargo") {
                    continue;
                }
                let size = dir_size(path);
                targets.push((path.to_path_buf(), size));
            }
        }
        targets
    }
}

impl Cleaner for CargoCleaner {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn detect(&self) -> ScanResult {
        let reg = self.home.join(".cargo/registry/cache");
        let reg_size = if reg.exists() {
            let s = dir_size(&reg);
            println!("[cargo] registry cache: {}", crate::format::format_bytes(s));
            s
        } else {
            0
        };

        let targets = self.get_target_dirs();
        let target_size: u64 = targets.iter().map(|(_, s)| s).sum();
        if !targets.is_empty() {
            println!("[cargo] found {} target/ directory(ies)", targets.len());
        }

        let total = reg_size + target_size;
        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            // Report the Cargo registry cache as primary target.
            r = r.with_target(reg.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let mut skipped: Vec<crate::cleaner::SkippedEntry> = vec![];
        let mut freed: u64 = 0;

        let reg = self.home.join(".cargo/registry/cache");
        if reg.exists() {
            let size = dir_size(&reg);
            if dry_run {
                println!(
                    "[dry-run] [cargo] would remove registry cache: {} ({})",
                    reg.display(),
                    crate::format::format_bytes(size)
                );
            } else {
                self.runner
                    .run("chflags", &["-R", "nouchg", &reg.to_string_lossy()])
                    .ok();
                if let Err(e) = crate::trash::delete_path(&reg) {
                    if crate::cleaner::is_skippable_error(&e) {
                        skipped.push(crate::cleaner::SkippedEntry {
                            path: reg.to_path_buf(),
                            reason: format!("{e:#}"),
                        });
                    } else {
                        return Err(e);
                    }
                } else {
                    freed += size;
                    println!("[cargo] removed registry cache: {}", reg.display());
                }
            }
        }

        let targets = self.get_target_dirs();
        if !dry_run && !targets.is_empty() {
            reporter.progress_init(self.name(), targets.len());
        }
        for (i, (path, size)) in targets.iter().enumerate() {
            if dry_run {
                println!(
                    "[dry-run] [cargo] would remove target dir: {} ({})",
                    path.display(),
                    crate::format::format_bytes(*size)
                );
            } else {
                reporter.progress_tick(path, i + 1, *size);
                self.runner
                    .run("chflags", &["-R", "nouchg", &path.to_string_lossy()])
                    .ok();
                if let Err(e) = crate::trash::delete_path(path) {
                    if crate::cleaner::is_skippable_error(&e) {
                        skipped.push(crate::cleaner::SkippedEntry {
                            path: path.to_path_buf(),
                            reason: format!("{e:#}"),
                        });
                    } else {
                        return Err(e);
                    }
                } else {
                    freed += size;
                    println!("[cargo] removed target dir: {}", path.display());
                }
            }
        }
        if !dry_run && !targets.is_empty() {
            reporter.progress_finish();
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: true,
            skipped,
            deleted_paths: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::SystemCommandRunner;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn detect_includes_primary_target_when_verbose() {
        let _guard = crate::test_helpers::VerboseGuard::new();
        let tmp = TempDir::new().unwrap();
        // Create registry cache so the cleaner reports Pruneable
        let reg = tmp.path().join(".cargo/registry/cache/pkg");
        fs::create_dir_all(&reg).unwrap();
        fs::write(reg.join("dummy.crate"), b"x").unwrap();

        let cleaner = CargoCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
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
                .contains(".cargo/registry/cache"),
            "target should point to registry cache"
        );
    }

    #[test]
    fn detect_omits_primary_target_when_not_verbose() {
        let _guard = crate::test_helpers::VerboseGuard::with_value(false);
        let tmp = TempDir::new().unwrap();
        let reg = tmp.path().join(".cargo/registry/cache/pkg");
        fs::create_dir_all(&reg).unwrap();
        fs::write(reg.join("dummy.crate"), b"x").unwrap();

        let cleaner = CargoCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
        let result = cleaner.detect();
        assert!(result.primary_target.is_none());
    }

    #[test]
    fn find_target_dirs_empty_home_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let result = CargoCleaner::find_target_dirs(tmp.path());
        assert!(result.is_empty());
    }

    #[test]
    fn find_target_dirs_finds_single_target_dir() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("my-project/target");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("dummy.o"), b"x").unwrap();
        let result = CargoCleaner::find_target_dirs(tmp.path());
        assert_eq!(result.len(), 1);
        assert!(result[0].0.ends_with("my-project/target"));
    }

    #[test]
    fn find_target_dirs_excludes_cargo_registry() {
        let tmp = TempDir::new().unwrap();
        // Create .cargo/registry (should be excluded)
        let reg = tmp.path().join(".cargo/registry/cache/pkg");
        fs::create_dir_all(&reg).unwrap();
        fs::write(reg.join("dummy.crate"), b"x").unwrap();
        // And a real project target dir
        let real = tmp.path().join("my-project/target");
        fs::create_dir_all(&real).unwrap();
        fs::write(real.join("dummy.o"), b"x").unwrap();

        let result = CargoCleaner::find_target_dirs(tmp.path());
        assert!(!result.is_empty());
        for (path, _) in &result {
            assert!(
                !path.to_string_lossy().contains(".cargo"),
                "paths containing .cargo must be excluded: {}",
                path.display()
            );
        }
    }

    #[test]
    fn find_target_dirs_respects_max_depth() {
        let tmp = TempDir::new().unwrap();
        // Create a shallow target dir at depth 2
        let shallow = tmp.path().join("a/target");
        fs::create_dir_all(&shallow).unwrap();
        fs::write(shallow.join("x.o"), b"x").unwrap();
        // Create a deep target dir at depth 6 (beyond max_depth=5)
        let deep = tmp.path().join("a/b/c/d/e/target");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("y.o"), b"x").unwrap();

        let result = CargoCleaner::find_target_dirs(tmp.path());
        assert!(
            result.iter().any(|(p, _)| p.ends_with("a/target")),
            "shallow target at depth 2 should be found"
        );
        assert!(
            !result.iter().any(|(p, _)| p.ends_with("a/b/c/d/e/target")),
            "deep target at depth 6 must be excluded by max_depth=5"
        );
    }
}
