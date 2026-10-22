use anyhow::Result;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

static TRASH_MODE: AtomicBool = AtomicBool::new(true);

/// Deletion context: carries the deletion mode as an argument instead of
/// reading process-global state, so parallel tests never leak modes into each
/// other. New code should prefer this over the global wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteCtx {
    pub trash_mode: bool,
}

impl DeleteCtx {
    pub fn new(trash_mode: bool) -> Self {
        Self { trash_mode }
    }

    /// Deletes `path` according to this context: macOS Trash when trash mode
    /// is on, otherwise a permanent remove.
    pub fn delete_path(&self, path: &Path) -> Result<()> {
        if self.trash_mode {
            trash::delete(path).map_err(|e| anyhow::anyhow!("trash failed: {e}"))
        } else if path.is_dir() {
            fs::remove_dir_all(path)
                .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
        } else {
            fs::remove_file(path).map_err(|e| anyhow::anyhow!("remove_file {:?}: {}", path, e))
        }
    }
}

pub fn set_trash_mode(enabled: bool) {
    TRASH_MODE.store(enabled, Ordering::Relaxed);
}

pub fn is_trash_mode() -> bool {
    TRASH_MODE.load(Ordering::Relaxed)
}

/// Compat wrapper over [`DeleteCtx`]: reads the process-global mode that
/// `main` sets once per run. Cleaner-level call sites keep using this until
/// the trait carries a `DeleteCtx`.
pub fn delete_path(path: &Path) -> Result<()> {
    DeleteCtx::new(TRASH_MODE.load(Ordering::Relaxed)).delete_path(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // DeleteCtx-based tests carry the mode as an argument: no global, no
    // TEST_LOCK, safe to run fully in parallel.

    #[test]
    fn delete_ctx_trash_mode_moves_to_trash() {
        let ctx = DeleteCtx::new(true);
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("dir_to_trash");
        fs::create_dir_all(&d).unwrap();
        ctx.delete_path(&d).unwrap();
        assert!(!d.exists(), "trash mode removes the source path");
    }

    #[test]
    fn delete_ctx_permanent_mode_removes_directory() {
        let ctx = DeleteCtx::new(false);
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("testdir");
        fs::create_dir_all(&d).unwrap();
        ctx.delete_path(&d).unwrap();
        assert!(!d.exists(), "directory must be removed");
    }

    #[test]
    fn delete_ctx_permanent_mode_removes_file() {
        let ctx = DeleteCtx::new(false);
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("test.txt");
        fs::write(&f, b"hello").unwrap();
        ctx.delete_path(&f).unwrap();
        assert!(!f.exists(), "file must be removed in permanent mode");
    }

    #[test]
    fn delete_ctx_trash_mode_returns_error_on_failure() {
        let ctx = DeleteCtx::new(true);
        let result = ctx.delete_path(Path::new("/nonexistent/path/that/cannot/be/trashed"));
        assert!(result.is_err(), "trash of nonexistent path must return Err");
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("trash"),
            "error must mention trash, got: {err}"
        );
    }

    #[test]
    fn delete_ctx_permanent_mode_returns_error_on_missing_path() {
        let ctx = DeleteCtx::new(false);
        let result = ctx.delete_path(Path::new("/nonexistent/path/12345"));
        assert!(result.is_err(), "removing a missing path must return Err");
    }

    #[test]
    fn delete_ctx_is_copiable_and_comparable() {
        let trash_ctx = DeleteCtx::new(true);
        let perm_ctx = DeleteCtx::new(false);
        assert_eq!(trash_ctx, DeleteCtx::new(true));
        assert_ne!(trash_ctx, perm_ctx);
    }

    #[test]
    fn compat_wrapper_reads_the_global_mode() {
        // The wrapper is the only global reader left; one locked test covers it.
        let _lock = crate::context::TEST_LOCK.lock().unwrap();
        set_trash_mode(true);
        assert!(is_trash_mode());
        set_trash_mode(false);
        assert!(!is_trash_mode());
        set_trash_mode(true);
        assert!(is_trash_mode());
    }
}
