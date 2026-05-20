use std::path::Path;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs;

static TRASH_MODE: AtomicBool = AtomicBool::new(true);

pub fn set_trash_mode(enabled: bool) {
    TRASH_MODE.store(enabled, Ordering::Relaxed);
}

pub fn is_trash_mode() -> bool {
    TRASH_MODE.load(Ordering::Relaxed)
}

pub fn delete_path(path: &Path) -> Result<()> {
    if TRASH_MODE.load(Ordering::Relaxed) {
        trash::delete(path).map_err(|e| anyhow::anyhow!("trash failed: {e}"))
    } else if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
    } else {
        fs::remove_file(path).map_err(|e| anyhow::anyhow!("remove_file {:?}: {}", path, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn delete_path_defaults_to_trash_mode() {
        set_trash_mode(true);
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("default_dir");
        fs::create_dir_all(&d).unwrap();
        delete_path(&d).unwrap();
        assert!(!d.exists(), "default mode must be trash (true)");
    }

    #[test]
    fn delete_path_in_normal_mode_removes_directory() {
        set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("testdir");
        fs::create_dir_all(&d).unwrap();
        delete_path(&d).unwrap();
        assert!(!d.exists(), "directory must be removed");
    }

    #[test]
    fn delete_path_in_trash_mode_removes_file_from_source() {
        set_trash_mode(true);
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("test.txt");
        fs::write(&f, b"hello").unwrap();
        delete_path(&f).unwrap();
        assert!(!f.exists(), "file must be removed from source after trash");
    }

    #[test]
    fn delete_path_in_trash_mode_returns_error_on_failure() {
        set_trash_mode(true);
        let result = delete_path(Path::new("/nonexistent/path/that/cannot/be/trashed"));
        assert!(result.is_err(), "trash of nonexistent path must return Err");
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("trash"), "error must mention trash, got: {err}");
    }

    #[test]
    fn is_trash_mode_defaults_to_true() {
        set_trash_mode(true);
        assert!(is_trash_mode(), "default must be true");
    }

    #[test]
    fn is_trash_mode_reflects_set_trash_mode() {
        set_trash_mode(true);
        assert!(is_trash_mode(), "must be true after set(true)");
        set_trash_mode(false);
        assert!(!is_trash_mode(), "must be false after set(false)");
    }

    #[test]
    fn delete_path_in_normal_mode_removes_file() {
        set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("test.txt");
        fs::write(&f, b"hello").unwrap();
        delete_path(&f).unwrap();
        assert!(!f.exists(), "file must be removed in normal mode");
    }
}
