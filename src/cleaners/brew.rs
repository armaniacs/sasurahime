#![allow(dead_code)]

use std::path::{Path, PathBuf};
use crate::cleaner::{Cleaner, CleanResult, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;

pub struct BrewCleaner {
    cache_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl BrewCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            cache_dir: home.join("Library/Caches/Homebrew"),
            runner,
        }
    }
}

impl Cleaner for BrewCleaner {
    fn name(&self) -> &'static str {
        "brew"
    }

    fn detect(&self) -> ScanResult {
        if !self.cache_dir.exists() {
            return ScanResult { name: self.name(), status: ScanStatus::NotFound };
        }
        let bytes = dir_size(&self.cache_dir);
        ScanResult {
            name: self.name(),
            status: if bytes > 0 { ScanStatus::Pruneable(bytes) } else { ScanStatus::Clean },
        }
    }

    fn clean(&self, _dry_run: bool) -> Result<CleanResult> {
        // Full implementation in Task 4
        todo!("BrewCleaner::clean not yet implemented")
    }
}
