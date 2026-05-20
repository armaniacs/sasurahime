use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct CargoCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl CargoCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
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

        let targets = Self::find_target_dirs(&self.home);
        let target_size: u64 = targets.iter().map(|(_, s)| s).sum();
        if !targets.is_empty() {
            println!("[cargo] found {} target/ directory(ies)", targets.len());
        }

        let total = reg_size + target_size;
        ScanResult {
            name: self.name(),
            status: if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
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
                crate::trash::delete_path(&reg)?;
                freed += size;
                println!("[cargo] removed registry cache: {}", reg.display());
            }
        }

        let targets = Self::find_target_dirs(&self.home);
        for (path, size) in &targets {
            if dry_run {
                println!(
                    "[dry-run] [cargo] would remove target dir: {} ({})",
                    path.display(),
                    crate::format::format_bytes(*size)
                );
            } else {
                self.runner
                    .run("chflags", &["-R", "nouchg", &path.to_string_lossy()])
                    .ok();
                crate::trash::delete_path(path)?;
                freed += size;
                println!("[cargo] removed target dir: {}", path.display());
            }
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
        })
    }
}
