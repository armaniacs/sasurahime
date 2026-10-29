use crate::cleaner::{
    CleanCancelled, CleanOptions, CleanResult, Cleaner, PerCleanerFilters, Risk, ScanResult,
    ScanStatus, SkipCause, SkippedEntry,
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
use crate::prompt::{self, PromptPort, TtyPrompt};
use crate::safety::{is_safe_delete_target, resolve_env_dir};
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

// Module responsibility boundary (PBI 57): this module carries the two
// CLI-tool cleaner shapes — `CliCleaner` (a tool run with an optional
// detect-dir fallback) and `PathCleaner` (a filesystem path with a size
// threshold). They are different responsibilities, so a split into two
// modules was considered; it was deliberately NOT done here because the
// constructor correspondence test (`constructors_have_matching_primary_...`
// and the per-target dispatch table in `main.rs`) fixes the pair's boundary:
// every named constructor exists on both types and a split would move the
// pairs apart without removing any coupling. The deletion-safety policy
// (PBI 57) and the CLI-fallback adapter now live in their own modules
// (`crate::safety`, `crate::cli_fallback`), so this file no longer carries
// those two responsibilities.

/// A cleaner that delegates the actual cleaning to an external CLI tool
/// (`program args`). When constructed with a `detect_dir`, the size is
/// measured from the filesystem and a missing tool falls back to deleting
/// that directory (when `fallback_delete` is set).
pub struct CliCleaner {
    display_name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    /// Commands run after the main one. Each entry is (program, args,
    /// failure remedy). A failure is non-fatal: it prints a warning and
    /// the remedy, then the clean continues.
    extra_commands: Vec<(&'static str, &'static [&'static str], Option<&'static str>)>,
    detect_dir: Option<PathBuf>,
    runner: Box<dyn CommandRunner>,
    /// Optional confirmation prompt shown before cleaning on an interactive TTY.
    /// Explanation printed above the confirm prompt. The Confirm prompt
    /// itself is a short "Proceed?" line — a long Confirm prompt wraps, and
    /// dialoguer only clears one terminal line, doubling the message.
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
            extra_commands: Vec::new(),
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

    /// Add commands run after the main one. Each entry is (program, args,
    /// failure remedy); the remedy prints when the command fails. Extras
    /// run only for cleaners constructed with a detect_dir — the
    /// no-detect-dir clean branch never executes them.
    pub fn with_extra_commands(
        mut self,
        cmds: Vec<(&'static str, &'static [&'static str], Option<&'static str>)>,
    ) -> Self {
        self.extra_commands.extend(cmds);
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
            confirm_message: Some(
                "Prunes cached Colima assets and asks the running VM to discard unused disk blocks (fstrim); no containers, images or volumes are deleted. If colima is not found, the ~/.colima directory (including its VM disk) is deleted instead.",
            ),
            fallback_delete: true,
            ..Self::with_detect_dir(
                "colima",
                "colima",
                &["prune", "--all", "--force"],
                home.join(".colima"),
                runner,
            )
            // Prune only clears colima's downloaded-asset cache; the space
            // the detection measures lives in the VM's sparse disk images,
            // which shrink only when the VM discards unused blocks.
            // Non-fatal: with the VM stopped the remedy prints and the
            // prune-only freed bytes are still reported.
            .with_extra_commands(vec![(
                "colima",
                &["ssh", "--", "sudo", "fstrim", "-av"],
                Some(COLIMA_REMEDY),
            )])
            .with_risk(Risk::Risky)
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
        let cache = resolve_env_dir("PUB_CACHE", "", home.join(".pub-cache"), "flutter");
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
        let cache = resolve_env_dir(
            "TF_PLUGIN_CACHE_DIR",
            "",
            home.join(".terraform.d/plugin-cache"),
            "terraform",
        );
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
        let cache_dir = resolve_env_dir("ACT_CACHE_DIR", "", home.join(".cache/act"), "act");
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

    /// Cleans with the run's options: `opts.trash_mode` is the resolved
    /// deletion mode for the fallback delete, so the seam outcome and the
    /// actual deletion can never disagree (the `CleanOptions` contract).
    fn clean(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.clean_with_mode(dry_run, reporter, opts.trash_mode)
    }
}

impl CliCleaner {
    /// The shared clean body. `mode` is the resolved deletion mode for the
    /// fallback delete: the seam outcome and the deletion agree by
    /// construction. The reporter suspends the live progress line while the
    /// confirm prompt owns the screen.
    fn clean_with_mode(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
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
                // PBI 69 (WHY the skip is not enough): the TUI's suppression
                // means the user already confirmed on a terminal, but a
                // non-TTY stdin means the confirm cannot be shown at all.
                // When this cleaner's destructive action (see the fallback
                // delete below) is reachable on that non-TTY stdin, the
                // refusal gate there stops it instead — colima's full
                // deletion may only run on a TTY where the confirm was
                // actually answered.
                if let Some(msg) = &self.confirm_message {
                    if !prompt::confirm_suppressed() && prompt::is_interactive() {
                        // The long explanation prints as normal output and
                        // the Confirm prompt stays one short line: dialoguer
                        // clears one line deep, so a wrapped prompt would
                        // double the message (the confirm-duplication fix).
                        println!("\n{msg}");
                        // The prompt owns the screen (suspension and the
                        // one-line Confirm prompt are owned by the port, so
                        // the live progress line never redraws under it).
                        let confirmed = TtyPrompt.confirm(reporter, "Proceed?")?;
                        if !confirmed {
                            println!("{}: cancelled", self.display_name);
                            eprintln!("  $ {program} {}", args.join(" "));
                            return Err(anyhow::Error::from(CleanCancelled));
                        }
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
                        // PBI 69 (WHY): this fallback delete removes the
                        // whole detect_dir tree — for colima, ~/.colima
                        // including the VM's disk images. That full
                        // deletion is the destructive action this
                        // cleaner's confirm exists for, and it must only
                        // run on a TTY: a non-TTY stdin cannot show the
                        // confirm (the gate above silently skips it), so
                        // refuse here with the standard remedy instead of
                        // deleting unconfirmed — the same non-TTY floor
                        // as ios-backup and apfs-snapshot. Only cleaners
                        // that declare a confirm are gated: the run-CLI
                        // path (a real `colima prune`) is a tool
                        // invocation, not the deletion.
                        if self.confirm_message.is_some()
                            && !prompt::gate_interactive(self.display_name)
                        {
                            return Ok(CleanResult::empty(self.name()));
                        }
                        // TOCTOU mitigation: re-validate fallback delete path.
                        if !is_safe_delete_target(detect_dir) {
                            eprintln!(
                                "[{}] warning: unsafe fallback path, skipping: {}",
                                self.display_name,
                                detect_dir.display()
                            );
                            // The fallback was refused before any deletion: the
                            // seam was never invoked, so no Trash claim may be
                            // made (same as the free `clean_cli_or_fallback`'s
                            // unsafe-path return).
                            return Ok(CleanResult {
                                name: self.name(),
                                bytes_freed: 0,
                                uses_trash: false,
                                skipped: vec![],
                                deleted_paths: vec![],
                            });
                        }
                        let outcome =
                            crate::trash::safe_delete(self.runner.as_ref(), detect_dir, mode)?;
                        if let Some(reason) = outcome.skip_reason {
                            // The seam classified the failure as skippable: the
                            // entry was NOT deleted, so no Trash claim may be
                            // made even though the seam resolved the mode (same
                            // as the unsafe-path return above).
                            return Ok(CleanResult {
                                name: self.name(),
                                bytes_freed: 0,
                                uses_trash: false,
                                skipped: vec![SkippedEntry {
                                    path: detect_dir.to_path_buf(),
                                    reason: SkipCause::Delete(reason),
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
                // Measure the sparse disk image before any command runs: the
                // after-clean message below must diff the pre-clean
                // allocation, exactly like the no-detect_dir branch.
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
                }
                for (extra_program, extra_args, remedy) in &self.extra_commands {
                    println!("$ {extra_program} {}", extra_args.join(" "));
                    match self.runner.run(extra_program, extra_args) {
                        Ok(output) if !output.status.success() => {
                            eprintln!(
                                "[{}] warning: `{} {}` exited with code {:?}",
                                self.display_name,
                                extra_program,
                                extra_args.join(" "),
                                output.status.code()
                            );
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            if !stderr.is_empty() {
                                eprintln!("[{}] stderr: {stderr}", self.display_name);
                            }
                            if let Some(remedy) = remedy {
                                eprintln!("[{}] {remedy}", self.display_name);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!(
                                "[{}] warning: `{} {}` failed: {e}",
                                self.display_name,
                                extra_program,
                                extra_args.join(" ")
                            );
                            if let Some(remedy) = remedy {
                                eprintln!("[{}] {remedy}", self.display_name);
                            }
                        }
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
                // The disk-image notice must stay live on the detect_dir
                // clean path: `Freed:` above measures the cache directory,
                // not the VM's sparse disk image, and fstrim only reclaims
                // unused blocks — the image file itself often does not
                // shrink, which is exactly what the message reports (with
                // the remedy when it does not). Success-only, same as the
                // no-detect_dir branch; colima is the only cleaner with
                // both a detect_dir and a notice.
                if output.status.success() {
                    if let Some(msg) = self
                        .notice
                        .as_ref()
                        .and_then(|n| n.after_clean_message(image_before, n.physical_total()))
                    {
                        println!("{msg}");
                    }
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

    /// Applies the combined per-cleaner + global filters. Global filters are
    /// the user's explicit intent and combine with the cleaner's own config
    /// (the stricter value wins). Out-of-condition entries are reported with
    /// a skip reason when global filters are active.
    fn clean(
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
        let mode = opts.trash_mode;
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
        // Caller-specific decisions stay here: per-dir existence, the age
        // and size filters, and the dry-run printing. The deletion
        // execution rides the delete ledger, which owns the per-entry
        // contracts (progress, skip records, freed sums, deleted paths,
        // CleanResult assembly).
        let mut items: Vec<(PathBuf, u64)> = Vec::new();
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
            if dry_run {
                println!("[dry-run] would remove: {}", d.display());
                continue;
            }
            items.push((d.clone(), size));
        }
        if dry_run {
            return Ok(CleanResult::empty(self.name()));
        }
        // There is no caller-side unsafe-path guard here anymore: the
        // seam's scope tripwire refuses the same paths at deletion time,
        // and the ledger records the refusal as a skipped entry (prose
        // reason) and continues — the skip channel and the deleted-path
        // audit trail stay consistent with every other ledger user.
        Ok(crate::trash::delete_ledger(
            self.display_name,
            reporter,
            &items,
            crate::trash::seam_delete(self.runner.as_ref(), mode),
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRunner;
    use std::sync::atomic::Ordering;

    #[test]
    fn clean_with_opts_pinned_permanent_deletes_for_real() {
        // CleanOptions.trash_mode must own the deletion mode: uses_trash and
        // the actual deletion can never disagree.
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
        // here, so the pinned permanent mode is observable. The guard
        // restores the previous mode on drop, even when an assert fails.
        let cleaner =
            PathCleaner::downloads(tmp.path(), Box::new(crate::command::SystemCommandRunner));
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Permanent,
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(
            !dir.exists(),
            "the pinned permanent mode must delete for real"
        );
        assert!(
            !result.uses_trash,
            "uses_trash must come from the pinned mode, not the global"
        );
    }

    #[test]
    fn clean_records_unsafe_paths_as_skipped_entries_and_continues() {
        // Ledger pin (PBI 70): an unsafe path (refused by the seam's scope
        // tripwire) is recorded as a skipped entry with a prose reason and
        // the run continues with the safe dirs. The pre-ledger loop printed
        // a bare warning and recorded nothing, so the skip channel and the
        // exit code could not see the refusal.
        let tmp = tempfile::TempDir::new().unwrap();
        let real = tmp.path().join("real_cache");
        std::fs::create_dir_all(&real).unwrap();
        std::fs::write(real.join("data.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = PathCleaner {
            display_name: "unsafe-pin",
            // /etc/hosts exists (so the caller's existence check passes it
            // through) and never passes is_safe_delete_target: the seam
            // bails fatally, and the ledger must record it, not abort.
            dirs: vec![std::path::PathBuf::from("/etc/hosts"), real.clone()],
            runner: Box::new(crate::test_helpers::MockRunner::new().with_success("chflags")),
            risk: Risk::Moderate,
            older_than_days: None,
            larger_than_mb: None,
        };
        // The ledger sums the size it was handed, measured before the
        // clean deletes the dir. The opts pin Permanent: the safe dir must
        // delete for real (the default Trash mode's backend is TCC-blocked
        // in this sandbox, which would skip the dir and blur the pin).
        let real_size = crate::format::dir_size(&real);
        let opts = crate::cleaner::CleanOptions {
            trash_mode: crate::trash::TrashMode::Permanent,
            ..Default::default()
        };
        let result = cleaner
            .clean(false, &crate::progress::DeepSuppressReporter, &opts)
            .unwrap();
        assert_eq!(result.skipped.len(), 1, "the unsafe path is recorded");
        assert_eq!(
            result.skipped[0].path,
            std::path::PathBuf::from("/etc/hosts")
        );
        assert!(
            matches!(&result.skipped[0].reason, SkipCause::Reason(reason) if reason.contains("refusing to delete unsafe path")),
            "the seam refusal rides as prose: {:?}",
            result.skipped[0].reason
        );
        assert!(!real.exists(), "the safe dir still deletes");
        assert_eq!(result.bytes_freed, real_size, "the safe dir is summed");
        assert_eq!(result.deleted_paths, vec![real]);
    }

    // ── CliCleaner trash_mode pin contract ──

    /// A CliCleaner whose CLI tool is missing: the fallback delete is the
    /// only deletion seam this cleaner owns, and it runs against `dir`.
    fn fallback_cli_cleaner(dir: &Path) -> CliCleaner {
        CliCleaner {
            display_name: "test-cli-pin",
            program: "definitely-not-installed-cli",
            args: &[],
            extra_commands: Vec::new(),
            detect_dir: Some(dir.to_path_buf()),
            runner: Box::new(crate::command::SystemCommandRunner),
            confirm_message: None,
            fallback_delete: true,
            notice: None,
            risk: Risk::Moderate,
        }
    }

    #[test]
    fn cli_clean_with_opts_pinned_permanent_deletes_for_real() {
        // The CleanOptions.trash_mode pin contract: a pinned Permanent must
        // delete for real and claim no Trash.
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = fallback_cli_cleaner(&dir);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Permanent,
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(
            !dir.exists(),
            "the pinned permanent mode must delete for real"
        );
        assert!(
            !result.uses_trash,
            "uses_trash must come from the pinned mode"
        );
    }

    #[test]
    fn cli_clean_with_opts_pinned_trash_claims_trash_only_when_deleted() {
        // A pinned Trash must resolve the fallback seam to trash.
        // PBI 43: a Trash claim may only accompany a real deletion — when the
        // Trash move lands, the freed result claims Trash; when it comes back
        // TCC-skipped (as in this sandbox for tempdir paths), the seam
        // classifies the failure as skippable, the skip is recorded, and the
        // not-deleted entry claims no Trash.
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = fallback_cli_cleaner(&dir);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Trash,
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        if result.bytes_freed > 0 {
            assert!(
                result.uses_trash,
                "a real deletion under the pinned Trash mode must claim Trash"
            );
        } else {
            assert!(
                !result.uses_trash,
                "a seam-skipped (not deleted) entry must never claim Trash"
            );
            assert_eq!(
                result.skipped.len(),
                1,
                "the seam-skip must be recorded as a SkippedEntry"
            );
        }
    }

    #[test]
    fn cli_clean_with_opts_default_carries_trash() {
        // The CleanOptions default carries the concrete Trash mode (the
        // historical default). PBI 43: a Trash claim may only accompany a
        // real deletion — when the Trash move lands, the freed result
        // claims Trash; when it comes back TCC-skipped (as in this sandbox
        // for tempdir paths), the seam classifies the failure as skippable,
        // the skip is recorded, and the not-deleted entry claims no Trash.
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = fallback_cli_cleaner(&dir);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::default(),
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        if result.bytes_freed > 0 {
            assert!(
                result.uses_trash,
                "a real deletion under the default Trash mode must claim Trash"
            );
        } else {
            assert!(
                !result.uses_trash,
                "a seam-skipped (not deleted) entry must never claim Trash"
            );
            assert_eq!(
                result.skipped.len(),
                1,
                "the seam-skip must be recorded as a SkippedEntry"
            );
        }
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

    // ── PBI 44: CliCleaner extra commands (colima prune + fstrim) ──

    /// Succeeds on the first call, fails on the second (VM stopped).
    struct OkThenErr(std::sync::atomic::AtomicUsize);
    impl CommandRunner for OkThenErr {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            use std::os::unix::process::ExitStatusExt;
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: vec![],
                    stderr: vec![],
                })
            } else {
                anyhow::bail!("failed to spawn `colima`: VM is not running")
            }
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    /// Succeeds and removes a canary file inside the detect dir during the
    /// fstrim run only, and only after the prune run has been seen, so the
    /// physical-size diff reports the follow-up's effect and the test fails
    /// if the extras never ran, ran before the main command, or were
    /// measured before the extras.
    struct ShrinkOnRun(PathBuf, std::sync::atomic::AtomicBool);
    impl CommandRunner for ShrinkOnRun {
        fn run(&self, _: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            use std::os::unix::process::ExitStatusExt;
            if args.first() == Some(&"prune") {
                self.1.store(true, Ordering::Relaxed);
            }
            if args.first() == Some(&"ssh") && self.1.load(Ordering::Relaxed) {
                let _ = fs::remove_file(self.0.join("keep.txt"));
            }
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn clean_runs_extra_command_and_its_effect_counts_in_freed_bytes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep.txt"), b"x").unwrap();
        let cleaner = CliCleaner::with_detect_dir(
            "test",
            "colima",
            &["prune", "--all", "--force"],
            dir.clone(),
            Box::new(ShrinkOnRun(
                dir.clone(),
                std::sync::atomic::AtomicBool::new(false),
            )),
        )
        .with_extra_commands(vec![(
            "colima",
            &["ssh", "--", "sudo", "fstrim", "-av"],
            Some("start the VM to trim"),
        )]);
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert!(
            result.bytes_freed > 0,
            "the extra command's effect must count in the freed bytes"
        );
    }

    #[test]
    fn clean_extra_command_failure_is_non_fatal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        fs::create_dir_all(&dir).unwrap();
        let cleaner = CliCleaner::with_detect_dir(
            "test",
            "colima",
            &["prune", "--all", "--force"],
            dir,
            Box::new(OkThenErr(std::sync::atomic::AtomicUsize::new(0))),
        )
        .with_extra_commands(vec![(
            "colima",
            &["ssh", "--", "sudo", "fstrim", "-av"],
            Some("start the VM to trim"),
        )]);
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &crate::cleaner::CleanOptions::default());
        assert!(
            result.is_ok(),
            "a failed extra command must not fail the clean"
        );
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

    // ── primary_target_display tests ────────────────────────────────────────

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
    fn colima_confirm_message_describes_the_actual_action() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let msg = cleaner
            .confirm_message
            .expect("colima has a confirmation prompt");
        assert!(msg.contains("fstrim"), "{msg}");
        assert!(
            !msg.contains("delete ALL stopped"),
            "the prompt must not describe colima delete: {msg}"
        );
        assert!(msg.contains("~/.colima"), "{msg}");
        assert!(msg.contains("deleted instead"), "{msg}");
        assert!(
            !msg.contains("Continue?"),
            "the explanation must not carry the prompt text: dialoguer clears one line, so a wrapped Confirm prompt doubles the message: {msg}"
        );
    }

    #[test]
    fn colima_fallback_delete_refuses_non_tty_stdin() {
        // PBI 69: the fallback delete removes the whole ~/.colima tree, VM
        // disk images included. colima's full deletion is TTY-only: a
        // non-TTY stdin cannot show the confirm, so the clean must refuse
        // with the standard remedy and delete nothing — the same non-TTY
        // floor as ios-backup / apfs-snapshot.
        if prompt::is_interactive() {
            // On a TTY the confirm prompt would block a test run; the TTY
            // behavior is pinned by the E2E suite instead.
            return;
        }
        let tmp = tempfile::TempDir::new().unwrap();
        let disk = tmp.path().join(".colima/_lima/colima/disk");
        fs::create_dir_all(disk.parent().unwrap()).unwrap();
        fs::write(&disk, b"vm disk bytes").unwrap();
        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let result = cleaner
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions::default(),
            )
            .expect("the refusal is a skip, not an error");
        assert_eq!(
            result.bytes_freed, 0,
            "the refusal must report zero freed bytes"
        );
        assert!(disk.exists(), "the VM disk must survive the refusal");
    }

    #[test]
    fn confirm_prompt_ownership_lives_in_the_port() {
        // The confirm branch's prompt contracts (suspension, the one-line
        // Confirm prompt) are owned by the prompt port (PBI 55): the
        // cleaner calls the port instead of running dialoguer itself, so
        // the dialoguer-redraw fix cannot resurface per call site. The
        // suspension contract is behaviorally tested in the port's own
        // tests (prompt::tests::tty_prompt_suspends_live_output_*); this
        // pins the seam shape.
        let src = include_str!("generic.rs");
        let start = src
            .find("fn clean_with_mode(")
            .expect("clean_with_mode source present");
        let end = src[start..]
            .find("impl Cleaner for PathCleaner")
            .map(|i| start + i)
            .unwrap_or(src.len());
        let body = &src[start..end];
        assert!(
            body.contains("TtyPrompt.confirm"),
            "the confirm prompt must run through the prompt port"
        );
        assert!(
            !body.contains("dialoguer::"),
            "the cleaner must not call dialoguer directly (the port owns the prompt)"
        );
        assert!(
            !body.contains("with_prompt(*msg)"),
            "the Confirm prompt must stay one short line — the long explanation prints as normal output above it, because dialoguer clears one line and a wrapped prompt doubles the message"
        );
    }

    #[test]
    fn non_vm_cleaner_has_no_advisory() {
        let cleaner = CliCleaner::npm(Box::new(MissingToolRunner));
        assert!(cleaner.advisory().is_none());
    }

    // ── PBI 43: PathCleaner skip visibility (SkippedEntry + uses_trash truth) ──

    #[test]
    fn clean_filtered_records_seam_skips_and_claims_no_trash_for_them() {
        // PBI 43: a permission-denied deletion is seam-classified skippable
        // and must surface as a SkippedEntry instead of a silent Ok(0). The
        // refusal comes from the test-only backend override: the platform
        // backends (Finder via osascript, trashItemAtURL via trashd) move
        // entries out of read-only parents on modern macOS, so a chmod alone
        // no longer forces the deterministic failure. The opts carry the
        // Trash mode, so the assertion follows the seam truth. The override
        // guard holds TEST_LOCK internally: no separate hold here (PBI 71).
        let _backend = crate::trash::TrashBackendOverrideGuard::set();
        let tmp = tempfile::TempDir::new().unwrap();
        let parent = tmp.path().join("locked");
        let dir = parent.join("cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("data.bin"), vec![0u8; 1024]).unwrap();
        let cleaner = PathCleaner::delete_dirs("test-eacces", dir.clone());
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: crate::trash::TrashMode::Trash,
        };
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter, &opts).unwrap();
        assert!(dir.exists(), "a permission-denied dir must survive");
        assert_eq!(
            result.skipped.len(),
            1,
            "the seam-classified failure must be recorded as a SkippedEntry"
        );
        assert_eq!(result.skipped[0].path, dir, "the skip must carry the path");
        // Typed classification (PBI 59): the seam's skip carries the variant,
        // not prose, so a reworded message cannot flip the skip.
        assert!(
            matches!(
                &result.skipped[0].reason,
                crate::cleaner::SkipCause::Delete(crate::cleaner::DeleteError::TrashRefused(_))
            ),
            "the skip must carry the typed TrashRefused classification: {:?}",
            result.skipped[0].reason
        );
        assert!(
            !result.uses_trash,
            "a run where nothing was deleted must not claim Trash"
        );
        assert_eq!(result.bytes_freed, 0, "nothing was deleted");
    }

    #[test]
    fn clean_filtered_all_nonexistent_targets_claim_no_trash() {
        // PBI 43: a zero-deletion run (all targets nonexistent) must report
        // uses_trash == false instead of the unconditionally resolved mode.
        // The global is pinned to trash so the old `mode.resolve()` reporting
        // would claim Trash for a run that deleted nothing.
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = PathCleaner::node_gyp(tmp.path(), Box::new(MissingToolRunner));
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner
            .clean(false, &reporter, &crate::cleaner::CleanOptions::default())
            .unwrap();
        assert!(
            !result.uses_trash,
            "a run where nothing was deleted must not claim Trash"
        );
        assert_eq!(result.bytes_freed, 0, "nothing exists to delete");
        assert!(
            result.skipped.is_empty(),
            "nonexistent targets record no skip"
        );
    }
}
