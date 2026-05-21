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
