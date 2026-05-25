use crate::cleaner::{CleanCancelled, CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::io::{stdin, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// When set, skip the interactive `confirm_message` prompt inside `clean()`.
/// Used by the TUI (interactive.rs) which already asks the user for confirmation
/// before the cleaning loop, avoiding a redundant second prompt.
static SKIP_CONFIRM: AtomicBool = AtomicBool::new(false);

/// Globally suppress the secondary confirmation prompt in `GenericCleaner::clean()`.
/// Call before the TUI cleaning loop and restore with `set_skip_confirm(false)` after.
pub fn set_skip_confirm(skip: bool) {
    SKIP_CONFIRM.store(skip, Ordering::Relaxed);
}

pub enum CleanMethod {
    Command {
        program: &'static str,
        args: &'static [&'static str],
    },
    CommandWithDetectDir {
        program: &'static str,
        args: &'static [&'static str],
        detect_dir: PathBuf,
    },
    DeleteDirs(Vec<PathBuf>),
}

pub struct GenericCleaner {
    display_name: &'static str,
    method: CleanMethod,
    runner: Box<dyn CommandRunner>,
    /// Optional confirmation prompt shown before cleaning on an interactive TTY.
    confirm_message: Option<&'static str>,
    /// If true and the CLI tool is not found, fall back to deleting detect_dir directly.
    fallback_delete: bool,
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
            confirm_message: None,
            fallback_delete: false,
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
        Self::command_cleaner("docker", "docker", &["system", "prune", "-f"], runner)
    }

    pub fn orbstack(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("orbstack", "orb", &["prune"], runner)
    }

    pub fn simulator(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "simulator",
            method: CleanMethod::CommandWithDetectDir {
                program: "xcrun",
                args: &["simctl", "delete", "unavailable"],
                detect_dir: home.join("Library/Developer/CoreSimulator"),
            },
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn colima_prune(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "colima",
            method: CleanMethod::CommandWithDetectDir {
                program: "colima",
                args: &["prune", "--all", "--force"],
                detect_dir: home.join(".colima"),
            },
            runner,
            confirm_message: Some("This will delete ALL stopped Colima VM disk data (containers, images, volumes). Continue?"),
            fallback_delete: true,
        }
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
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn spm_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join("Library/Caches/org.swift.swiftpm");
        Self {
            display_name: "spm",
            method: CleanMethod::DeleteDirs(vec![cache]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn trash(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let trash_dir = home.join(".Trash");
        Self {
            display_name: "trash",
            method: CleanMethod::DeleteDirs(vec![trash_dir]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn downloads(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let dl_dir = home.join("Downloads");
        Self {
            display_name: "downloads",
            method: CleanMethod::DeleteDirs(vec![dl_dir]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    #[allow(dead_code)]
    pub fn cargo_registry(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join(".cargo/registry/cache");
        Self {
            display_name: "cargo-registry",
            method: CleanMethod::DeleteDirs(vec![cache]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn vscode_extensions(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join(".vscode/extensions");
        Self {
            display_name: "vscode-extensions",
            method: CleanMethod::DeleteDirs(vec![cache]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn maven(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "maven",
            method: CleanMethod::CommandWithDetectDir {
                program: "mvn",
                args: &["dependency:purge-local-repository"],
                detect_dir: home.join(".m2/repository"),
            },
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn terraform(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = std::env::var("TF_PLUGIN_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".terraform.d/plugin-cache"));
        Self {
            display_name: "terraform",
            method: CleanMethod::DeleteDirs(vec![cache]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn flutter(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = std::env::var("PUB_CACHE")
            .map(PathBuf::from)
            .unwrap_or_else(|_| home.join(".pub-cache"));
        Self {
            display_name: "flutter",
            method: CleanMethod::CommandWithDetectDir {
                program: "dart",
                args: &["pub", "cache", "clean"],
                detect_dir: cache,
            },
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn volta(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "volta",
            method: CleanMethod::DeleteDirs(vec![home.join(".volta/cache")]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn sbt(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "sbt",
            method: CleanMethod::DeleteDirs(vec![home.join(".sbt"), home.join(".ivy2/cache")]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn tree_sitter(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name: "tree-sitter",
            method: CleanMethod::DeleteDirs(vec![home.join(".cache/tree-sitter")]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }

    pub fn act(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache_dir = match std::env::var("ACT_CACHE_DIR") {
            Ok(dir) => {
                let p = PathBuf::from(&dir);
                if !is_safe_delete_target(&p) {
                    eprintln!(
                        "[act] WARNING: ACT_CACHE_DIR={} points to an unsafe path, using default",
                        dir
                    );
                    home.join(".cache/act")
                } else {
                    p
                }
            }
            Err(_) => home.join(".cache/act"),
        };
        Self {
            display_name: "act",
            method: CleanMethod::DeleteDirs(vec![cache_dir]),
            runner,
            confirm_message: None,
            fallback_delete: false,
        }
    }
}

impl GenericCleaner {
    fn primary_target_display(&self) -> Option<String> {
        match &self.method {
            CleanMethod::Command { .. } => None,
            CleanMethod::CommandWithDetectDir { detect_dir, .. } => {
                Some(detect_dir.to_string_lossy().to_string())
            }
            CleanMethod::DeleteDirs(dirs) => dirs.first().map(|d| d.to_string_lossy().to_string()),
        }
    }
}

impl Cleaner for GenericCleaner {
    fn is_available(&self) -> bool {
        match &self.method {
            CleanMethod::Command { program, .. } => self.runner.exists(program),
            CleanMethod::CommandWithDetectDir { program, .. } => {
                if self.fallback_delete {
                    true
                } else {
                    self.runner.exists(program)
                }
            }
            CleanMethod::DeleteDirs(_) => true,
        }
    }

    fn name(&self) -> &'static str {
        self.display_name
    }

    fn detect(&self) -> ScanResult {
        let make_result = |status| {
            let mut r = ScanResult::new(self.name(), status);
            if crate::context::is_verbose() {
                r.primary_target = self.primary_target_display();
            }
            r
        };

        match &self.method {
            CleanMethod::Command { program, .. } => {
                if !self.runner.exists(program) {
                    return make_result(ScanStatus::NotFound);
                }
                make_result(ScanStatus::Pruneable(0))
            }
            CleanMethod::CommandWithDetectDir {
                program,
                detect_dir,
                ..
            } => {
                if !detect_dir.exists() || (!self.runner.exists(program) && !self.fallback_delete) {
                    return make_result(ScanStatus::NotFound);
                }
                let bytes = dir_size(detect_dir);
                make_result(if bytes > 0 {
                    ScanStatus::Pruneable(bytes)
                } else {
                    ScanStatus::Clean
                })
            }
            CleanMethod::DeleteDirs(dirs) => {
                let existing: Vec<_> = dirs.iter().filter(|d| d.exists()).collect();
                if existing.is_empty() {
                    return make_result(ScanStatus::NotFound);
                }
                let bytes: u64 = existing.iter().map(|d| dir_size(d)).sum();
                make_result(if bytes > 0 {
                    ScanStatus::Pruneable(bytes)
                } else {
                    ScanStatus::Clean
                })
            }
        }
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        match &self.method {
            CleanMethod::Command { program, args } => {
                if !self.runner.exists(program) {
                    println!("{}: not found, skipping", self.display_name);
                    return Ok(CleanResult {
                        name: self.name(),
                        bytes_freed: 0,
                        uses_trash: false,
                        skipped: vec![],
                    });
                }
                if dry_run {
                    println!("[dry-run] would run: {program} {}", args.join(" "));
                    return Ok(CleanResult {
                        name: self.name(),
                        bytes_freed: 0,
                        uses_trash: false,
                        skipped: vec![],
                    });
                }
                println!("$ {program} {}", args.join(" "));
                self.runner.run(program, args)?;
                Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                })
            }
            CleanMethod::CommandWithDetectDir {
                program,
                args,
                detect_dir,
            } => {
                let size_before = if detect_dir.exists() {
                    dir_size(detect_dir)
                } else {
                    0
                };

                // Interactive confirmation prompt when configured.
                // Skipped when running under the interactive TUI (which already
                // asked for confirmation) or when stdin is not a terminal.
                if let Some(msg) = &self.confirm_message {
                    if !SKIP_CONFIRM.load(Ordering::Relaxed)
                        && stdin().is_terminal()
                        && !dialoguer::Confirm::new()
                            .with_prompt(*msg)
                            .default(false)
                            .interact()?
                    {
                        println!("{}: cancelled", self.display_name);
                        eprintln!("  $ {program} {}", args.join(" "));
                        return Err(anyhow::Error::from(CleanCancelled));
                    }
                }

                if !self.runner.exists(program) {
                    if self.fallback_delete && detect_dir.exists() {
                        if dry_run {
                            println!(
                                "[dry-run] would remove: {} ({} bytes)",
                                detect_dir.display(),
                                crate::format::format_bytes(size_before)
                            );
                            return Ok(CleanResult {
                                name: self.name(),
                                bytes_freed: 0,
                                uses_trash: false,
                                skipped: vec![],
                            });
                        }
                        let path_str = detect_dir.to_string_lossy();
                        if let Err(e) = self.runner.run("chflags", &["-R", "nouchg", &path_str]) {
                            eprintln!(
                                "[{}] warning: chflags failed for {}: {e}",
                                self.display_name,
                                detect_dir.display()
                            );
                        }
                        crate::trash::delete_path(detect_dir)?;
                        println!(
                            "[{}] removed cache: {}",
                            self.display_name,
                            detect_dir.display()
                        );
                        return Ok(CleanResult {
                            name: self.name(),
                            bytes_freed: size_before,
                            uses_trash: true,
                            skipped: vec![],
                        });
                    }
                    println!("{}: not found, skipping", self.display_name);
                    return Ok(CleanResult {
                        name: self.name(),
                        bytes_freed: 0,
                        uses_trash: false,
                        skipped: vec![],
                    });
                }
                if dry_run {
                    println!("[dry-run] would run: {program} {}", args.join(" "));
                    if size_before > 0 {
                        println!(
                            "[dry-run] would free: {}",
                            crate::format::format_bytes(size_before)
                        );
                    }
                    return Ok(CleanResult {
                        name: self.name(),
                        bytes_freed: 0,
                        uses_trash: false,
                        skipped: vec![],
                    });
                }
                println!("$ {program} {}", args.join(" "));
                let output = self.runner.run(program, args)?;
                if !output.status.success() {
                    eprintln!(
                        "[{}] warning: `{} {}` exited with code {:?}",
                        self.display_name,
                        program,
                        args.join(" "),
                        output.status.code()
                    );
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.is_empty() {
                        eprintln!("[{}] stderr: {stderr}", self.display_name);
                    }
                }
                let size_after = if detect_dir.exists() {
                    dir_size(detect_dir)
                } else {
                    0
                };
                let freed = size_before.saturating_sub(size_after);
                if freed > 0 {
                    println!("Freed: {}", crate::format::format_bytes(freed));
                }
                Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: freed,
                    uses_trash: false,
                    skipped: vec![],
                })
            }
            CleanMethod::DeleteDirs(dirs) => {
                let cleanable: Vec<&PathBuf> = dirs.iter().filter(|d| d.exists()).collect();
                if !dry_run && !cleanable.is_empty() {
                    reporter.progress_init(self.name(), cleanable.len());
                }
                let mut freed: u64 = 0;
                for (i, dir) in cleanable.iter().enumerate() {
                    let size = dir_size(dir);
                    if dry_run {
                        println!("[dry-run] would remove: {}", dir.display());
                    } else {
                        reporter.progress_tick(dir, i + 1, size);
                        let path_str = dir.to_string_lossy();
                        if let Err(e) = self.runner.run("chflags", &["-R", "nouchg", &path_str]) {
                            eprintln!(
                                "[{}] warning: chflags failed for {}: {e}",
                                self.display_name,
                                dir.display()
                            );
                        }
                        crate::trash::delete_path(dir)?;
                        freed += size;
                        println!("Removed: {}", dir.display());
                    }
                }
                if !dry_run && !cleanable.is_empty() {
                    reporter.progress_finish();
                }
                Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: freed,
                    uses_trash: true,
                    skipped: vec![],
                })
            }
        }
    }
}

pub fn is_safe_delete_target(path: &Path) -> bool {
    let check = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let path_str = check.to_string_lossy();
    if path_str.is_empty() {
        return false;
    }
    if path_str.contains("..") {
        return false;
    }
    check != Path::new("/")
        && !check.starts_with("/System")
        && !check.starts_with("/etc")
        && !check.starts_with("/var")
        && !check.starts_with("/tmp")
        && !check.starts_with("/private")
        && !check.starts_with("/dev")
        && !check.starts_with("/proc")
        && !check.starts_with("/Applications")
        && !check.starts_with("/usr")
}

/// Configuration for [`clean_cli_or_fallback`].
pub struct CliFallbackConfig<'a> {
    pub tool: &'a str,
    pub args: &'a [&'a str],
    /// Whether to recreate the cache directory after fallback deletion.
    pub recreate: bool,
}

/// Run a CLI tool to clean a cache directory, falling back to direct deletion
/// when the tool is not installed.
///
/// This eliminates the duplicated "CLI first → fallback" pattern between
/// `huggingface.rs` and `pre_commit.rs`.
pub fn clean_cli_or_fallback(
    name: &'static str,
    dir: &Path,
    runner: &dyn CommandRunner,
    config: &CliFallbackConfig,
    dry_run: bool,
) -> Result<CleanResult> {
    if !dir.exists() {
        return Ok(CleanResult {
            name,
            bytes_freed: 0,
            uses_trash: false,
            skipped: vec![],
        });
    }

    // Prefer CLI if available
    if runner.exists(config.tool) {
        if dry_run {
            println!(
                "[dry-run] [{name}] would run: {} {}",
                config.tool,
                config.args.join(" ")
            );
            let size = dir_size(dir);
            println!(
                "[dry-run] [{name}] would free: {}",
                crate::format::format_bytes(size)
            );
            return Ok(CleanResult {
                name,
                bytes_freed: 0,
                uses_trash: false,
                skipped: vec![],
            });
        }
        let size_before = dir_size(dir);
        let output = runner.run(config.tool, config.args)?;
        if !output.status.success() {
            anyhow::bail!(
                "{} failed with exit code {:?}",
                config.tool,
                output.status.code()
            );
        }
        println!("[{name}] ran {} {}", config.tool, config.args.join(" "));
        return Ok(CleanResult {
            name,
            bytes_freed: size_before,
            uses_trash: false,
            skipped: vec![],
        });
    }

    // Fallback: delete directly
    let size = dir_size(dir);
    if dry_run {
        println!(
            "[dry-run] [{name}] would remove: {} ({})",
            dir.display(),
            crate::format::format_bytes(size)
        );
        return Ok(CleanResult {
            name,
            bytes_freed: 0,
            uses_trash: false,
            skipped: vec![],
        });
    }

    let path_str = dir.to_string_lossy();
    if let Err(e) = runner.run("chflags", &["-R", "nouchg", &path_str]) {
        eprintln!(
            "[{name}] warning: chflags failed for {}: {e}",
            dir.display()
        );
    }
    crate::trash::delete_path(dir)?;
    if config.recreate {
        fs::create_dir_all(dir)?;
    }
    println!("[{name}] removed cache: {}", dir.display());
    Ok(CleanResult {
        name,
        bytes_freed: size,
        uses_trash: true,
        skipped: vec![],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRunner;

    struct MissingToolRunner;
    impl CommandRunner for MissingToolRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            unreachable!()
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }

    #[test]
    fn command_with_detect_dir_fallback_reports_pruneable_when_tool_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Directory exists but the cleaning tool is not installed;
        // fallback_delete should allow detect to report Pruneable anyway.
        fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
        fs::write(tmp.path().join(".colima/_lima/colima/dummy.img"), b"x").unwrap();

        let cleaner = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::Pruneable(_)),
            "expected Pruneable when dir exists (fallback active), got {:#?}",
            result.status
        );
    }

    #[test]
    fn command_without_fallback_returns_not_found_when_tool_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
        fs::write(tmp.path().join(".colima/_lima/colima/dummy.img"), b"x").unwrap();

        // Using a non-colima CommandWithDetectDir cleaner (no fallback).
        let cleaner = GenericCleaner::simulator(tmp.path(), Box::new(MissingToolRunner));
        let result = cleaner.detect();
        assert!(
            matches!(result.status, ScanStatus::NotFound),
            "expected NotFound when tool missing and no fallback, got {:#?}",
            result.status
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_root() {
        assert!(!is_safe_delete_target(Path::new("/")));
    }

    #[test]
    fn is_safe_delete_target_rejects_system_dirs() {
        assert!(!is_safe_delete_target(Path::new("/System/Library")));
        assert!(!is_safe_delete_target(Path::new("/etc/hosts")));
        assert!(!is_safe_delete_target(Path::new("/var/log")));
        assert!(!is_safe_delete_target(Path::new("/Applications/Xcode.app")));
    }

    #[test]
    fn is_safe_delete_target_rejects_empty() {
        assert!(!is_safe_delete_target(Path::new("")));
    }

    #[test]
    fn is_safe_delete_target_allows_home_cache() {
        assert!(is_safe_delete_target(Path::new("/Users/test/.cache/act")));
        assert!(is_safe_delete_target(Path::new(
            "/Users/test/Library/Caches"
        )));
    }

    #[test]
    fn is_safe_delete_target_rejects_tmp() {
        assert!(!is_safe_delete_target(Path::new("/tmp")));
        assert!(!is_safe_delete_target(Path::new("/tmp/safe-dir")));
    }

    #[test]
    fn is_safe_delete_target_rejects_private_tmp() {
        assert!(!is_safe_delete_target(Path::new("/private/tmp")));
        assert!(!is_safe_delete_target(Path::new("/dev/null")));
        assert!(!is_safe_delete_target(Path::new("/proc/self")));
    }

    #[test]
    fn is_safe_delete_target_rejects_usr() {
        assert!(!is_safe_delete_target(Path::new("/usr/local")));
        assert!(!is_safe_delete_target(Path::new("/usr/lib")));
    }

    #[test]
    fn is_safe_delete_target_rejects_dotdot_traversal() {
        assert!(!is_safe_delete_target(Path::new(
            "/Users/foo/../../etc/passwd"
        )));
        assert!(!is_safe_delete_target(Path::new("/tmp/../etc")));
    }

    #[test]
    fn is_safe_delete_target_canonicalize_follows_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let target = tmp.path().join("symlink_to_etc");
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/etc", &target).unwrap();
            assert!(
                !is_safe_delete_target(&target),
                "symlink to /etc must be rejected"
            );
        }
    }

    #[test]
    fn is_safe_delete_target_rejects_dotdot_redirection() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        fs::create_dir_all(&sub).unwrap();
        let traversed = sub.join("../../etc");
        assert!(
            !is_safe_delete_target(&traversed),
            "path with .. to /etc must be rejected"
        );
    }

    #[test]
    fn act_path_validates_env_var_and_falls_back() {
        let tmp = tempfile::TempDir::new().unwrap();
        let prev = std::env::var("ACT_CACHE_DIR").ok();
        std::env::set_var("ACT_CACHE_DIR", "/");
        let cleaner =
            GenericCleaner::act(tmp.path(), Box::new(crate::command::SystemCommandRunner));
        match prev {
            Some(v) => std::env::set_var("ACT_CACHE_DIR", v),
            None => std::env::remove_var("ACT_CACHE_DIR"),
        }
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    // ── primary_target_display tests ────────────────────────────────────────

    #[test]
    fn primary_target_display_for_command_returns_none() {
        let cleaner = GenericCleaner::bun(Box::new(MissingToolRunner));
        assert!(cleaner.primary_target_display().is_none());
    }

    #[test]
    fn primary_target_display_for_command_with_detect_dir_returns_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let target = cleaner.primary_target_display();
        assert!(target.is_some());
        assert!(target.unwrap().contains(".colima"));
    }

    #[test]
    fn primary_target_display_for_delete_dirs_returns_first_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = GenericCleaner::node_gyp(tmp.path(), Box::new(MissingToolRunner));
        let target = cleaner.primary_target_display();
        assert!(target.is_some());
        let path = target.unwrap();
        assert!(path.contains(".cache/node-gyp"));
    }

    #[test]
    fn detect_includes_primary_target_when_verbose() {
        let _guard = crate::context::TEST_LOCK.lock().unwrap();
        crate::context::set_verbose(true);
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/act");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("dummy"), b"x").unwrap();

        let cleaner = GenericCleaner::act(tmp.path(), Box::new(MissingToolRunner));
        let result = cleaner.detect();
        assert!(
            result.primary_target.is_some(),
            "primary_target should be set when verbose"
        );
        assert!(
            result.primary_target.unwrap().contains(".cache/act"),
            "target should point to act cache dir"
        );
        crate::context::set_verbose(false);
    }

    #[test]
    fn detect_omits_primary_target_when_not_verbose() {
        let _guard = crate::context::TEST_LOCK.lock().unwrap();
        crate::context::set_verbose(false);
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/act");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("dummy"), b"x").unwrap();

        let cleaner = GenericCleaner::act(tmp.path(), Box::new(MissingToolRunner));
        let result = cleaner.detect();
        assert!(result.primary_target.is_none());
    }
}
