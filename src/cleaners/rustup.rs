use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub struct RustupCleaner {
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
        let unused: Vec<_> = all
            .iter()
            .filter(|t| !active.contains(t.as_str()))
            .collect();
        let bytes: u64 = unused
            .iter()
            .map(|t| {
                let dir = self.home.join(".rustup/toolchains").join(t);
                if dir.exists() {
                    dir_size(&dir)
                } else {
                    0
                }
            })
            .sum();
        ScanResult {
            name: self.name(),
            status: if bytes > 0 {
                ScanStatus::Pruneable(bytes)
            } else {
                ScanStatus::Clean
            },
        }
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
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
            let size = {
                let dir = self.home.join(".rustup/toolchains").join(toolchain);
                if dir.exists() {
                    dir_size(&dir)
                } else {
                    0
                }
            };
            if dry_run {
                println!("[dry-run] [rustup] would remove toolchain: {toolchain}");
            } else {
                self.runner
                    .run("rustup", &["toolchain", "remove", toolchain])?;
                freed += size;
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
    use crate::command::CommandRunner;
    use std::fs;
    use std::process::Output;
    use tempfile::TempDir;

    /// Runner that records calls and returns controlled output.
    struct FakeRunner {
        toolchain_list: &'static str,
    }
    impl CommandRunner for FakeRunner {
        fn run(&self, _program: &str, args: &[&str]) -> anyhow::Result<Output> {
            assert_eq!(args, &["toolchain", "list"]);
            Ok(Output {
                status: std::process::ExitStatus::default(),
                stdout: self.toolchain_list.as_bytes().to_vec(),
                stderr: vec![],
            })
        }
        fn exists(&self, _program: &str) -> bool {
            true
        }
    }

    #[test]
    fn detect_measures_actual_toolchain_dirs() {
        let tmp = TempDir::new().unwrap();
        let toolchains = tmp.path().join(".rustup/toolchains");
        // Active toolchain — has a dir with files
        fs::create_dir_all(toolchains.join("stable-aarch64-apple-darwin")).unwrap();
        fs::write(
            toolchains.join("stable-aarch64-apple-darwin/rustc"), &[0u8; 2048],
        )
        .unwrap();
        // Unused toolchain — has a dir with files
        fs::create_dir_all(toolchains.join("nightly-2026-05-01-aarch64-apple-darwin")).unwrap();
        fs::write(
            toolchains
                .join("nightly-2026-05-01-aarch64-apple-darwin/rustc"), &[0u8; 4096],
        )
        .unwrap();

        let runner = FakeRunner {
            toolchain_list:
                "stable-aarch64-apple-darwin (default)\nnightly-2026-05-01-aarch64-apple-darwin\n",
        };
        let cleaner = RustupCleaner {
            home: tmp.path().to_path_buf(),
            runner: Box::new(runner),
        };
        let result = cleaner.detect();

        let expected = crate::format::dir_size(
            &toolchains.join("nightly-2026-05-01-aarch64-apple-darwin"),
        );
        match result.status {
            ScanStatus::Pruneable(bytes) => assert_eq!(
                bytes, expected,
                "detect must report actual dir_size of unused toolchain, not a fixed estimate"
            ),
            other => panic!("expected Pruneable, got {other:?}"),
        }
    }

    #[test]
    fn detect_returns_clean_when_all_active() {
        let tmp = TempDir::new().unwrap();
        let toolchains = tmp.path().join(".rustup/toolchains");
        fs::create_dir_all(toolchains.join("stable-aarch64-apple-darwin")).unwrap();
        fs::write(toolchains.join("stable-aarch64-apple-darwin/rustc"), &[0u8; 64]).unwrap();

        let runner = FakeRunner {
            toolchain_list: "stable-aarch64-apple-darwin (default)\n",
        };
        let cleaner = RustupCleaner {
            home: tmp.path().to_path_buf(),
            runner: Box::new(runner),
        };
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Clean),
            "expected Clean, got {:#?}",
            result.status
        );
    }

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
