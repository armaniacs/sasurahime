use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub enum CleanMethod {
    Command {
        program: &'static str,
        args: &'static [&'static str],
    },
    DeleteDirs(Vec<PathBuf>),
}

pub struct GenericCleaner {
    display_name: &'static str,
    method: CleanMethod,
    runner: Box<dyn CommandRunner>,
}

impl GenericCleaner {
    fn command_cleaner(
        display_name: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        runner: Box<dyn CommandRunner>,
    ) -> Self {
        Self {
            display_name,
            method: CleanMethod::Command { program, args },
            runner,
        }
    }

    pub fn bun(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("bun", "bun", &["pm", "cache", "rm"], runner)
    }

    pub fn go(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("go", "go", &["clean", "-cache"], runner)
    }

    pub fn pip(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("pip", "pip", &["cache", "purge"], runner)
    }

    pub fn npm(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("npm", "npm", &["cache", "clean", "--force"], runner)
    }

    pub fn yarn(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("yarn", "yarn", &["cache", "clean"], runner)
    }

    pub fn pnpm(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("pnpm", "pnpm", &["store", "prune"], runner)
    }

    pub fn deno(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("deno", "deno", &["cache", "-r"], runner)
    }

    pub fn pipx(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("pipx", "pipx", &["cache", "purge"], runner)
    }

    pub fn docker(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("docker", "docker", &["system", "prune", "-af"], runner)
    }

    pub fn orbstack(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("orbstack", "orb", &["prune"], runner)
    }

    pub fn cocoapods(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("cocoapods", "pod", &["cache", "clean", "--all"], runner)
    }

    pub fn conda(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("conda", "conda", &["clean", "--all", "-y"], runner)
    }

    pub fn poetry(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("poetry", "poetry", &["cache", "clear", "--all"], runner)
    }

    pub fn node_gyp(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "node-gyp",
            method: CleanMethod::DeleteDirs(vec![
                home.join(".cache/node-gyp"),
                home.join("Library/Caches/node-gyp"),
            ]),
            runner,
        }
    }

    pub fn spm_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join("Library/Caches/org.swift.swiftpm");
        Self {
            display_name: "spm",
            method: CleanMethod::DeleteDirs(vec![cache]),
            runner,
        }
    }

    pub fn cargo_registry(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join(".cargo/registry/cache");
        Self {
            display_name: "cargo-registry",
            method: CleanMethod::DeleteDirs(vec![cache]),
            runner,
        }
    }
}

impl Cleaner for GenericCleaner {
    fn name(&self) -> &'static str {
        self.display_name
    }

    fn detect(&self) -> ScanResult {
        match &self.method {
            CleanMethod::Command { program, .. } => {
                if !self.runner.exists(program) {
                    return ScanResult {
                        name: self.name(),
                        status: ScanStatus::NotFound,
                    };
                }
                // Size is unknown without running the tool; report as pruneable.
                ScanResult {
                    name: self.name(),
                    status: ScanStatus::Pruneable(0),
                }
            }
            CleanMethod::DeleteDirs(dirs) => {
                let existing: Vec<_> = dirs.iter().filter(|d| d.exists()).collect();
                if existing.is_empty() {
                    return ScanResult {
                        name: self.name(),
                        status: ScanStatus::NotFound,
                    };
                }
                let bytes: u64 = existing.iter().map(|d| dir_size(d)).sum();
                ScanResult {
                    name: self.name(),
                    status: if bytes > 0 {
                        ScanStatus::Pruneable(bytes)
                    } else {
                        ScanStatus::Clean
                    },
                }
            }
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        match &self.method {
            CleanMethod::Command { program, args } => {
                if !self.runner.exists(program) {
                    println!("{}: not found, skipping", self.display_name);
                    return Ok(CleanResult {
                        name: self.name(),
                        bytes_freed: 0,
                    });
                }
                if dry_run {
                    println!("[dry-run] would run: {program} {}", args.join(" "));
                    return Ok(CleanResult {
                        name: self.name(),
                        bytes_freed: 0,
                    });
                }
                self.runner.run(program, args)?;
                Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                })
            }
            CleanMethod::DeleteDirs(dirs) => {
                let mut freed: u64 = 0;
                for dir in dirs {
                    if !dir.exists() {
                        continue;
                    }
                    let size = dir_size(dir);
                    if dry_run {
                        println!("[dry-run] would remove: {}", dir.display());
                    } else {
                        // GAP-010: clear uchg flag before removal (macOS safety rule)
                        let path_str = dir.to_string_lossy();
                        let _ = self.runner.run("chmod", &["-R", "nouchg", &path_str]);
                        fs::remove_dir_all(dir)
                            .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", dir, e))?;
                        freed += size;
                        println!("Removed: {}", dir.display());
                    }
                }
                Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: freed,
                })
            }
        }
    }
}
