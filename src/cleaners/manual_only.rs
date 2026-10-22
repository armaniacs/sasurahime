use crate::cleaner::{CleanOptions, CleanResult, Cleaner, ScanResult};
use crate::progress::ProgressReporter;
use anyhow::Result;

/// Wraps a cleaner so that `--yes` never selects it. Scan, interactive
/// selection and explicit `clean <target>` are unaffected.
pub struct ManualOnly {
    inner: Box<dyn Cleaner>,
}

impl ManualOnly {
    pub fn new(inner: Box<dyn Cleaner>) -> Self {
        Self { inner }
    }
}

impl Cleaner for ManualOnly {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn detect(&self) -> ScanResult {
        self.inner.detect()
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        self.inner.clean(dry_run, reporter)
    }

    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.inner.clean_with_opts(dry_run, reporter, opts)
    }

    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    fn risk(&self) -> crate::cleaner::Risk {
        // Display passthrough: the wrapper shows the inner target's own risk
        // (Moderate, Risky, ...) so the scan table stays informative.
        self.inner.risk()
    }

    fn sub_targets(&self) -> Vec<(&'static str, u64)> {
        self.inner.sub_targets()
    }

    fn advisory(&self) -> Option<String> {
        self.inner.advisory()
    }

    fn auto_clean_allowed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::ScanStatus;

    struct Stub;

    impl Cleaner for Stub {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new("stub", ScanStatus::Pruneable(7))
        }
        fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
            Ok(CleanResult {
                name: "stub",
                bytes_freed: 7,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            })
        }
    }

    #[test]
    fn default_cleaner_blocks_auto_clean() {
        // The trait default risk is Moderate, so an unclassified cleaner is
        // never cleaned unattended.
        assert!(!Stub.auto_clean_allowed());
        assert_eq!(crate::cleaner::Risk::Moderate, Stub.risk());
    }

    #[test]
    fn manual_only_blocks_auto_clean() {
        let c = ManualOnly::new(Box::new(Stub));
        assert!(!c.auto_clean_allowed());
    }

    #[test]
    fn manual_only_delegates_risk_for_display() {
        let c = ManualOnly::new(Box::new(Stub));
        assert_eq!(
            Stub.risk(),
            c.risk(),
            "wrapper must pass the inner risk through"
        );
    }

    #[test]
    fn manual_only_delegates_name_detect_and_clean() {
        let c = ManualOnly::new(Box::new(Stub));
        assert_eq!(c.name(), "stub");
        assert!(matches!(c.detect().status, ScanStatus::Pruneable(7)));
        let reporter = crate::progress::VerboseProgress::new();
        assert_eq!(c.clean(false, &reporter).unwrap().bytes_freed, 7);
    }
}
