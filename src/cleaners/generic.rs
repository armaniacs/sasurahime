use crate::cleaner::{CleanCancelled, CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
#[cfg(test)]
use crate::command::SystemCommandRunner;
use crate::config::Config;
use crate::format::dir_size;
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::io::{stdin, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

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
    /// Skip entries with mtime newer than this many days (None = no filter).
    older_than_days: Option<u32>,
    /// Skip entries with total size below this many MiB (None = no filter).
    larger_than_mb: Option<u64>,
}

impl GenericCleaner {
    fn base_cleaner(
        display_name: &'static str,
        method: CleanMethod,
        runner: Box<dyn CommandRunner>,
    ) -> Self {
        Self {
            display_name,
            method,
            runner,
            confirm_message: None,
            fallback_delete: false,
            older_than_days: None,
            larger_than_mb: None,
        }
    }

    fn command_cleaner(
        display_name: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        runner: Box<dyn CommandRunner>,
    ) -> Self {
        Self::base_cleaner(display_name, CleanMethod::Command { program, args }, runner)
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
        Self::base_cleaner(
            "simulator",
            CleanMethod::CommandWithDetectDir {
                program: "xcrun",
                args: &["simctl", "delete", "unavailable"],
                detect_dir: home.join("Library/Developer/CoreSimulator"),
            },
            runner,
        )
    }

    pub fn colima_prune(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            confirm_message: Some("This will delete ALL stopped Colima VM disk data (containers, images, volumes). Continue?"),
            fallback_delete: true,
            ..Self::base_cleaner(
                "colima",
                CleanMethod::CommandWithDetectDir {
                    program: "colima",
                    args: &["prune", "--all", "--force"],
                    detect_dir: home.join(".colima"),
                },
                runner,
            )
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
        Self::base_cleaner(
            "node-gyp",
            CleanMethod::DeleteDirs(vec![
                home.join(".cache/node-gyp"),
                home.join("Library/Caches/node-gyp"),
            ]),
            runner,
        )
    }

    pub fn spm_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join("Library/Caches/org.swift.swiftpm");
        Self::base_cleaner("spm", CleanMethod::DeleteDirs(vec![cache]), runner)
    }

    pub fn trash(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "trash",
            CleanMethod::DeleteDirs(vec![home.join(".Trash")]),
            runner,
        )
    }

    pub fn downloads(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "downloads",
            CleanMethod::DeleteDirs(vec![home.join("Downloads")]),
            runner,
        )
        .with_older_than(30)
    }

    #[allow(dead_code)]
    pub fn cargo_registry(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "cargo-registry",
            CleanMethod::DeleteDirs(vec![home.join(".cargo/registry/cache")]),
            runner,
        )
    }

    pub fn vscode_extensions(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "vscode-extensions",
            CleanMethod::DeleteDirs(vec![home.join(".vscode/extensions")]),
            runner,
        )
    }

    pub fn maven(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "maven",
            CleanMethod::CommandWithDetectDir {
                program: "mvn",
                args: &["dependency:purge-local-repository"],
                detect_dir: home.join(".m2/repository"),
            },
            runner,
        )
    }

    pub fn terraform(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = match std::env::var("TF_PLUGIN_CACHE_DIR") {
            Ok(dir) => {
                let p = PathBuf::from(&dir);
                if !is_safe_delete_target(&p) {
                    eprintln!(
                        "[terraform] WARNING: TF_PLUGIN_CACHE_DIR={dir} points to an unsafe path, using default"
                    );
                    home.join(".terraform.d/plugin-cache")
                } else {
                    p
                }
            }
            Err(_) => home.join(".terraform.d/plugin-cache"),
        };
        Self::base_cleaner("terraform", CleanMethod::DeleteDirs(vec![cache]), runner)
    }

    pub fn flutter(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = match std::env::var("PUB_CACHE") {
            Ok(dir) => {
                let p = PathBuf::from(&dir);
                if !is_safe_delete_target(&p) {
                    eprintln!(
                        "[flutter] WARNING: PUB_CACHE={dir} points to an unsafe path, using default"
                    );
                    home.join(".pub-cache")
                } else {
                    p
                }
            }
            Err(_) => home.join(".pub-cache"),
        };
        Self::base_cleaner(
            "flutter",
            CleanMethod::CommandWithDetectDir {
                program: "dart",
                args: &["pub", "cache", "clean"],
                detect_dir: cache,
            },
            runner,
        )
    }

    pub fn volta(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "volta",
            CleanMethod::DeleteDirs(vec![home.join(".volta/cache")]),
            runner,
        )
    }

    pub fn sbt(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "sbt",
            CleanMethod::DeleteDirs(vec![home.join(".sbt"), home.join(".ivy2/cache")]),
            runner,
        )
    }

    pub fn tree_sitter(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::base_cleaner(
            "tree-sitter",
            CleanMethod::DeleteDirs(vec![home.join(".cache/tree-sitter")]),
            runner,
        )
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
        Self::base_cleaner("act", CleanMethod::DeleteDirs(vec![cache_dir]), runner)
    }
}

impl GenericCleaner {
    /// Create a `DeleteDirs` cleaner with a given display name and target path.
    /// The runner is a no-op since `DeleteDirs` cleaners never invoke external commands.
    #[cfg(test)]
    pub fn delete_dirs(name: &'static str, path: PathBuf) -> Self {
        Self::base_cleaner(
            name,
            CleanMethod::DeleteDirs(vec![path]),
            Box::new(SystemCommandRunner),
        )
    }

    /// Apply an age filter: only entries with mtime older than `days` days
    /// will be included in detect/clean.
    pub fn with_older_than(mut self, days: u32) -> Self {
        self.older_than_days = Some(days);
        self
    }

    /// Apply a size filter: only entries with total size >= `mb` MiB
    /// will be included in detect/clean.
    pub fn with_larger_than(mut self, mb: u64) -> Self {
        self.larger_than_mb = Some(mb);
        self
    }

    /// Apply per-cleaner config filters from the parsed config file.
    /// Looks up `self.name()` in `config.per_cleaner` and applies
    /// `older_than_days` and `larger_than_mb` if present.
    pub fn with_config(self, config: &Config) -> Self {
        let name = self.name();
        if let Some(pcc) = config.per_cleaner.get(name) {
            if (pcc.older_than_days.is_some() || pcc.larger_than_mb.is_some())
                && matches!(
                    self.method,
                    CleanMethod::Command { .. } | CleanMethod::CommandWithDetectDir { .. }
                )
            {
                eprintln!("Warning: per-cleaner filters (older_than_days, larger_than_mb) are not supported for '{}': command-based cleaner. Filters apply only to DeleteDirs cleaners.", name);
            }
            let mut c = self;
            if let Some(days) = pcc.older_than_days {
                c = c.with_older_than(days);
            }
            if let Some(mb) = pcc.larger_than_mb {
                c = c.with_larger_than(mb);
            }
            c
        } else {
            self
        }
    }

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
                let entries: Vec<(PathBuf, u64)> = dirs
                    .iter()
                    .filter(|d| d.exists())
                    .filter_map(|d| {
                        let meta = fs::metadata(d).ok()?;
                        if !meets_age_filter(&meta, self.older_than_days) {
                            return None;
                        }
                        let size = dir_size(d);
                        if !meets_size_filter(size, self.larger_than_mb) {
                            return None;
                        }
                        Some((d.clone(), size))
                    })
                    .collect();
                if entries.is_empty() {
                    return make_result(ScanStatus::NotFound);
                }
                let bytes: u64 = entries.iter().map(|(_, s)| s).sum();
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
                let cleanable: Vec<&PathBuf> = dirs
                    .iter()
                    .filter(|d| d.exists())
                    .filter(|d| {
                        fs::metadata(d)
                            .map(|m| meets_age_filter(&m, self.older_than_days))
                            .unwrap_or(false)
                    })
                    .filter(|d| meets_size_filter(dir_size(d), self.larger_than_mb))
                    .collect();
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
    // Block exact sensitive roots and system paths.
    // Allow macOS tempdirs (/var/folders/...) while blocking /var itself
    // and known system subdirectories.
    let is_exact = |p: &str| check == Path::new(p);
    let is_sys = |p: &str| check.starts_with(p);
    !is_exact("/")
        && !is_exact("/private")
        && !is_sys("/System")
        && !is_sys("/etc")
        && !is_sys("/dev")
        && !is_sys("/proc")
        && !is_sys("/Applications")
        && !is_sys("/usr")
        && !is_sys("/tmp")
        && !is_sys("/private/etc")
        && !is_sys("/private/tmp")
        && !is_sys("/private/var/db")
        && !is_sys("/private/var/log")
        && !is_sys("/private/var/run")
        && !is_sys("/var/log")
        && !is_sys("/var/db")
        && !is_sys("/var/root")
        && !is_sys("/var/run")
        && !is_exact("/var")
}

/// Returns `true` if the file or directory at `path` has a modification time
/// older than `days` days. Returns `false` if the path does not exist or
/// its mtime cannot be determined.
#[cfg(test)]
pub fn is_older_than(path: &Path, days: u32) -> bool {
    let threshold = Duration::from_secs(u64::from(days) * 86_400);
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
        .is_some_and(|age| age > threshold)
}

/// Returns `true` if no age filter is set, or if the metadata's mtime is
/// older than the configured number of days.
pub fn meets_age_filter(metadata: &fs::Metadata, older_than_days: Option<u32>) -> bool {
    match older_than_days {
        Some(days) => {
            let threshold = Duration::from_secs(u64::from(days) * 86_400);
            metadata
                .modified()
                .ok()
                .and_then(|mtime| SystemTime::now().duration_since(mtime).ok())
                .is_some_and(|age| age > threshold)
        }
        None => true,
    }
}

/// Returns `true` if no size filter is set, or if `size` >= the configured
/// threshold in mebibytes.
pub fn meets_size_filter(size: u64, larger_than_mb: Option<u64>) -> bool {
    match larger_than_mb {
        Some(mb) => size >= mb * 1_048_576,
        None => true,
    }
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

    // ── is_older_than / meets_age_filter / meets_size_filter ─────────────────

    use filetime::FileTime;
    use std::time::{Duration, SystemTime};

    fn write_aged_dir(path: &Path, secs_old: u64) {
        fs::create_dir_all(path).unwrap();
        // Write a file first so the dir exists with content, THEN set mtime.
        // On macOS, creating a file inside a dir updates the parent's mtime,
        // so we must set mtime after the write.
        fs::write(path.join(".aged"), b"x").unwrap();
        let mtime = SystemTime::now() - Duration::from_secs(secs_old);
        filetime::set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
    }

    #[test]
    fn is_older_than_returns_true_when_older() {
        let tmp = tempfile::TempDir::new().unwrap();
        let d = tmp.path().join("old_dir");
        write_aged_dir(&d, 30 * 86_400 + 1);
        assert!(is_older_than(&d, 30));
    }

    #[test]
    fn is_older_than_returns_false_when_newer() {
        let tmp = tempfile::TempDir::new().unwrap();
        let d = tmp.path().join("new_dir");
        fs::create_dir_all(&d).unwrap();
        assert!(!is_older_than(&d, 30));
    }

    #[test]
    fn is_older_than_nonexistent_returns_false() {
        assert!(!is_older_than(Path::new("/nonexistent/path"), 7));
    }

    #[test]
    fn meets_age_filter_none_returns_true() {
        let tmp = tempfile::TempDir::new().unwrap();
        let meta = fs::metadata(tmp.path()).unwrap();
        assert!(meets_age_filter(&meta, None));
    }

    #[test]
    fn meets_age_filter_some_old_enough_returns_true() {
        let tmp = tempfile::TempDir::new().unwrap();
        let d = tmp.path().join("old");
        write_aged_dir(&d, 30 * 86_400 + 1);
        let meta = fs::metadata(&d).unwrap();
        assert!(meets_age_filter(&meta, Some(30)));
    }

    #[test]
    fn meets_age_filter_some_too_new_returns_false() {
        let tmp = tempfile::TempDir::new().unwrap();
        let d = tmp.path().join("new");
        fs::create_dir_all(&d).unwrap();
        let meta = fs::metadata(&d).unwrap();
        assert!(!meets_age_filter(&meta, Some(30)));
    }

    #[test]
    fn meets_size_filter_none_returns_true() {
        assert!(meets_size_filter(0, None));
        assert!(meets_size_filter(1_000_000, None));
    }

    #[test]
    fn meets_size_filter_some_above_threshold_returns_true() {
        assert!(meets_size_filter(2_000_000, Some(1))); // 2 MB >= 1 MB
    }

    #[test]
    fn meets_size_filter_some_below_threshold_returns_false() {
        assert!(!meets_size_filter(500_000, Some(1))); // 0.5 MB < 1 MB
    }

    #[test]
    fn meets_size_filter_exact_match_returns_true() {
        assert!(meets_size_filter(1_048_576, Some(1))); // exactly 1 MiB
    }

    // ── DeleteDirs with filters ──────────────────────────────────────────────

    #[test]
    fn detect_with_older_than_days_filters_new_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Old dir: should be included (write_aged_dir writes file THEN sets mtime)
        let old_dir = tmp.path().join("old_cache");
        write_aged_dir(&old_dir, 30 * 86_400 + 1);
        // New dir: should be excluded (recent mtime)
        let new_dir = tmp.path().join("new_cache");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join("data"), b"x").unwrap();

        let cleaner = GenericCleaner {
            display_name: "test",
            method: CleanMethod::DeleteDirs(vec![old_dir.clone(), new_dir.clone()]),
            runner: Box::new(MissingToolRunner),
            confirm_message: None,
            fallback_delete: false,
            older_than_days: Some(30),
            larger_than_mb: None,
        };
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
        let old_size = dir_size(&old_dir);
        assert!(old_size > 0);
        assert_eq!(
            result.bytes_for_test(),
            old_size,
            "only the old dir should be counted"
        );
    }

    #[test]
    fn detect_with_larger_than_mb_filters_small_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let small_dir = tmp.path().join("small");
        fs::create_dir_all(&small_dir).unwrap();
        fs::write(small_dir.join("small_file"), b"x").unwrap();

        // Create a sufficiently large dir (write enough bytes to exceed 1 MB)
        let large_dir = tmp.path().join("large");
        fs::create_dir_all(&large_dir).unwrap();
        let large_data = vec![0u8; 2_000_000];
        fs::write(large_dir.join("large_file"), &large_data).unwrap();

        let cleaner = GenericCleaner {
            display_name: "test",
            method: CleanMethod::DeleteDirs(vec![small_dir.clone(), large_dir.clone()]),
            runner: Box::new(MissingToolRunner),
            confirm_message: None,
            fallback_delete: false,
            older_than_days: None,
            larger_than_mb: Some(1),
        };
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
        let large_size = dir_size(&large_dir);
        assert!(large_size >= 1_048_576);
        assert_eq!(
            result.bytes_for_test(),
            large_size,
            "only the large dir should be counted"
        );
    }

    #[test]
    fn detect_with_filters_removes_all_entries_when_none_match() {
        let tmp = tempfile::TempDir::new().unwrap();
        let new_dir = tmp.path().join("new");
        fs::create_dir_all(&new_dir).unwrap();
        fs::write(new_dir.join("data"), b"x").unwrap();

        let cleaner = GenericCleaner {
            display_name: "test",
            method: CleanMethod::DeleteDirs(vec![new_dir]),
            runner: Box::new(MissingToolRunner),
            confirm_message: None,
            fallback_delete: false,
            older_than_days: Some(30),
            larger_than_mb: None,
        };
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    #[test]
    fn with_config_older_than_days_filters_detect_results() {
        use filetime::FileTime;
        use std::time::{Duration, SystemTime};

        let tmp = tempfile::TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(data_dir.join("old.log"), b"x").unwrap();
        // Set directory mtime after writing file (file writes can update dir mtime)
        let old = SystemTime::now() - Duration::from_secs(7 * 86_400);
        filetime::set_file_mtime(&data_dir, FileTime::from_system_time(old)).unwrap();

        let mut per_cleaner = std::collections::HashMap::new();
        per_cleaner.insert(
            "test-older".to_string(),
            crate::config::PerCleanerConfig {
                older_than_days: Some(5),
                larger_than_mb: None,
            },
        );
        let config = crate::config::Config {
            per_cleaner,
            ..crate::config::Config::default()
        };
        let cleaner =
            GenericCleaner::delete_dirs("test-older", data_dir.clone()).with_config(&config);
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
    }

    #[test]
    fn with_config_no_match_for_name_leaves_cleaner_unchanged() {
        use crate::test_helpers::write_aged_file;

        let tmp = tempfile::TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        fs::create_dir_all(&data_dir).unwrap();
        write_aged_file(&data_dir.join("any.log"), 1, b"x");

        let mut per_cleaner = std::collections::HashMap::new();
        per_cleaner.insert(
            "other-cleaner".to_string(),
            crate::config::PerCleanerConfig {
                older_than_days: Some(999),
                larger_than_mb: None,
            },
        );
        let config = crate::config::Config {
            per_cleaner,
            ..crate::config::Config::default()
        };
        let cleaner =
            GenericCleaner::delete_dirs("no-match-cleaner", data_dir.clone()).with_config(&config);
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
    }
}
