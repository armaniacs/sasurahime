use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ScanStatus {
    /// Bytes available to reclaim.
    Pruneable(u64),
    Clean,
    NotFound,
    /// The target is present but could not be measured (permission denied,
    /// external command failed or timed out). Never counts toward totals.
    Unavailable,
    /// The tool is installed but detect cannot measure a byte count without
    /// running it (CLI-delegating cleaners). Displayed as `unknown` and
    /// never counted toward totals.
    Unknown,
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
    /// Cleaner name — pub field for downstream inspection.
    #[cfg_attr(not(test), expect(dead_code, reason = "pub API — callers may read"))]
    pub name: &'static str,
    pub bytes_freed: u64,
    /// Whether the operation used macOS Trash (vs permanent deletion).
    pub uses_trash: bool,
    pub skipped: Vec<SkippedEntry>,
    /// Paths that were successfully deleted (populated when --verbose is active).
    /// Primarily used for audit trail in --permanent mode.
    #[expect(dead_code, reason = "pub API — callers may read")]
    pub deleted_paths: Vec<PathBuf>,
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

/// Options for fine-grained control over clean operations.
/// The default (`all: false`) preserves existing behavior.
#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct CleanOptions {
    pub all: bool,
    /// Include Moderate/Risky/Manual targets in a `clean --all` run. Without
    /// this, only `Safe` (auto-clean-allowed) targets are cleaned.
    pub include_manual: bool,
    /// Global age filter (days): only entries older than this are cleaned.
    /// Combines with a cleaner's own filter (the stricter wins).
    pub older_than_days: Option<u32>,
    /// Global size filter (MB): only dirs of at least this size are cleaned.
    pub larger_than_mb: Option<u64>,
    /// Deletion mode carried as an argument. `None` reads the process-global
    /// mode (compat); `Some(v)` pins the mode so `uses_trash` and the actual
    /// deletion can never disagree. `main` sets it once per run.
    pub trash_mode: Option<bool>,
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
    msg.contains("trash failed")
        || msg.starts_with("Permission denied")
        || msg.starts_with("Operation not permitted")
        || msg.starts_with("Resource busy")
}

/// Which `[cleaner.<name>]` filters a cleaner actually honours. Command-based
/// cleaners never look at them; `logs` only reads `older_than_days`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerCleanerFilters {
    None,
    OlderThanDays,
    Both,
}

/// Risk classification of a clean target. One classification decides
/// `--yes` behavior, TUI initial selection and the scan display.
///
/// The default is `Moderate` so a new target can never silently become
/// safe to clean unattended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    /// Regenerable developer caches: safe to clean unattended.
    Safe,
    /// Large or partially user-visible data: requires explicit selection.
    Moderate,
    /// Destructive or irreversible operations.
    Risky,
    /// User data: cleaned only when explicitly selected.
    Manual,
}

impl Risk {
    /// Label used by the scan table, `scan --json` and the TUI.
    pub fn label(self) -> &'static str {
        match self {
            Risk::Safe => "Safe",
            Risk::Moderate => "Moderate",
            Risk::Risky => "Risky",
            Risk::Manual => "Manual",
        }
    }
}

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    /// Read-only. Never deletes anything.
    fn detect(&self) -> ScanResult;
    /// Performs cleanup. When `dry_run` is true, must not delete anything.
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
    /// Clean with additional options. Default impl calls `clean()` ignoring opts.
    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.clean(dry_run, reporter)
    }
    /// Whether this cleaner is available (tool installed, config exists, etc.).
    /// Defaults to true; override to skip unavailable cleaners during scan.
    fn is_available(&self) -> bool {
        true
    }

    /// Risk classification of this target. Defaults to `Moderate`: unclassified
    /// targets are never cleaned unattended.
    fn risk(&self) -> Risk {
        Risk::Moderate
    }

    /// Whether `--yes` (run_auto) may clean this target. Derived from
    /// `risk()`: only `Risk::Safe` targets run unattended.
    fn auto_clean_allowed(&self) -> bool {
        self.risk() == Risk::Safe
    }

    /// Read-only note shown under the scan table. Must not run external commands.
    fn advisory(&self) -> Option<String> {
        None
    }

    /// Which per-cleaner filters this cleaner honours, for startup warnings
    /// about `[cleaner.<name>]` settings that would otherwise be ignored.
    fn per_cleaner_filters(&self) -> PerCleanerFilters {
        PerCleanerFilters::None
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
    fn is_skippable_error_permission_denied() {
        let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
        assert!(is_skippable_error(&e));
    }

    // ── Risk ──

    struct StubCleaner {
        risk: Option<Risk>,
    }
    impl Cleaner for StubCleaner {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new("stub", ScanStatus::NotFound)
        }
        fn clean(&self, _: bool, _: &dyn ProgressReporter) -> Result<CleanResult> {
            unimplemented!()
        }
        fn risk(&self) -> Risk {
            self.risk.unwrap_or(Risk::Moderate)
        }
    }

    #[test]
    fn risk_defaults_to_moderate() {
        let c = StubCleaner { risk: None };
        assert_eq!(
            c.risk(),
            Risk::Moderate,
            "unclassified targets are Moderate"
        );
        assert!(!c.auto_clean_allowed(), "Moderate never runs unattended");
    }

    #[test]
    fn risk_orders_safe_before_manual() {
        assert!(Risk::Safe < Risk::Moderate);
        assert!(Risk::Moderate < Risk::Risky);
        assert!(Risk::Risky < Risk::Manual);
    }

    #[test]
    fn risk_labels_match_display_vocabulary() {
        assert_eq!(Risk::Safe.label(), "Safe");
        assert_eq!(Risk::Moderate.label(), "Moderate");
        assert_eq!(Risk::Risky.label(), "Risky");
        assert_eq!(Risk::Manual.label(), "Manual");
    }

    #[test]
    fn auto_clean_allowed_derives_from_safe_risk() {
        for (risk, allowed) in [
            (Risk::Safe, true),
            (Risk::Moderate, false),
            (Risk::Risky, false),
            (Risk::Manual, false),
        ] {
            let c = StubCleaner { risk: Some(risk) };
            assert_eq!(c.auto_clean_allowed(), allowed, "risk {risk:?}");
        }
    }

    #[test]
    fn is_skippable_error_would_block() {
        let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_already_exists() {
        let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_not_found_is_not_skippable() {
        let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(!is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_connection_refused_is_not_skippable() {
        let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
        assert!(!is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_permission_denied_string() {
        let e = anyhow::anyhow!("Permission denied: /some/path");
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_operation_not_permitted_string() {
        let e = anyhow::anyhow!("Operation not permitted");
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_resource_busy_string() {
        let e = anyhow::anyhow!("Resource busy");
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_trash_failed_string() {
        let e = anyhow::anyhow!("trash failed: could not move to trash");
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_arbitrary_error_is_not_skippable() {
        let e = anyhow::anyhow!("something went horribly wrong");
        assert!(!is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_clean_cancelled_is_not_skippable() {
        let e = anyhow::Error::from(crate::cleaner::CleanCancelled);
        assert!(!is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_false_positive_permission_denied_in_filename() {
        let e = anyhow::anyhow!("Database says: Operation not permitted in current mode");
        assert!(
            !is_skippable_error(&e),
            "sentence with 'Operation not permitted' => not skippable"
        );
    }

    #[test]
    fn is_skippable_error_permission_denied_sentence_not_io_error() {
        let e = anyhow::anyhow!("Error: user lacks 'Permission denied' access to resource");
        assert!(
            !is_skippable_error(&e),
            "sentence with 'Permission denied' substring => not skippable"
        );
    }

    #[test]
    fn is_skippable_error_trash_failed_in_error_message() {
        let e = anyhow::anyhow!("trash failed: /path/to/file");
        assert!(is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_resource_busy_in_unrelated_error() {
        let e = anyhow::anyhow!("The device reports 'Resource busy' in its status string");
        assert!(
            !is_skippable_error(&e),
            "sentence with 'Resource busy' substring => not skippable"
        );
    }

    #[test]
    fn is_skippable_error_unrelated_error_without_skip_keywords() {
        let e = anyhow::anyhow!("Disk full: cannot write to /dev/sda1");
        assert!(!is_skippable_error(&e));
    }

    #[test]
    fn is_skippable_error_filesystem_corruption() {
        let e = anyhow::anyhow!("Input/output error: critical filesystem corruption detected");
        assert!(!is_skippable_error(&e));
    }
}
