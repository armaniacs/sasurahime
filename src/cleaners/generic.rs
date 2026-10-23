use crate::cleaner::{
    CleanCancelled, CleanOptions, CleanResult, Cleaner, PerCleanerFilters, Risk, ScanResult,
    ScanStatus, SkippedEntry,
};
use crate::command::CommandRunner;
#[cfg(test)]
use crate::command::SystemCommandRunner;
use crate::config::Config;
use crate::disk_image::{
    colima_candidates, docker_desktop_candidates, DiskImageNotice, COLIMA_REMEDY, DOCKER_REMEDY,
};
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

/// Globally suppress the secondary confirmation prompt in `CliCleaner::clean()`.
/// Call before the TUI cleaning loop and restore with `set_skip_confirm(false)` after.
pub fn set_skip_confirm(skip: bool) {
    SKIP_CONFIRM.store(skip, Ordering::Relaxed);
}

/// A cleaner that delegates the actual cleaning to an external CLI tool
/// (`program args`). When constructed with a `detect_dir`, the size is
/// measured from the filesystem and a missing tool falls back to deleting
/// that directory (when `fallback_delete` is set).
pub struct CliCleaner {
    display_name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    detect_dir: Option<PathBuf>,
    runner: Box<dyn CommandRunner>,
    /// Optional confirmation prompt shown before cleaning on an interactive TTY.
    confirm_message: Option<&'static str>,
    /// If true and the CLI tool is not found, fall back to deleting detect_dir directly.
    fallback_delete: bool,
    /// Sparse VM disk image that pruning inside the VM may not shrink.
    notice: Option<DiskImageNotice>,
    /// Risk classification (defaults to Moderate via `new`).
    risk: Risk,
}

/// A cleaner that removes a fixed list of directories, with optional age and
/// size filters. Never invokes the target tool; the runner is only used for
/// `chflags`.
pub struct PathCleaner {
    display_name: &'static str,
    dirs: Vec<PathBuf>,
    runner: Box<dyn CommandRunner>,
    /// Skip entries with mtime newer than this many days (None = no filter).
    older_than_days: Option<u32>,
    /// Skip entries with total size below this many MiB (None = no filter).
    larger_than_mb: Option<u64>,
    /// Risk classification (defaults to Moderate via `new`).
    risk: Risk,
}

impl CliCleaner {
    fn new(
        display_name: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        runner: Box<dyn CommandRunner>,
    ) -> Self {
        Self {
            display_name,
            program,
            args,
            detect_dir: None,
            runner,
            confirm_message: None,
            fallback_delete: false,
            notice: None,
            risk: Risk::Moderate,
        }
    }

    fn with_detect_dir(
        display_name: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        detect_dir: PathBuf,
        runner: Box<dyn CommandRunner>,
    ) -> Self {
        Self {
            detect_dir: Some(detect_dir),
            ..Self::new(display_name, program, args, runner)
        }
    }

    pub fn with_notice(mut self, notice: DiskImageNotice) -> Self {
        self.notice = Some(notice);
        self
    }

    /// Override the risk classification (default Moderate).
    pub fn with_risk(mut self, risk: Risk) -> Self {
        self.risk = risk;
        self
    }
}

impl PathCleaner {
    fn new(display_name: &'static str, dirs: Vec<PathBuf>, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            display_name,
            dirs,
            runner,
            older_than_days: None,
            larger_than_mb: None,
            risk: Risk::Moderate,
        }
    }

    /// Override the risk classification (default Moderate).
    pub fn with_risk(mut self, risk: Risk) -> Self {
        self.risk = risk;
        self
    }
}

impl CliCleaner {
    // ── CLI-delegating cleaners (external tool does the cleaning) ──

    pub fn bun(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("bun", "bun", &["pm", "cache", "rm"], runner)
    }

    pub fn go(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("go", "go", &["clean", "-cache"], runner)
    }

    pub fn pip(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("pip", "pip", &["cache", "purge"], runner)
    }

    pub fn npm(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("npm", "npm", &["cache", "clean", "--force"], runner)
    }

    pub fn yarn(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("yarn", "yarn", &["cache", "clean"], runner)
    }

    pub fn pnpm(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("pnpm", "pnpm", &["store", "prune"], runner)
    }

    pub fn deno(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("deno", "deno", &["cache", "-r"], runner)
    }

    pub fn pipx(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("pipx", "pipx", &["cache", "purge"], runner)
    }

    pub fn docker(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new("docker", "docker", &["system", "prune", "-f"], runner).with_notice(
            DiskImageNotice::new("docker", docker_desktop_candidates(home), DOCKER_REMEDY),
        )
    }

    pub fn orbstack(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("orbstack", "orb", &["prune"], runner)
    }

    pub fn gem(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("gem", "gem", &["cleanup"], runner)
    }

    pub fn bundle(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("bundle", "bundle", &["clean"], runner)
    }

    pub fn dotnet(runner: Box<dyn CommandRunner>) -> Self {
        Self::new(
            "dotnet",
            "dotnet",
            &["nuget", "locals", "all", "--clear"],
            runner,
        )
    }

    pub fn simulator(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::with_detect_dir(
            "simulator",
            "xcrun",
            &["simctl", "delete", "unavailable"],
            home.join("Library/Developer/CoreSimulator"),
            runner,
        )
    }

    pub fn colima_prune(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            confirm_message: Some("This will delete ALL stopped Colima VM disk data (containers, images, volumes). Continue?"),
            fallback_delete: true,
            ..Self::with_detect_dir(
                "colima",
                "colima",
                &["prune", "--all", "--force"],
                home.join(".colima"),
                runner,
            )
            .with_risk(Risk::Safe)
        }
        .with_notice(DiskImageNotice::new(
            "colima",
            colima_candidates(home),
            COLIMA_REMEDY,
        ))
    }

    pub fn cocoapods(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("cocoapods", "pod", &["cache", "clean", "--all"], runner)
    }

    pub fn maven(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::with_detect_dir(
            "maven",
            "mvn",
            &["dependency:purge-local-repository"],
            home.join(".m2/repository"),
            runner,
        )
    }

    pub fn conda(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("conda", "conda", &["clean", "--all", "-y"], runner)
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
        Self::with_detect_dir("flutter", "dart", &["pub", "cache", "clean"], cache, runner)
    }

    pub fn poetry(runner: Box<dyn CommandRunner>) -> Self {
        Self::new("poetry", "poetry", &["cache", "clear", "--all"], runner)
    }
}

impl PathCleaner {
    // ── path-based cleaners (fixed directory list, optional filters) ──

    pub fn node_gyp(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new(
            "node-gyp",
            vec![
                home.join(".cache/node-gyp"),
                home.join("Library/Caches/node-gyp"),
            ],
            runner,
        )
    }

    pub fn spm_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        let cache = home.join("Library/Caches/org.swift.swiftpm");
        Self::new("spm", vec![cache], runner)
    }

    pub fn trash(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new("trash", vec![home.join(".Trash")], runner)
    }

    pub fn downloads(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new("downloads", vec![home.join("Downloads")], runner)
            .with_older_than(30)
            .with_risk(Risk::Manual)
    }

    pub fn cargo_registry(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new(
            "cargo-registry",
            vec![home.join(".cargo/registry/cache")],
            runner,
        )
    }

    pub fn vscode_extensions(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new(
            "vscode-extensions",
            vec![home.join(".vscode/extensions")],
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
        Self::new("terraform", vec![cache], runner)
    }

    pub fn volta(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new("volta", vec![home.join(".volta/cache")], runner)
    }

    pub fn sbt(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new(
            "sbt",
            vec![home.join(".sbt"), home.join(".ivy2/cache")],
            runner,
        )
    }

    pub fn tree_sitter(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::new("tree-sitter", vec![home.join(".cache/tree-sitter")], runner)
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
        Self::new("act", vec![cache_dir], runner).with_risk(Risk::Safe)
    }
}

impl PathCleaner {
    /// Create a `DeleteDirs` cleaner with a given display name and target path.
    /// The runner is a no-op since path cleaners never invoke target commands.
    #[cfg(test)]
    pub fn delete_dirs(name: &'static str, path: PathBuf) -> Self {
        Self::new(name, vec![path], Box::new(SystemCommandRunner))
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
        self.dirs.first().map(|d| d.to_string_lossy().to_string())
    }
}

impl CliCleaner {
    fn primary_target_display(&self) -> Option<String> {
        self.detect_dir
            .as_ref()
            .map(|d| d.to_string_lossy().to_string())
    }
}

impl Cleaner for CliCleaner {
    fn per_cleaner_filters(&self) -> PerCleanerFilters {
        PerCleanerFilters::None
    }

    fn is_available(&self) -> bool {
        self.detect_dir.is_some() && self.fallback_delete || self.runner.exists(self.program)
    }

    fn advisory(&self) -> Option<String> {
        self.notice.as_ref().and_then(|n| n.render())
    }

    fn risk(&self) -> Risk {
        self.risk
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

        match &self.detect_dir {
            Some(detect_dir) => {
                if !detect_dir.exists()
                    || (!self.runner.exists(self.program) && !self.fallback_delete)
                {
                    return make_result(ScanStatus::NotFound);
                }
                let bytes = crate::scan_cache::cached_dir_size(detect_dir);
                make_result(if bytes > 0 {
                    ScanStatus::Pruneable(bytes)
                } else {
                    ScanStatus::Clean
                })
            }
            None => {
                if !self.runner.exists(self.program) {
                    return make_result(ScanStatus::NotFound);
                }
                // The tool is installed but detect cannot measure a byte count
                // without running it: report unknown instead of a lying 0 B.
                make_result(ScanStatus::Unknown)
            }
        }
    }

    fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let program = self.program;
        let args = self.args;
        match &self.detect_dir {
            None => {
                if !self.runner.exists(program) {
                    println!("{}: not found, skipping", self.display_name);
                    return Ok(CleanResult::empty(self.name()));
                }
                if dry_run {
                    println!("[dry-run] would run: {program} {}", args.join(" "));
                    return Ok(CleanResult::empty(self.name()));
                }
                println!("$ {program} {}", args.join(" "));
                let image_before = self.notice.as_ref().map_or(0, |n| n.physical_total());
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
                } else if let Some(msg) = self
                    .notice
                    .as_ref()
                    .and_then(|n| n.after_clean_message(image_before, n.physical_total()))
                {
                    println!("{msg}");
                }
                Ok(CleanResult {
                    name: self.name(),
                    bytes_freed: 0,
                    uses_trash: false,
                    skipped: vec![],
                    deleted_paths: vec![],
                })
            }
            Some(detect_dir) => {
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
                            return Ok(CleanResult::empty(self.name()));
                        }
                        // TOCTOU mitigation: re-validate fallback delete path.
                        if !is_safe_delete_target(detect_dir) {
                            eprintln!(
                                "[{}] warning: unsafe fallback path, skipping: {}",
                                self.display_name,
                                detect_dir.display()
                            );
                            return Ok(CleanResult {
                                name: self.name(),
                                bytes_freed: 0,
                                uses_trash: crate::trash::is_trash_mode(),
                                skipped: vec![],
                                deleted_paths: vec![],
                            });
                        }
                        let outcome = crate::trash::safe_delete(
                            self.runner.as_ref(),
                            detect_dir,
                            crate::trash::TrashMode::Global,
                        )?;
                        if let Some(reason) = outcome.skip_reason {
                            return Ok(CleanResult {
                                name: self.name(),
                                bytes_freed: 0,
                                uses_trash: outcome.uses_trash,
                                skipped: vec![SkippedEntry {
                                    path: detect_dir.to_path_buf(),
                                    reason,
                                }],
                                deleted_paths: vec![],
                            });
                        }
                        println!(
                            "[{}] removed cache: {}",
                            self.display_name,
                            detect_dir.display()
                        );
                        return Ok(CleanResult::freed(
                            self.name(),
                            size_before,
                            outcome.uses_trash,
                        ));
                    }
                    println!("{}: not found, skipping", self.display_name);
                    return Ok(CleanResult::empty(self.name()));
                }
                if dry_run {
                    println!("[dry-run] would run: {program} {}", args.join(" "));
                    if size_before > 0 {
                        println!(
                            "[dry-run] would free: {}",
                            crate::format::format_bytes(size_before)
                        );
                    }
                    return Ok(CleanResult::empty(self.name()));
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
                    deleted_paths: vec![],
                })
            }
        }
    }
}

impl Cleaner for PathCleaner {
    fn per_cleaner_filters(&self) -> PerCleanerFilters {
        PerCleanerFilters::Both
    }

    fn is_available(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        self.display_name
    }

    fn risk(&self) -> Risk {
        self.risk
    }

    fn detect(&self) -> ScanResult {
        let make_result = |status| {
            let mut r = ScanResult::new(self.name(), status);
            if crate::context::is_verbose() {
                r.primary_target = self.primary_target_display();
            }
            r
        };
        let entries: Vec<(PathBuf, u64)> = self
            .dirs
            .iter()
            .filter(|d| d.exists())
            .filter_map(|d| {
                let meta = fs::metadata(d).ok()?;
                if !meets_age_filter(&meta, self.older_than_days) {
                    return None;
                }
                let size = crate::scan_cache::cached_dir_size(d);
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

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let any_filter = self.older_than_days.is_some() || self.larger_than_mb.is_some();
        // Plain clean keeps the process-global trash mode (compat path).
        self.clean_filtered(
            dry_run,
            reporter,
            self.older_than_days,
            self.larger_than_mb,
            any_filter,
            crate::trash::TrashMode::Global,
        )
    }

    /// Applies the combined per-cleaner + global filters. Global filters are
    /// the user's explicit intent and combine with the cleaner's own config
    /// (the stricter value wins). Out-of-condition entries are reported with
    /// a skip reason when global filters are active.
    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> Result<CleanResult> {
        let older = match (self.older_than_days, opts.older_than_days) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let larger = match (self.larger_than_mb, opts.larger_than_mb) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        let any_filter = older.is_some() || larger.is_some();
        let mode = opts.trash_mode.unwrap_or(crate::trash::TrashMode::Global);
        self.clean_filtered(dry_run, reporter, older, larger, any_filter, mode)
    }
}

impl PathCleaner {
    fn clean_filtered(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        older_than_days: Option<u32>,
        larger_than_mb: Option<u64>,
        explain_skips: bool,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        let mut freed: u64 = 0;
        for d in &self.dirs {
            if !d.exists() {
                continue;
            }
            let age_ok = fs::metadata(d)
                .map(|m| meets_age_filter(&m, older_than_days))
                .unwrap_or(false);
            let size = dir_size(d);
            let size_ok = meets_size_filter(size, larger_than_mb);
            if !age_ok {
                if explain_skips {
                    if let Some(days) = older_than_days {
                        println!("Skipped {}: younger than {days} day(s)", d.display());
                    }
                }
                continue;
            }
            if !size_ok {
                if explain_skips {
                    if let Some(mb) = larger_than_mb {
                        println!("Skipped {}: smaller than {mb} MB", d.display());
                    }
                }
                continue;
            }
            freed += self.clean_one(d, size, dry_run, reporter, mode)?;
        }
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: mode.resolve(),
            skipped: vec![],
            deleted_paths: vec![],
        })
    }

    fn clean_one(
        &self,
        dir: &Path,
        size: u64,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<u64> {
        if dry_run {
            println!("[dry-run] would remove: {}", dir.display());
            return Ok(0);
        }
        // Verify path safety at clean-time (TOCTOU mitigation for
        // env-var-derived paths: terraform, flutter, act).
        if !is_safe_delete_target(dir) {
            eprintln!(
                "[{}] warning: unsafe path, skipping: {}",
                self.display_name,
                dir.display()
            );
            return Ok(0);
        }
        reporter.progress_tick(dir, 1, size);
        let outcome = crate::trash::safe_delete(self.runner.as_ref(), dir, mode)?;
        if let Some(reason) = outcome.skip_reason {
            log::warn!(
                "[{}] skipping {}: {reason}",
                self.display_name,
                dir.display()
            );
            return Ok(0);
        }
        println!("Removed: {}", dir.display());
        Ok(size)
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
    // Block HOME itself and every ancestor of HOME: an env-var redirect
    // (ACT_CACHE_DIR=$HOME, TF_PLUGIN_CACHE_DIR=/Users) must never be
    // deletable, and subdirectories of HOME stay allowed.
    let home = std::env::var("HOME").unwrap_or_default();
    if !home.is_empty() {
        // HOME must be canonicalized with the target: a symlinked HOME would
        // otherwise defeat this check (canonical(HOME) != raw HOME).
        let home_path = fs::canonicalize(&home).unwrap_or_else(|_| PathBuf::from(&home));
        if check == home_path || home_path.starts_with(&check) {
            return false;
        }
    }
    // A relative path resolves against the process CWD: env vars like
    // XDG_CACHE_HOME="" must never yield a deletable target.
    if !check.is_absolute() {
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
        && !is_sys("/private/var/tmp")
        && !is_sys("/private/var/root")
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
        Some(mb) => size >= mb.saturating_mul(1_048_576),
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
        return Ok(CleanResult::empty(name));
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
            return Ok(CleanResult::empty(name));
        }
        let size_before = dir_size(dir);
        let output = runner.run(config.tool, config.args)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            log::warn!(
                "{} failed (exit {:?}): {stderr}",
                config.tool,
                output.status.code()
            );
            anyhow::bail!(
                "{} failed with exit code {:?}",
                config.tool,
                output.status.code()
            );
        }
        println!("[{name}] ran {} {}", config.tool, config.args.join(" "));
        return Ok(CleanResult::freed(name, size_before, false));
    }

    // Fallback: delete directly
    let size = dir_size(dir);
    if dry_run {
        println!(
            "[dry-run] [{name}] would remove: {} ({})",
            dir.display(),
            crate::format::format_bytes(size)
        );
        return Ok(CleanResult::empty(name));
    }
    // Clean-time safety re-validation, same as the other deletion sinks:
    // env-var-derived dirs are only checked at construction. This is
    // validate-then-use (not an atomic guarantee): a concurrent swap in the
    // parent dir between this check and the delete remains possible.
    if !is_safe_delete_target(dir) {
        eprintln!("[{name}] warning: unsafe path, skipping: {}", dir.display());
        return Ok(CleanResult::empty(name));
    }

    let outcome = crate::trash::safe_delete(runner, dir, crate::trash::TrashMode::Global)?;
    if let Some(reason) = outcome.skip_reason {
        return Ok(CleanResult {
            name,
            bytes_freed: 0,
            uses_trash: outcome.uses_trash,
            skipped: vec![SkippedEntry {
                path: dir.to_path_buf(),
                reason,
            }],
            deleted_paths: vec![],
        });
    }
    if config.recreate {
        fs::create_dir_all(dir)?;
    }
    println!("[{name}] removed cache: {}", dir.display());
    Ok(CleanResult::freed(name, size, outcome.uses_trash))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRunner;

    #[test]
    fn clean_with_opts_trash_mode_overrides_the_global() {
        // CleanOptions.trash_mode must pin the deletion mode: uses_trash and
        // the actual deletion can never disagree with the global read.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        // PathCleaner::downloads targets HOME/Downloads and carries a
        // 30-day age filter, so the dir mtime must be old.
        let dir = tmp.path().join("Downloads");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let status = std::process::Command::new("touch")
            .args(["-t", "202001010000"])
            .arg(&dir)
            .status()
            .unwrap();
        assert!(status.success());
        // Simulate the global default: trash. A Trash op would be TCC-denied
        // here, so the pinned permanent mode is observable.
        crate::trash::set_trash_mode(true);
        let cleaner =
            PathCleaner::downloads(tmp.path(), Box::new(crate::command::SystemCommandRunner));
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: Some(crate::trash::TrashMode::Permanent),
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean_with_opts(false, &reporter, &opts).unwrap();
        assert!(
            !dir.exists(),
            "the pinned permanent mode must delete for real"
        );
        assert!(
            !result.uses_trash,
            "uses_trash must come from the pinned mode, not the global"
        );
        crate::trash::set_trash_mode(false);
        drop(_lock);
    }

    #[test]
    fn clean_cli_or_fallback_revalidates_safety_at_clean_time() {
        // A dir that is unsafe at clean time (here: HOME itself) must be
        // skipped by the fallback delete, not removed.
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        std::fs::write(home.join("keep.txt"), b"data").unwrap();
        let config = CliFallbackConfig {
            tool: "definitely-not-installed-tool",
            args: &[],
            recreate: false,
        };
        let result = clean_cli_or_fallback(
            "test-fallback",
            &home,
            &crate::command::SystemCommandRunner,
            &config,
            false,
        )
        .unwrap();
        assert_eq!(
            result.bytes_freed, 0,
            "an unsafe dir must not be deleted by the fallback"
        );
        assert!(
            home.join("keep.txt").exists(),
            "the unsafe dir must survive"
        );
    }

    struct MissingToolRunner;
    impl CommandRunner for MissingToolRunner {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            unreachable!()
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }

    // ── type split: per_cleaner_filters / is_available live on each type ──

    #[test]
    fn cli_cleaner_per_cleaner_filters_are_none() {
        let cleaner = CliCleaner::bun(Box::new(crate::test_helpers::MockRunner::new()));
        assert!(matches!(
            cleaner.per_cleaner_filters(),
            PerCleanerFilters::None
        ));
    }

    #[test]
    fn path_cleaner_per_cleaner_filters_are_both() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = PathCleaner::node_gyp(
            tmp.path(),
            Box::new(crate::test_helpers::MockRunner::new().with_not_found()),
        );
        assert!(matches!(
            cleaner.per_cleaner_filters(),
            PerCleanerFilters::Both
        ));
    }

    #[test]
    fn path_cleaner_is_always_available() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = PathCleaner::tree_sitter(tmp.path(), Box::new(MissingToolRunner));
        assert!(cleaner.is_available());
    }

    #[test]
    fn cli_cleaner_availability_follows_program_presence() {
        let present = CliCleaner::bun(Box::new(
            crate::test_helpers::MockRunner::new().with_success("bun"),
        ));
        assert!(present.is_available());
        let missing = CliCleaner::bun(Box::new(MissingToolRunner));
        assert!(!missing.is_available());
    }

    #[test]
    fn cli_cleaner_with_detect_dir_and_fallback_is_always_available() {
        // fallback_delete cleaners (colima) are available even when the CLI
        // tool is missing, because they fall back to deleting the dir.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        assert!(cleaner.is_available());
    }

    #[test]
    fn command_with_detect_dir_fallback_reports_pruneable_when_tool_missing() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Directory exists but the cleaning tool is not installed;
        // fallback_delete should allow detect to report Pruneable anyway.
        fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
        fs::write(tmp.path().join(".colima/_lima/colima/dummy.img"), b"x").unwrap();

        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
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
        let cleaner = CliCleaner::simulator(tmp.path(), Box::new(MissingToolRunner));
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
    fn is_safe_delete_target_rejects_home_and_ancestors() {
        // ACT_CACHE_DIR=$HOME or a /Users redirect must never be deletable:
        // an env var can point a Risk::Safe target at the whole home.
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().canonicalize().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &home.to_string_lossy());
        assert!(
            !is_safe_delete_target(&home),
            "HOME itself is never a delete target"
        );
        assert!(
            !is_safe_delete_target(home.parent().unwrap()),
            "an ancestor of HOME is never a delete target"
        );
        // Subdirectories of HOME stay deletable.
        let cache = home.join(".cache/act");
        std::fs::create_dir_all(&cache).unwrap();
        assert!(
            is_safe_delete_target(&cache),
            "subdirs of HOME remain deletable"
        );
    }

    #[test]
    fn is_safe_delete_target_canonicalizes_home() {
        // A symlinked HOME must not defeat the guard: a target equal to
        // canonical(HOME) is still HOME.
        let real = tempfile::TempDir::new().unwrap();
        let link_dir = tempfile::TempDir::new().unwrap();
        let link = link_dir.path().join("linkhome");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("HOME", &link.to_string_lossy());
        let canonical_home = std::fs::canonicalize(real.path()).unwrap();
        assert!(
            !is_safe_delete_target(&canonical_home),
            "canonical(HOME) is still HOME even when HOME itself is a symlink"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_relative_paths() {
        // A relative env-derived target resolves against the process CWD:
        // never a safe delete target.
        assert!(!is_safe_delete_target(Path::new("relative/cache")));
        assert!(!is_safe_delete_target(Path::new("pre-commit")));
    }

    #[test]
    fn is_safe_delete_target_allows_home_cache() {
        assert!(is_safe_delete_target(Path::new("/Users/test/.cache/act")));
        assert!(is_safe_delete_target(Path::new(
            "/Users/test/Library/Caches"
        )));
    }

    #[test]
    fn meets_size_filter_saturates_on_overflow() {
        // mb = 2^44 wraps mb * 1_048_576 to 0, which would pass every size
        // and delete more than configured.
        assert!(
            !meets_size_filter(0, Some(1 << 44)),
            "an overflowing threshold must saturate, not wrap to 0"
        );
        assert!(meets_size_filter(1_048_576, Some(1)));
        assert!(!meets_size_filter(0, Some(1)));
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
    fn is_safe_delete_target_canonicalize_fallback_nonexistent() {
        let p = Path::new("/nonexistent_dir_for_test/deep/path");
        assert!(
            is_safe_delete_target(p),
            "non-existent path should fall through (not in blocklist)"
        );
    }

    #[test]
    fn is_safe_delete_target_canonicalize_fallback_rejects_dotdot() {
        let p = Path::new("/nonexistent/../../etc");
        assert!(
            !is_safe_delete_target(p),
            "path with .. must be rejected even if uncanonicalizable"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_private_var_tmp() {
        assert!(
            !is_safe_delete_target(Path::new("/private/var/tmp")),
            "/private/var/tmp must be rejected"
        );
        assert!(
            !is_safe_delete_target(Path::new("/private/var/tmp/some-dir")),
            "/private/var/tmp/* must be rejected"
        );
    }

    #[test]
    fn is_safe_delete_target_rejects_private_var_root() {
        assert!(
            !is_safe_delete_target(Path::new("/private/var/root")),
            "/private/var/root must be rejected"
        );
    }

    #[test]
    fn act_path_validates_env_var_and_falls_back() {
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let tmp = tempfile::TempDir::new().unwrap();
        let _guard = crate::test_helpers::EnvGuard::set("ACT_CACHE_DIR", "/");
        let cleaner = PathCleaner::act(tmp.path(), Box::new(crate::command::SystemCommandRunner));
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    // ── primary_target_display tests ────────────────────────────────────────

    #[test]
    fn primary_target_display_for_command_returns_none() {
        let cleaner = CliCleaner::bun(Box::new(MissingToolRunner));
        assert!(cleaner.primary_target_display().is_none());
    }

    #[test]
    fn primary_target_display_for_command_with_detect_dir_returns_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let target = cleaner.primary_target_display();
        assert!(target.is_some());
        assert!(target.unwrap().contains(".colima"));
    }

    #[test]
    fn primary_target_display_for_delete_dirs_returns_first_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = PathCleaner::node_gyp(tmp.path(), Box::new(MissingToolRunner));
        let target = cleaner.primary_target_display();
        assert!(target.is_some());
        let path = target.unwrap();
        assert!(path.contains(".cache/node-gyp"));
    }

    #[test]
    fn detect_includes_primary_target_when_verbose() {
        let _guard = crate::test_helpers::VerboseGuard::new();
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/act");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("dummy"), b"x").unwrap();

        let cleaner = PathCleaner::act(tmp.path(), Box::new(MissingToolRunner));
        let result = cleaner.detect();
        assert!(
            result.primary_target.is_some(),
            "primary_target should be set when verbose"
        );
        assert!(
            result.primary_target.unwrap().contains(".cache/act"),
            "target should point to act cache dir"
        );
    }

    #[test]
    fn detect_omits_primary_target_when_not_verbose() {
        let _guard = crate::test_helpers::VerboseGuard::with_value(false);
        let tmp = tempfile::TempDir::new().unwrap();
        let cache = tmp.path().join(".cache/act");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("dummy"), b"x").unwrap();

        let cleaner = PathCleaner::act(tmp.path(), Box::new(MissingToolRunner));
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

        let cleaner = PathCleaner {
            display_name: "test",
            dirs: vec![old_dir.clone(), new_dir.clone()],
            runner: Box::new(MissingToolRunner),
            risk: Risk::Moderate,
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

        let cleaner = PathCleaner {
            display_name: "test",
            dirs: vec![small_dir.clone(), large_dir.clone()],
            runner: Box::new(MissingToolRunner),
            risk: Risk::Moderate,
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

        let cleaner = PathCleaner {
            display_name: "test",
            dirs: vec![new_dir],
            runner: Box::new(MissingToolRunner),
            risk: Risk::Moderate,
            older_than_days: Some(30),
            larger_than_mb: None,
        };
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::NotFound));
    }

    #[test]
    fn with_config_older_than_days_filters_detect_results() {
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
        let cleaner = PathCleaner::delete_dirs("test-older", data_dir.clone()).with_config(&config);
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
            PathCleaner::delete_dirs("no-match-cleaner", data_dir.clone()).with_config(&config);
        let result = cleaner.detect();
        assert!(matches!(result.status, ScanStatus::Pruneable(_)));
    }

    fn write_sparse_image(path: &Path) {
        use std::io::Write;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![1u8; 2 * 1024 * 1024]).unwrap();
        f.set_len(64 * 1024 * 1024).unwrap();
    }

    #[test]
    fn docker_advisory_reports_virtual_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_sparse_image(
            &tmp.path()
                .join("Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"),
        );
        let runner = crate::test_helpers::MockRunner::new().with_success("docker");
        let cleaner = CliCleaner::docker(tmp.path(), Box::new(runner));
        let text = cleaner.advisory().expect("advisory when Docker.raw exists");
        assert!(text.contains("Docker.raw"), "{text}");
        assert!(text.contains("Disk image size"), "{text}");
    }

    #[test]
    fn docker_advisory_is_none_without_virtual_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let runner = crate::test_helpers::MockRunner::new().with_success("docker");
        assert!(CliCleaner::docker(tmp.path(), Box::new(runner))
            .advisory()
            .is_none());
    }

    #[test]
    fn docker_detect_does_not_run_commands_and_stays_unknown() {
        let tmp = tempfile::TempDir::new().unwrap();
        struct ExistsOnlyRunner;
        impl CommandRunner for ExistsOnlyRunner {
            fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
                unreachable!("detect must not execute external commands")
            }
            fn exists(&self, _: &str) -> bool {
                true
            }
        }
        let cleaner = CliCleaner::docker(tmp.path(), Box::new(ExistsOnlyRunner));
        assert!(matches!(cleaner.detect().status, ScanStatus::Unknown));
    }

    #[test]
    fn colima_advisory_reports_disk_images() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_sparse_image(&tmp.path().join(".colima/_lima/colima/disk"));
        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let text = cleaner
            .advisory()
            .expect("advisory when colima disk exists");
        assert!(text.contains("[colima]"), "{text}");
        assert!(text.contains("fstrim"), "{text}");
    }

    #[test]
    fn non_vm_cleaner_has_no_advisory() {
        let cleaner = CliCleaner::npm(Box::new(MissingToolRunner));
        assert!(cleaner.advisory().is_none());
    }
}
