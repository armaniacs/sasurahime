#![allow(dead_code)]

use std::path::{Path, PathBuf};
use crate::cleaner::{Cleaner, CleanResult, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;

pub struct UvCleaner {
    cache_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl UvCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            cache_dir: home.join(".cache/uv"),
            runner,
        }
    }
}

impl Cleaner for UvCleaner {
    fn name(&self) -> &'static str {
        "uv"
    }

    fn detect(&self) -> ScanResult {
        let archive = self.cache_dir.join("archive-v0");
        if !self.cache_dir.exists() {
            return ScanResult { name: self.name(), status: ScanStatus::NotFound };
        }
        let bytes = dir_size(&archive);
        ScanResult {
            name: self.name(),
            status: if bytes > 0 { ScanStatus::Pruneable(bytes) } else { ScanStatus::Clean },
        }
    }

    fn clean(&self, _dry_run: bool) -> Result<CleanResult> {
        // Full implementation in Task 3
        todo!("UvCleaner::clean not yet implemented")
    }
}
