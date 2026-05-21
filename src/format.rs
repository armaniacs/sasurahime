use std::os::unix::fs::MetadataExt;

/// Returns the total physical disk usage in bytes of all files under `path`.
/// Uses `st_blocks × 512` (physical blocks) instead of logical file size so
/// that sparse files (e.g. VM disk images in `~/.colima`) are reported at
/// their actual on-disk footprint, matching what `du` shows.
/// Returns 0 if the path does not exist or cannot be read.
pub fn dir_size(path: &std::path::Path) -> u64 {
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.blocks() * 512)
        .sum()
}

/// Formats a byte count as a human-readable string using binary units.
pub fn format_bytes(bytes: u64) -> String {
    const GIB: u64 = 1_073_741_824;
    const MIB: u64 = 1_048_576;
    const KIB: u64 = 1_024;

    if bytes >= GIB {
        format!("{:.1} GB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KB", bytes as f64 / KIB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    // ── dir_size ────────────────────────────────────────────────────────────

    #[test]
    fn dir_size_missing_path_returns_zero() {
        assert_eq!(dir_size(Path::new("/nonexistent/path/12345")), 0);
    }

    #[test]
    fn dir_size_empty_dir_returns_zero() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(dir_size(tmp.path()), 0);
    }

    #[test]
    fn dir_size_measures_physical_blocks_not_logical_len() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("small.txt");
        fs::write(&f, b"hello").unwrap();
        let meta = fs::metadata(&f).unwrap();
        let expected_physical = meta.blocks() * 512;
        assert_eq!(dir_size(tmp.path()), expected_physical);
        // Physical size (padded to block) is > logical 5 bytes on APFS
        assert!(
            expected_physical >= 5,
            "physical blocks must cover the 5-byte file"
        );
    }

    #[test]
    fn dir_size_sums_multiple_files() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.bin"), &[0u8; 100]).unwrap();
        fs::write(tmp.path().join("b.bin"), &[0u8; 200]).unwrap();
        let expected: u64 = [tmp.path().join("a.bin"), tmp.path().join("b.bin")]
            .iter()
            .map(|p| fs::metadata(p).map(|m| m.blocks() * 512).unwrap_or(0))
            .sum();
        assert_eq!(dir_size(tmp.path()), expected);
    }

    #[test]
    fn dir_size_does_not_follow_symlinks() {
        let tmp = TempDir::new().unwrap();
        let real_dir = tmp.path().join("real");
        fs::create_dir(&real_dir).unwrap();
        fs::write(real_dir.join("data.bin"), &[0u8; 64]).unwrap();
        #[cfg(unix)]
        {
            let link = tmp.path().join("link");
            std::os::unix::fs::symlink(&real_dir, &link).unwrap();
            // dir_size on the symlink follows the starting path (resolves real dir),
            // but walkdir does NOT descend into symlinks it encounters.
            // The symlink itself is not a file → not counted.
            assert_eq!(dir_size(&link), dir_size(&real_dir));
        }
    }

    #[test]
    fn format_bytes_gb() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
        assert_eq!(format_bytes(2_147_483_648), "2.0 GB");
    }

    #[test]
    fn format_bytes_mb() {
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
        assert_eq!(format_bytes(10_485_760), "10.0 MB");
    }

    #[test]
    fn format_bytes_kb() {
        assert_eq!(format_bytes(1_024), "1.0 KB");
    }

    #[test]
    fn format_bytes_b() {
        assert_eq!(format_bytes(512), "512 B");
    }
}
