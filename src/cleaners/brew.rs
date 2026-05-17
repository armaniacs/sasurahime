use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
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

    /// Parses a size string like "16.6GB" or "194.3MB" into bytes.
    pub fn parse_size_str(s: &str) -> Option<u64> {
        if let Some(n) = s.strip_suffix("GB") {
            let v: f64 = n.trim().parse().ok()?;
            Some((v * 1_073_741_824.0) as u64)
        } else if let Some(n) = s.strip_suffix("MB") {
            let v: f64 = n.trim().parse().ok()?;
            Some((v * 1_048_576.0) as u64)
        } else if let Some(n) = s.strip_suffix("KB") {
            let v: f64 = n.trim().parse().ok()?;
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
    fn name(&self) -> &'static str {
        "brew"
    }

    fn detect(&self) -> ScanResult {
        if !self.cache_dir.exists() {
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        let bytes = dir_size(&self.cache_dir);
        ScanResult {
            name: self.name(),
            status: if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        if !self.runner.exists("brew") {
            println!("brew: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
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
}
