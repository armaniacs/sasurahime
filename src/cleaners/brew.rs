use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct BrewCleaner {
    cache_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl BrewCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            cache_dir: home.join("Library/Caches/Homebrew"),
            runner,
        }
    }

    /// Parses a size string like "16.6GB", "194.3MB", "1 GB", or "512kb"
    /// (case-insensitive, space-separated or joined) into bytes.
    pub fn parse_size_str(s: &str) -> Option<u64> {
        // Space-separated form: "194.3 MB" → ("194.3", "MB")
        if let Some((num, unit)) = s.split_once(' ') {
            let v: f64 = num.trim().parse().ok()?;
            let u = unit.trim().to_ascii_uppercase();
            return Some(match u.as_str() {
                "GB" => (v * 1_073_741_824.0) as u64,
                "MB" => (v * 1_048_576.0) as u64,
                "KB" => (v * 1_024.0) as u64,
                _ => return None,
            });
        }

        // Joined form: "16.6GB" / "512kb" / "1.0Gb"
        let upper = s.to_ascii_uppercase();
        if let Some(n) = upper.strip_suffix("GB") {
            let v: f64 = n.parse().ok()?;
            Some((v * 1_073_741_824.0) as u64)
        } else if let Some(n) = upper.strip_suffix("MB") {
            let v: f64 = n.parse().ok()?;
            Some((v * 1_048_576.0) as u64)
        } else if let Some(n) = upper.strip_suffix("KB") {
            let v: f64 = n.parse().ok()?;
            Some((v * 1_024.0) as u64)
        } else {
            None
        }
    }

    /// Extracts freed bytes from brew's output line:
    /// "This operation has freed approximately 16.6GB of disk space."
    pub fn parse_brew_freed_bytes(output: &str) -> u64 {
        for line in output.lines() {
            if line.contains("freed approximately") {
                for token in line.split_whitespace() {
                    if let Some(bytes) = Self::parse_size_str(token) {
                        return bytes;
                    }
                }
            }
        }
        0
    }
}

impl Cleaner for BrewCleaner {
    fn is_available(&self) -> bool {
        self.runner.exists("brew")
    }

    fn name(&self) -> &'static str {
        "brew"
    }

    fn detect(&self) -> ScanResult {
        if !self.cache_dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let bytes = dir_size(&self.cache_dir);
        let mut r = ScanResult::new(
            self.name(),
            if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(self.cache_dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.runner.exists("brew") {
            println!("brew: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }

        let mut args = vec!["cleanup", "-s", "--prune=all"];
        if dry_run {
            args.push("--dry-run");
            println!("[dry-run] would run: brew {}", args.join(" "));
        }

        let output = self.runner.run("brew", &args)?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let freed = Self::parse_brew_freed_bytes(&stdout);

        if freed > 0 {
            println!("Freed: {}", format_bytes(freed));
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: false,
            skipped: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_size_str_gb() {
        let bytes = BrewCleaner::parse_size_str("16.6GB").unwrap();
        assert_eq!(bytes, (16.6_f64 * 1_073_741_824.0) as u64);
    }

    #[test]
    fn parse_size_str_mb() {
        let bytes = BrewCleaner::parse_size_str("194.3MB").unwrap();
        assert_eq!(bytes, (194.3_f64 * 1_048_576.0) as u64);
    }

    #[test]
    fn parse_size_str_invalid() {
        assert_eq!(BrewCleaner::parse_size_str("abc"), None);
        assert_eq!(BrewCleaner::parse_size_str(""), None);
    }

    #[test]
    fn parse_brew_freed_bytes_extracts_gb() {
        let line = "This operation has freed approximately 16.6GB of disk space.";
        let bytes = BrewCleaner::parse_brew_freed_bytes(line);
        assert_eq!(bytes, (16.6_f64 * 1_073_741_824.0) as u64);
    }

    #[test]
    fn parse_brew_freed_bytes_no_match_returns_zero() {
        assert_eq!(BrewCleaner::parse_brew_freed_bytes("no freed here"), 0);
    }

    // ── GAP-003: lowercase / space-separated / edge-case unit forms ─────────
    #[test]
    fn parse_size_str_lowercase_gb() {
        let bytes = BrewCleaner::parse_size_str("16.6gb").unwrap();
        assert_eq!(bytes, (16.6_f64 * 1_073_741_824.0) as u64);
    }

    #[test]
    fn parse_size_str_space_separated_mb() {
        let bytes = BrewCleaner::parse_size_str("194.3 MB").unwrap();
        assert_eq!(bytes, (194.3_f64 * 1_048_576.0) as u64);
    }

    #[test]
    fn parse_size_str_kb_lowercase() {
        let bytes = BrewCleaner::parse_size_str("512kb").unwrap();
        assert_eq!(bytes, 512 * 1_024);
    }

    #[test]
    fn parse_size_str_zero_gb() {
        assert_eq!(BrewCleaner::parse_size_str("0GB"), Some(0));
    }

    #[test]
    fn parse_size_str_is_case_insensitive() {
        assert_eq!(BrewCleaner::parse_size_str("1.0Gb"), Some(1_073_741_824));
        assert_eq!(BrewCleaner::parse_size_str("2.0mB"), Some(2 * 1_048_576));
    }
}
