use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes, parse_size_str, SizeUnits};
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
    ///
    /// Kept as a wrapper (PBI-51): the implementation now lives in the shared
    /// [`crate::format::parse_size_str`] with binary units, matching
    /// [`crate::format::format_bytes`]; behavior and tests are unchanged.
    pub fn parse_size_str(s: &str) -> Option<u64> {
        parse_size_str(s, SizeUnits::Binary)
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
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Safe
    }

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
                deleted_paths: vec![],
            });
        }

        let mut args = vec!["cleanup", "-s", "--prune=all"];
        if dry_run {
            args.push("--dry-run");
            println!("[dry-run] would run: brew {}", args.join(" "));
        }

        // The cache-dir size before the run: the only independent evidence
        // of whether the cleanup actually freed anything (PBI 53).
        let before_size = dir_size(&self.cache_dir);
        let output = self.runner.run("brew", &args)?;
        if !output.status.success() {
            // PBI 53: a failed cleanup must not have its (partial) stdout
            // parsed as a successful run's freed-bytes report.
            eprintln!(
                "[brew] cleanup exited with {} - skipping",
                output.status.code().unwrap_or(-1)
            );
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            });
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let freed = Self::parse_brew_freed_bytes(&stdout);
        let after = dir_size(&self.cache_dir);

        if freed > 0 {
            println!("Freed: {}", format_bytes(freed));
        } else if !dry_run && after < before_size {
            // Parse-miss evidence (PBI 53): brew exited 0 and the cache dir
            // shrank, but the freed-bytes line did not parse (e.g. brew
            // changed its wording). Never report 0 silently.
            eprintln!(
                "[brew] freed-bytes line could not be parsed, but the cache shrank by {}",
                format_bytes(before_size - after)
            );
        }

        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: false,
            skipped: vec![],
            deleted_paths: vec![],
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

    // ── PBI 53: exit-code verification ──

    #[test]
    fn clean_fails_closed_when_cleanup_exits_nonzero() {
        // A failed `brew cleanup` must not have its stdout parsed as a
        // freed-bytes report (PBI 53).
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = tmp.path().join("Library/Caches/Homebrew");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("pkg.tar.gz"), vec![0u8; 512]).unwrap();
        let runner = crate::test_helpers::MockRunner::new()
            .with_success("brew")
            .with_exit_code("brew", 1 << 8);
        let cleaner = BrewCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(result.bytes_freed, 0, "a failed cleanup must report 0");
    }

    #[test]
    fn clean_parses_freed_bytes_from_successful_output() {
        // A successful run with the freed line still reports the parsed
        // amount (regression guard for the exit-code change).
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = tmp.path().join("Library/Caches/Homebrew");
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("pkg.tar.gz"), vec![0u8; 512]).unwrap();
        let runner = crate::test_helpers::MockRunner::new().with_stdout(
            "brew",
            "This operation has freed approximately 1.0MB of disk space.\n",
        );
        let cleaner = BrewCleaner::new(tmp.path(), Box::new(runner));
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(result.bytes_freed, 1_048_576);
    }
}
