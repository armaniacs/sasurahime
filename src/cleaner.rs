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
    /// Why the entry was skipped: a typed deletion failure (the seam's
    /// classification) or a prose reason (a blocked app, a vanished path).
    pub reason: SkipCause,
}

/// Why an entry was skipped. The deletion arm carries the seam's typed
/// [`DeleteError`] so the skip/fatal classification is matched on by variant,
/// never by message text: rewording an error message cannot flip a skip into
/// a fatal failure (PBI 59).
#[derive(Debug)]
pub enum SkipCause {
    /// The seam classified the deletion failure as skippable (a refused
    /// trash move, a permission denial, a busy target).
    Delete(DeleteError),
    /// A prose reason: a blocked app, a vanished path, a config mismatch.
    Reason(String),
}

impl std::fmt::Display for SkipCause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipCause::Delete(err) => write!(f, "{err}"),
            SkipCause::Reason(reason) => write!(f, "{reason}"),
        }
    }
}

/// A deletion failure, classified by cause instead of by message text (PBI
/// 59): a refusal is [`DeleteError::is_skippable`] by variant, so rewording
/// the Display text can never flip a skip into a fatal error.
#[derive(Debug)]
pub enum DeleteError {
    /// The trash backend (trashItemAtURL / Finder) refused to move the entry.
    TrashRefused(String),
    /// The filesystem denied the removal (EPERM / EACCES).
    PermissionDenied(std::io::Error),
    /// The target was busy or otherwise unremovable-but-expected: the
    /// errnos EAGAIN, EBUSY, and EEXIST, matched at kind level as
    /// `WouldBlock`, `ResourceBusy`, and `AlreadyExists` respectively by
    /// [`classify_io_error`].
    Busy(std::io::Error),
    /// Any other deletion failure: fatal, not classified.
    Io(std::io::Error),
}

impl DeleteError {
    /// True when the clean run should continue past this failure (the entry
    /// was NOT deleted): a refused trash move, a permission denial, or a
    /// busy target. Everything else is fatal.
    pub fn is_skippable(&self) -> bool {
        match self {
            DeleteError::TrashRefused(_)
            | DeleteError::PermissionDenied(_)
            | DeleteError::Busy(_) => true,
            DeleteError::Io(_) => false,
        }
    }
}

impl std::fmt::Display for DeleteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeleteError::TrashRefused(msg) => write!(f, "trash failed: {msg}"),
            DeleteError::PermissionDenied(e) | DeleteError::Busy(e) | DeleteError::Io(e) => {
                write!(f, "{e}")
            }
        }
    }
}

impl std::error::Error for DeleteError {}

/// Classifies a filesystem error by kind: a permission denial is a skip, a
/// busy target is a skip, everything else is fatal.
///
/// The errno-to-kind mapping follows the Rust toolchain, not macOS:
/// EAGAIN becomes `ErrorKind::WouldBlock` and EBUSY becomes
/// `ErrorKind::ResourceBusy`, although older std releases mapped EBUSY to
/// `WouldBlock` — matching both kinds keeps the behavior safe either way.
/// EEXIST becomes `ErrorKind::AlreadyExists`, grouped with the busy skips
/// because on the delete path EEXIST is expected rather than fatal: a
/// racing clean may have already removed (or recreated) the candidate, so
/// skipping it loses nothing.
pub fn classify_io_error(e: std::io::Error) -> DeleteError {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => DeleteError::PermissionDenied(e),
        std::io::ErrorKind::WouldBlock
        | std::io::ErrorKind::ResourceBusy
        | std::io::ErrorKind::AlreadyExists => DeleteError::Busy(e),
        _ => DeleteError::Io(e),
    }
}

#[derive(Debug)]
pub struct CleanResult {
    /// Cleaner name — pub field for downstream inspection.
    pub name: &'static str,
    pub bytes_freed: u64,
    /// Whether the operation used macOS Trash (vs permanent deletion).
    pub uses_trash: bool,
    pub skipped: Vec<SkippedEntry>,
    /// Paths that were successfully deleted (populated when --verbose is active).
    /// Primarily used for audit trail in --permanent mode.
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

    /// A result reporting `bytes_freed` freed with the seam-resolved trash
    /// truth. Prefer this over hand-rolled struct literals so the
    /// `uses_trash` decision has one owner per call site.
    pub fn freed(name: &'static str, bytes_freed: u64, uses_trash: bool) -> Self {
        Self {
            name,
            bytes_freed,
            uses_trash,
            skipped: Vec::new(),
            deleted_paths: Vec::new(),
        }
    }

    /// A result for "nothing happened": a tool that is not installed, a
    /// cancelled run, a no-op target. Zero freed, nothing deleted.
    pub fn empty(name: &'static str) -> Self {
        Self {
            name,
            bytes_freed: 0,
            uses_trash: false,
            skipped: Vec::new(),
            deleted_paths: Vec::new(),
        }
    }
}

/// Options for fine-grained control over clean operations.
/// The default (`all: false`) preserves existing behavior.
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
    /// deletion can never disagree. `main` sets the global once per run.
    pub trash_mode: Option<crate::trash::TrashMode>,
}

#[derive(Debug)]
pub struct CleanCancelled;

impl std::fmt::Display for CleanCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cleaning cancelled by user")
    }
}

impl std::error::Error for CleanCancelled {}

/// The in-process clean seam (PBI-50): the binary wires this callback to the
/// same dispatch the `clean <target>` CLI uses —
/// `(target name, optional sub-target, dry_run, reporter) -> CleanResult`.
/// It must return the REAL CleanResult (never a fabricated zero) so the
/// caller's `Total freed` reflects actual bytes. `sub = None` is a
/// whole-target clean; `Some(sub)` is the sub-target dispatch (xcode only).
pub type CleanByName<'a> =
    dyn Fn(&str, Option<&str>, bool, &dyn ProgressReporter) -> Result<CleanResult> + 'a;

/// Which `[cleaner.<name>]` filters a cleaner actually honours. Command-based
/// cleaners never look at them; a cleaner's `per_cleaner_filters()` override
/// is the one owner of this fact and `main`'s ineffective-setting warning
/// reads it verbatim. (`library-logs` merges the global `CleanOptions::
/// older_than_days` into its own recency heuristic — an orthogonal mechanism
/// that never comes from a `[cleaner.<name>]` section.)
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

    /// Whether this cleaner's default-options clean path (what `--yes` runs:
    /// the batch with `CleanOptions::default`) gates on an interactive prompt
    /// (`prompt::gate_interactive`) and therefore skips on a non-TTY stdin
    /// even when `auto_clean_allowed()` selects it. Defaults to false; the
    /// cleaners that gate on that exact path override it so run_auto can
    /// name them in its skipped-targets notice (PBI 60).
    fn interactive_gated(&self) -> bool {
        false
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

// Builtin target names that live outside the main.rs macro table (shared with
// the library's correspondence-table tests and `main`'s PBI metadata).
pub fn extra_targets() -> &'static [(&'static str, &'static str)] {
    &[
        (
            "caches",
            "All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)",
        ),
        ("logs", "Log files older than N days"),
        ("xcode", "Xcode DerivedData project directories"),
        ("trash", "~/.Trash size (scan only)"),
        ("ollama", "Ollama model cache"),
        (
            "library-logs",
            "Analyze and clean ~/Library/Logs/ with heuristic recommendations",
        ),
        ("device-support", "Xcode DeviceSupport old version cleanup"),
        (
            "ios-backup",
            "iOS device backups (irreversible — backed up to Trash)",
        ),
        (
            "apfs-snapshot",
            "APFS local Time Machine snapshots (tmutil deletelocalsnapshot)",
        ),
        (
            "vm-swap",
            "/private/var/vm swap files (report only; re-initialized on reboot)",
        ),
        (
            "all",
            "Explicit full clean (Safe targets; --include-manual for the rest)",
        ),
        (
            "project-artifacts",
            "Stale project artifacts (node_modules / target / .build with manifests)",
        ),
    ]
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
    fn delete_error_trash_refused_is_skippable() {
        let e = DeleteError::TrashRefused("could not move to trash".to_string());
        assert!(e.is_skippable());
    }

    #[test]
    fn delete_error_permission_denied_is_skippable() {
        let e = DeleteError::PermissionDenied(std::io::Error::from(
            std::io::ErrorKind::PermissionDenied,
        ));
        assert!(e.is_skippable());
    }

    #[test]
    fn delete_error_busy_is_skippable() {
        let e = DeleteError::Busy(std::io::Error::from(std::io::ErrorKind::WouldBlock));
        assert!(e.is_skippable());
    }

    #[test]
    fn delete_error_io_is_not_skippable() {
        let e = DeleteError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(!e.is_skippable(), "an unclassified io error is fatal");
    }

    #[test]
    fn delete_error_rewording_does_not_flip_the_classification() {
        // PBI 59 acceptance: rewording the Display text changes nothing. Two
        // TrashRefused errors with completely different messages classify
        // the same.
        let a = DeleteError::TrashRefused("trash failed: could not move to trash".to_string());
        let b = DeleteError::TrashRefused("totally different wording entirely".to_string());
        assert_eq!(a.is_skippable(), b.is_skippable());
    }

    #[test]
    fn classify_io_error_maps_the_kinds() {
        // The one io-kind mapping (PBI 62): the four skippable kinds the
        // removed `is_skippable_error` duplicated live only here — a
        // permission denial is a permission skip, a busy target is a busy
        // skip (EAGAIN maps to WouldBlock and EBUSY to ResourceBusy; older
        // std releases mapped EBUSY to WouldBlock, so both kinds are
        // matched, and EEXIST maps to AlreadyExists), and everything else
        // is fatal.
        for kind in [
            std::io::ErrorKind::PermissionDenied,
            std::io::ErrorKind::WouldBlock,
            std::io::ErrorKind::ResourceBusy,
            std::io::ErrorKind::AlreadyExists,
        ] {
            let e = classify_io_error(std::io::Error::from(kind));
            assert!(e.is_skippable(), "{kind:?} must stay skippable");
        }
        for kind in [
            std::io::ErrorKind::NotFound,
            std::io::ErrorKind::ConnectionRefused,
        ] {
            let e = classify_io_error(std::io::Error::from(kind));
            assert!(!e.is_skippable(), "{kind:?} must stay fatal");
        }
    }

    #[test]
    fn classify_io_error_matches_the_kind_never_the_message_text() {
        // PBI 59/62: classification is by io ErrorKind, never by message
        // text, so a misleading message cannot flip a fatal kind into a
        // skip (the old prose test's subject, made typed).
        let misleading = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Permission denied: /some/path",
        );
        let e = classify_io_error(misleading);
        assert!(
            matches!(e, DeleteError::Io(_)),
            "a non-skippable kind stays fatal regardless of message text: {e}"
        );
        assert!(!e.is_skippable());
    }
}
