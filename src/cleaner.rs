use crate::progress::ProgressReporter;
use anyhow::Result;

#[derive(Debug, Clone)]
pub enum ScanStatus {
    /// Bytes available to reclaim.
    Pruneable(u64),
    Clean,
    NotFound,
    #[allow(dead_code)]
    PermissionDenied,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub name: &'static str,
    pub status: ScanStatus,
}

#[derive(Debug)]
pub struct CleanResult {
    #[allow(dead_code)]
    pub name: &'static str,
    pub bytes_freed: u64,
}

#[derive(Debug)]
pub struct CleanCancelled;

impl std::fmt::Display for CleanCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cleaning cancelled by user")
    }
}

impl std::error::Error for CleanCancelled {}

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    /// Read-only. Never deletes anything.
    fn detect(&self) -> ScanResult;
    /// Performs cleanup. When `dry_run` is true, must not delete anything.
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
}
