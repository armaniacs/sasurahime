use std::sync::atomic::{AtomicBool, Ordering};

/// Serializes tests that read/write the global VERBOSE/DRY_RUN flags.
/// Without this, parallel tests can race on the AtomicBool values.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// When set, cleaners emit detailed file/dir and command output.
static VERBOSE: AtomicBool = AtomicBool::new(false);

/// When set, cleaners perform dry-run (no actual deletion).
static DRY_RUN: AtomicBool = AtomicBool::new(false);

pub fn set_verbose(v: bool) {
    VERBOSE.store(v, Ordering::Relaxed);
}

pub fn is_verbose() -> bool {
    VERBOSE.load(Ordering::Relaxed)
}

pub fn set_dry_run(v: bool) {
    DRY_RUN.store(v, Ordering::Relaxed);
}

/// `#[allow(dead_code)]`: reserved for future `--dry-run` chain support.
#[allow(dead_code)]
pub fn is_dry_run() -> bool {
    DRY_RUN.load(Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper that acquires the global test lock, runs a closure, and releases.
    fn with_test_lock<F, T>(f: F) -> T
    where
        F: FnOnce() -> T,
    {
        let _guard = crate::context::TEST_LOCK.lock().unwrap();
        f()
    }

    #[test]
    fn verbose_defaults_to_false() {
        with_test_lock(|| {
            set_verbose(false);
            assert!(!is_verbose());
        });
    }

    #[test]
    fn set_verbose_true_makes_is_verbose_true() {
        with_test_lock(|| {
            set_verbose(true);
            assert!(is_verbose());
            set_verbose(false);
        });
    }

    #[test]
    fn set_verbose_false_makes_is_verbose_false() {
        with_test_lock(|| {
            set_verbose(true);
            set_verbose(false);
            assert!(!is_verbose());
        });
    }

    #[test]
    fn dry_run_defaults_to_false() {
        with_test_lock(|| {
            set_dry_run(false);
            assert!(!is_dry_run());
        });
    }

    #[test]
    fn set_dry_run_true_makes_is_dry_run_true() {
        with_test_lock(|| {
            set_dry_run(true);
            assert!(is_dry_run());
            set_dry_run(false);
        });
    }
}
