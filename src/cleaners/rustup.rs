use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct RustupCleaner {
    #[allow(dead_code)]
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl RustupCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
        }
    }

    /// Parses `rustup toolchain list` output.
    /// Active toolchains are marked with "(default)" or "(override)".
    fn parse_toolchains(stdout: &str) -> (Vec<String>, HashSet<String>) {
        let mut all = vec![];
        let mut active = HashSet::new();
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let name = trimmed.split_whitespace().next().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            let is_active = trimmed.contains("(default)") || trimmed.contains("(override)");
            all.push(name.to_string());
            if is_active {
                active.insert(name.to_string());
            }
        }
        (all, active)
    }
}

impl Cleaner for RustupCleaner {
    fn name(&self) -> &'static str {
        "rustup"
    }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("rustup") {
            return ScanResult {
                name: self.name(),
                status: ScanStatus::NotFound,
            };
        }
        let output = match self.runner.run("rustup", &["toolchain", "list"]) {
            Ok(o) => o,
            Err(_) => {
                return ScanResult {
                    name: self.name(),
                    status: ScanStatus::NotFound,
                }
            }
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (all, active) = Self::parse_toolchains(&stdout);
        let unused_count = all.iter().filter(|t| !active.contains(t.as_str())).count();
        let bytes = unused_count as u64 * 300_000_000;
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
        if !self.runner.exists("rustup") {
            println!("rustup: not found, skipping");
            return Ok(CleanResult {
                name: self.name(),
                bytes_freed: 0,
            });
        }
        let output = self.runner.run("rustup", &["toolchain", "list"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (all, active) = Self::parse_toolchains(&stdout);
        let mut freed: u64 = 0;

        for toolchain in &all {
            if active.contains(toolchain) {
                continue;
            }
            if dry_run {
                println!("[dry-run] [rustup] would remove toolchain: {toolchain}");
            } else {
                self.runner
                    .run("rustup", &["toolchain", "remove", toolchain])?;
                freed += 300_000_000;
                println!("[rustup] removed toolchain: {toolchain}");
            }
        }
        if freed == 0 && !dry_run {
            println!("[rustup] no unused toolchains found");
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
    fn parse_toolchains_active_only() {
        let stdout =
            "stable-aarch64-apple-darwin (default)\nnightly-2026-05-01-aarch64-apple-darwin\n";
        let (all, active) = RustupCleaner::parse_toolchains(stdout);
        assert_eq!(all.len(), 2);
        assert!(active.contains("stable-aarch64-apple-darwin"));
        assert!(!active.contains("nightly-2026-05-01-aarch64-apple-darwin"));
    }
}
