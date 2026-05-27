use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::PathBuf;

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
    /// Primary cache directory this cleaner monitors.
    /// Populated when running under --verbose. Otherwise empty.
    pub primary_target: Option<String>,
}

impl ScanResult {
    pub fn new(name: &'static str, status: ScanStatus) -> Self {
        Self {
            name,
            status,
            primary_target: None,
        }
    }

    /// Sets the primary target path unconditionally.
    /// Callers should gate this behind `crate::context::is_verbose()` as needed.
    pub fn with_target(mut self, target: impl Into<String>) -> Self {
        self.primary_target = Some(target.into());
        self
    }
}

#[cfg(test)]
impl ScanResult {
    /// Returns the pruneable byte count, or 0 if not Pruneable.
    pub fn bytes_for_test(&self) -> u64 {
        match &self.status {
            ScanStatus::Pruneable(b) => *b,
            _ => 0,
        }
    }
}

#[derive(Debug)]
pub struct SkippedEntry {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug)]
pub struct CleanResult {
    #[allow(dead_code)]
    pub name: &'static str,
    pub bytes_freed: u64,
    #[allow(dead_code)]
    pub uses_trash: bool,
    pub skipped: Vec<SkippedEntry>,
}

impl CleanResult {
    pub fn exit_code(&self) -> i32 {
        if self.bytes_freed == 0 && !self.skipped.is_empty() {
            1
        } else {
            0
        }
    }
}

#[allow(dead_code)]
pub const LARGE_TRASH_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024;

#[allow(dead_code)]
pub fn format_trash_warning(
    bytes_freed: u64,
    uses_trash: bool,
    is_dry_run: bool,
) -> Option<String> {
    if is_dry_run || !uses_trash || bytes_freed == 0 {
        return None;
    }
    Some(format!(
        "Note: Moved {} to Trash. Run 'Empty Trash' to free disk space.",
        crate::format::format_bytes(bytes_freed)
    ))
}

#[allow(dead_code)]
pub fn format_large_trash_warning(
    bytes_freed: u64,
    uses_trash: bool,
    is_dry_run: bool,
) -> Option<String> {
    if is_dry_run || !uses_trash || bytes_freed < LARGE_TRASH_THRESHOLD_BYTES {
        return None;
    }
    Some("⚠  Large files will be moved to Trash (not immediately freed).".to_string())
}

#[derive(Debug)]
pub struct CleanCancelled;

impl std::fmt::Display for CleanCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cleaning cancelled by user")
    }
}

impl std::error::Error for CleanCancelled {}

pub fn is_skippable_error(e: &anyhow::Error) -> bool {
    if let Some(io_err) = e.downcast_ref::<std::io::Error>() {
        return matches!(
            io_err.kind(),
            std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::WouldBlock
                | std::io::ErrorKind::AlreadyExists
        );
    }
    let msg = format!("{e:#}");
    msg.contains("Permission denied")
        || msg.contains("Operation not permitted")
        || msg.contains("Resource busy")
        || msg.contains("trash failed")
}

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    /// Read-only. Never deletes anything.
    fn detect(&self) -> ScanResult;
    /// Performs cleanup. When `dry_run` is true, must not delete anything.
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
    /// Whether this cleaner is available (tool installed, config exists, etc.).
    /// Defaults to true; override to skip unavailable cleaners during scan.
    fn is_available(&self) -> bool {
        true
    }

    /// Returns sub-targets with display names and sizes (for TUI/CLI).
    fn sub_targets(&self) -> Vec<(&'static str, u64)> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_result_new_has_no_primary_target() {
        let r = ScanResult::new("test", ScanStatus::Clean);
        assert_eq!(r.name, "test");
        assert!(matches!(r.status, ScanStatus::Clean));
        assert!(r.primary_target.is_none());
    }

    #[test]
    fn scan_result_with_target_sets_primary() {
        let r = ScanResult::new("test", ScanStatus::Clean).with_target("/some/path");
        assert_eq!(r.primary_target.as_deref(), Some("/some/path"));
    }

    #[test]
    fn format_trash_warning_returns_none_for_no_bytes() {
        assert!(format_trash_warning(0, true, false).is_none());
    }

    #[test]
    fn format_trash_warning_returns_none_when_not_trash() {
        assert!(format_trash_warning(1024, false, false).is_none());
    }

    #[test]
    fn format_trash_warning_returns_none_for_dry_run() {
        assert!(format_trash_warning(1024, true, true).is_none());
    }

    #[test]
    fn format_trash_warning_returns_message_when_applicable() {
        let msg = format_trash_warning(1024, true, false);
        assert!(msg.is_some());
        let text = msg.unwrap();
        assert!(text.contains("Moved"), "should mention Moved: {text}");
        assert!(
            text.contains("Empty Trash"),
            "should mention Empty Trash: {text}"
        );
        assert!(
            text.contains("1.0 KB"),
            "should show formatted size: {text}"
        );
    }

    #[test]
    fn format_large_trash_warning_returns_none_below_threshold() {
        assert!(format_large_trash_warning(LARGE_TRASH_THRESHOLD_BYTES - 1, true, false).is_none());
    }

    #[test]
    fn format_large_trash_warning_returns_none_when_not_trash() {
        assert!(format_large_trash_warning(LARGE_TRASH_THRESHOLD_BYTES, false, false).is_none());
    }

    #[test]
    fn format_large_trash_warning_returns_message_above_threshold() {
        let msg = format_large_trash_warning(LARGE_TRASH_THRESHOLD_BYTES, true, false);
        assert!(msg.is_some());
        assert!(msg.unwrap().contains("Large files"));
    }
}
