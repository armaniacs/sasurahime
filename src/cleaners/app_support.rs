use crate::cleaner::{
    CleanOptions, CleanResult, Cleaner, ScanResult, ScanStatus, SkipCause, SkippedEntry,
};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
use crate::hint::apps::KNOWN_ENTRIES;
use crate::hint::{is_running, quit_app_and_wait, relaunch_app, BaseDir, HintEntry};
use crate::progress::ProgressReporter;
use crate::prompt::PromptPort;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

pub const DEFAULT_MIN_APP_BYTES: u64 = 64 * 1024 * 1024;

/// One application whose cache subdirectories may be removed.
/// Only paths listed in `subpaths` are ever deletable; everything else under
/// `root` (settings, logins, IndexedDB, Local Storage, workspaces) is user data.
#[derive(Debug)]
pub struct AppSupportTarget {
    pub hint_name: &'static str,
    pub root: &'static str,
    pub subpaths: &'static [&'static str],
}

pub const TARGETS: &[AppSupportTarget] = &[
    AppSupportTarget {
        hint_name: "Slack",
        root: "Slack",
        subpaths: &[
            "Cache",
            "Code Cache",
            "GPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
            "logs",
        ],
    },
    AppSupportTarget {
        hint_name: "Claude (desktop)",
        root: "Claude",
        subpaths: &[
            "Cache",
            "Code Cache",
            "GPUCache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
        ],
    },
    AppSupportTarget {
        hint_name: "Obsidian",
        root: "obsidian",
        subpaths: &[
            "Cache",
            "Code Cache",
            "GPUCache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
        ],
    },
    AppSupportTarget {
        hint_name: "VSCode caches",
        root: "Code",
        subpaths: &[
            "Cache",
            "CachedData",
            "CachedExtensionVSIXs",
            "CachedProfilesData",
            "Code Cache",
            "GPUCache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
            "logs",
        ],
    },
    AppSupportTarget {
        hint_name: "Google Chrome",
        root: "Google/Chrome",
        subpaths: &[
            "ShaderCache",
            "GrShaderCache",
            "GraphiteDawnCache",
            "Default/Cache",
            "Default/Cache_Data",
            "Default/Code Cache",
            "Default/GPUCache",
            "Default/DawnGraphiteCache",
            "Default/DawnWebGPUCache",
            "Default/Service Worker/CacheStorage",
            "Default/Service Worker/ScriptCache",
        ],
    },
];

/// Looks up the hint entry that carries process name, quit/relaunch app names
/// and the size threshold for this target.
pub fn hint_entry(target: &AppSupportTarget) -> Option<&'static HintEntry> {
    KNOWN_ENTRIES
        .iter()
        .find(|e| e.base_dir == BaseDir::AppSupport && e.display_name == target.hint_name)
}

#[derive(Debug, PartialEq, Eq)]
pub enum GuardError {
    NotAllowed,
    NotFound,
    Symlink,
    NotDirectory,
    Unreadable,
    OutsideBase,
}

impl std::fmt::Display for GuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GuardError::NotAllowed => "path is not in the allowlist",
            GuardError::NotFound => "path does not exist",
            GuardError::Symlink => "path contains a symbolic link",
            GuardError::NotDirectory => "path is not a directory",
            GuardError::Unreadable => "path is not readable",
            GuardError::OutsideBase => "path resolves outside Application Support",
        };
        f.write_str(s)
    }
}

/// The only way to obtain a deletable path. `sub` must equal one of
/// `target.subpaths` verbatim, and no component below `base` may be a symlink.
pub fn resolve_allowed(
    base: &Path,
    target: &AppSupportTarget,
    sub: &str,
) -> Result<PathBuf, GuardError> {
    if !target.subpaths.contains(&sub) {
        return Err(GuardError::NotAllowed);
    }
    let rel = Path::new(target.root).join(sub);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(GuardError::NotAllowed);
    }
    let mut cur = base.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        let meta = match fs::symlink_metadata(&cur) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(GuardError::NotFound),
            Err(_) => return Err(GuardError::Unreadable),
        };
        if meta.file_type().is_symlink() {
            return Err(GuardError::Symlink);
        }
        if !meta.is_dir() {
            return Err(GuardError::NotDirectory);
        }
    }
    let canonical_base = base.canonicalize().map_err(|_| GuardError::Unreadable)?;
    let canonical = cur.canonicalize().map_err(|_| GuardError::Unreadable)?;
    if !canonical.starts_with(&canonical_base) {
        return Err(GuardError::OutsideBase);
    }
    Ok(cur)
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub target: &'static AppSupportTarget,
    pub sub: &'static str,
    /// For display only. Deletion re-resolves via `resolve_allowed`.
    pub path: PathBuf,
    pub size: u64,
}

pub fn scan_candidates(base: &Path, min_app_bytes: Option<u64>) -> Vec<Candidate> {
    let mut all = Vec::new();
    for target in TARGETS {
        let mut found = Vec::new();
        for sub in target.subpaths {
            let Ok(path) = resolve_allowed(base, target, sub) else {
                continue;
            };
            let size = dir_size(&path);
            if size == 0 {
                continue;
            }
            found.push(Candidate {
                target,
                sub,
                path,
                size,
            });
        }
        let total: u64 = found.iter().map(|c| c.size).sum();
        let threshold = min_app_bytes
            .or_else(|| hint_entry(target).map(|e| e.threshold_bytes))
            .unwrap_or(DEFAULT_MIN_APP_BYTES);
        if total > threshold {
            all.extend(found);
        }
    }
    all.sort_by_key(|c| std::cmp::Reverse(c.size));
    all
}

const NAME: &str = "app-support";

pub fn format_candidate_label(c: &Candidate, running: bool) -> String {
    format!(
        "{:<18} {:<44} {:>10}{}",
        c.target.hint_name,
        format!("{}/{}", c.target.root, c.sub),
        format_bytes(c.size),
        if running { "  [running]" } else { "" }
    )
}

pub fn format_leftover_line(l: &Leftover) -> String {
    format!("  {}  {}", l.path.display(), format_bytes(l.size))
}

/// Up to this many leftovers are opened in Finder without asking. Selecting
/// is already the user's decision, but a long selection would flood the
/// screen with Finder windows, so more than this asks once first.
pub const OPEN_CONFIRM_THRESHOLD: usize = 5;

pub fn open_needs_confirmation(selected: usize) -> bool {
    selected > OPEN_CONFIRM_THRESHOLD
}

enum AppState {
    Ready,
    Quit,
    Blocked(String),
}

pub struct AppSupportCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    min_app_bytes: Option<u64>,
    app_roots: Vec<PathBuf>,
}

/// The real system roots (`/Applications`, `/System/Applications`, ...) cost
/// minutes of cold metadata I/O on a fresh CI runner, so there the default
/// collection limits itself to the HOME-derived root, where fixtures put
/// their apps. An injected root set (tests, config) is never trimmed.
fn default_app_roots(home: &Path) -> Vec<PathBuf> {
    if crate::config::is_ci() {
        vec![home.join("Applications")]
    } else {
        vec![
            PathBuf::from("/Applications"),
            PathBuf::from("/Applications/Utilities"),
            PathBuf::from("/System/Applications"),
            PathBuf::from("/System/Applications/Utilities"),
            home.join("Applications"),
        ]
    }
}

impl AppSupportCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            min_app_bytes: None,
            app_roots: default_app_roots(home),
        }
    }

    #[cfg(test)]
    fn with_min_app_bytes(mut self, n: u64) -> Self {
        self.min_app_bytes = Some(n);
        self
    }

    #[cfg(test)]
    fn with_app_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.app_roots = roots;
        self
    }

    fn leftovers(&self) -> Vec<Leftover> {
        let installed = collect_installed_apps(&self.app_roots, self.runner.as_ref());
        find_leftovers(
            &self.base(),
            &installed,
            SystemTime::now(),
            MIN_LEFTOVER_BYTES,
            RECENT_DAYS,
        )
    }

    /// Display only. Selected entries are opened in Finder; nothing is deleted.
    fn show_leftovers(&self, reporter: &dyn ProgressReporter, dry_run: bool) -> Result<()> {
        if !self.base().exists() {
            return Ok(());
        }
        if !dry_run && !crate::prompt::is_interactive() {
            return Ok(());
        }
        let left = self.leftovers();
        if left.is_empty() {
            return Ok(());
        }
        println!(
            "[{NAME}] possible leftovers of uninstalled apps (display only, nothing is deleted):"
        );
        for l in &left {
            println!("{}", format_leftover_line(l));
        }
        if dry_run {
            return Ok(());
        }
        let items: Vec<String> = left.iter().map(format_leftover_line).collect();
        let defaults = vec![false; items.len()];
        let picked = crate::prompt::TtyPrompt.multi_select(
            reporter,
            Some("Select leftovers to reveal in Finder (space to toggle, enter to confirm)"),
            &items,
            &defaults,
        )?;
        if picked.is_empty() {
            return Ok(());
        }
        if open_needs_confirmation(picked.len()) {
            let ok = crate::prompt::TtyPrompt.confirm(
                reporter,
                &format!("This opens {} Finder windows. Continue?", picked.len()),
            )?;
            if !ok {
                println!("[{NAME}] nothing opened");
                return Ok(());
            }
        }
        let paths: Vec<PathBuf> = picked.into_iter().map(|i| left[i].path.clone()).collect();
        self.reveal_paths(&paths);
        Ok(())
    }

    /// Goes through the injected runner (not `explorer::open_in_finder`) so
    /// that tests can count the `open` calls without launching Finder.
    fn reveal_paths(&self, paths: &[PathBuf]) {
        for p in paths {
            if let Err(e) = crate::command::open_path(self.runner.as_ref(), p) {
                eprintln!("[{NAME}] error opening Finder: {e}");
            }
        }
    }

    fn base(&self) -> PathBuf {
        self.home.join("Library/Application Support")
    }

    fn empty_result() -> CleanResult {
        CleanResult::empty(NAME)
    }

    fn running_apps(&self, cands: &[Candidate]) -> HashSet<&'static str> {
        let mut running = HashSet::new();
        for c in cands {
            if running.contains(c.target.hint_name) {
                continue;
            }
            let up = hint_entry(c.target)
                .and_then(|e| e.process_name)
                .map(|n| is_running(n, self.runner.as_ref()))
                .unwrap_or(false);
            if up {
                running.insert(c.target.hint_name);
            }
        }
        running
    }

    fn print_dry_run(&self, cands: &[Candidate]) {
        if cands.is_empty() {
            println!("[{NAME}] nothing to clean");
            return;
        }
        println!(
            "[{NAME}] dry-run: {} cleanable cache directories",
            cands.len()
        );
        for c in cands {
            println!(
                "  would remove: {}  ({}, {})",
                c.path.display(),
                c.target.hint_name,
                format_bytes(c.size)
            );
        }
    }

    fn interactive_select(
        &self,
        reporter: &dyn ProgressReporter,
        cands: &[Candidate],
        running: &HashSet<&'static str>,
    ) -> Result<Vec<Candidate>> {
        let items: Vec<String> = cands
            .iter()
            .map(|c| format_candidate_label(c, running.contains(c.target.hint_name)))
            .collect();
        let defaults = vec![false; items.len()];
        println!("\nCache directories under ~/Library/Application Support/ (none selected by default):\n");
        let picked = crate::prompt::TtyPrompt.multi_select(reporter, None, &items, &defaults)?;
        Ok(picked.into_iter().map(|i| cands[i].clone()).collect())
    }

    fn confirm_quits(
        &self,
        reporter: &dyn ProgressReporter,
        selected: &[Candidate],
        running: &HashSet<&'static str>,
    ) -> Result<HashSet<&'static str>> {
        let mut approved = HashSet::new();
        for c in selected {
            let name = c.target.hint_name;
            if !running.contains(name) || approved.contains(name) {
                continue;
            }
            let ok = crate::prompt::TtyPrompt.confirm(
                reporter,
                &format!("{name} is running. Quit it now? (its own save dialogs apply)"),
            )?;
            if ok {
                approved.insert(name);
            }
        }
        Ok(approved)
    }

    fn prepare_app(
        &self,
        target: &'static AppSupportTarget,
        approved: &HashSet<&'static str>,
    ) -> AppState {
        let Some(entry) = hint_entry(target) else {
            return AppState::Blocked("no hint entry".to_string());
        };
        let up = entry
            .process_name
            .map(|n| is_running(n, self.runner.as_ref()))
            .unwrap_or(false);
        if !up {
            return AppState::Ready;
        }
        if !approved.contains(target.hint_name) {
            return AppState::Blocked(format!("{} is running", target.hint_name));
        }
        match quit_app_and_wait(entry, self.runner.as_ref()) {
            Ok(()) => AppState::Quit,
            Err(e) => AppState::Blocked(format!("{e}")),
        }
    }

    /// Deletes the selected candidates. The paths come from `resolve_allowed`,
    /// never from `Candidate::path`, so a forged or stale candidate cannot
    /// reach outside the allowlist. `mode` is the deletion mode for the seam:
    /// `Global` reads the process-global mode; `Trash` / `Permanent` pin it
    /// so the seam outcome and the deletion agree.
    pub(crate) fn apply_selection(
        &self,
        selected: &[Candidate],
        quit_approved: &HashSet<&'static str>,
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        let base = self.base();
        let mut states: Vec<(&'static str, AppState)> = Vec::new();
        let mut skipped = Vec::new();
        let mut items: Vec<(std::path::PathBuf, u64)> = Vec::new();

        // Prepare the apps and resolve the allowlisted paths BEFORE the
        // delete loop: a blocked app or a path outside the allowlist is a
        // skip, not a deletion. The deletion itself (progress, per-entry
        // skip classification, freed sums) lives in the shared
        // `crate::trash::delete_entries`.
        for cand in selected {
            let name = cand.target.hint_name;
            let idx = match states.iter().position(|(n, _)| *n == name) {
                Some(i) => i,
                None => {
                    let st = self.prepare_app(cand.target, quit_approved);
                    states.push((name, st));
                    states.len() - 1
                }
            };
            if let AppState::Blocked(reason) = &states[idx].1 {
                skipped.push(SkippedEntry {
                    path: cand.path.clone(),
                    reason: SkipCause::Reason(reason.clone()),
                });
                continue;
            }
            let path = match resolve_allowed(&base, cand.target, cand.sub) {
                Ok(p) => p,
                Err(e) => {
                    skipped.push(SkippedEntry {
                        path: cand.path.clone(),
                        reason: SkipCause::Reason(e.to_string()),
                    });
                    continue;
                }
            };
            items.push((path, cand.size));
        }

        let mut result =
            crate::trash::delete_entries(NAME, self.runner.as_ref(), reporter, mode, &items);

        for (name, state) in &states {
            if !matches!(state, AppState::Quit) {
                continue;
            }
            let entry = TARGETS
                .iter()
                .find(|t| t.hint_name == *name)
                .and_then(hint_entry);
            if let Some(entry) = entry {
                if let Err(e) = relaunch_app(entry, self.runner.as_ref()) {
                    eprintln!("[{NAME}] failed to relaunch {name}: {e}");
                }
            }
        }

        // Merge the pre-delete skips (blocked apps, unresolvable paths) into
        // the shared loop's result.
        result.skipped.extend(skipped);
        Ok(result)
    }

    /// The shared clean body. `mode` is the deletion mode threaded into
    /// `clean_cache_dirs`: `Global` reads the process-global mode; `Trash` /
    /// `Permanent` pin it so the seam outcome and the deletion agree.
    fn clean_with_mode(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        let result = self.clean_cache_dirs(dry_run, reporter, mode)?;
        self.show_leftovers(reporter, dry_run)?;
        Ok(result)
    }

    fn clean_cache_dirs(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        mode: crate::trash::TrashMode,
    ) -> Result<CleanResult> {
        let base = self.base();
        if !base.exists() {
            return Ok(Self::empty_result());
        }
        let cands = scan_candidates(&base, self.min_app_bytes);
        if dry_run {
            self.print_dry_run(&cands);
            return Ok(Self::empty_result());
        }
        if cands.is_empty() {
            println!("[{NAME}] nothing to clean");
            return Ok(Self::empty_result());
        }
        if !crate::prompt::gate_interactive(NAME) {
            return Ok(Self::empty_result());
        }
        let running = self.running_apps(&cands);
        let selected = self.interactive_select(reporter, &cands, &running)?;
        if selected.is_empty() {
            println!("[{NAME}] nothing selected");
            return Ok(Self::empty_result());
        }
        let approved = self.confirm_quits(reporter, &selected, &running)?;
        let result = self.apply_selection(&selected, &approved, reporter, mode)?;
        println!("[{NAME}] total freed: {}", format_bytes(result.bytes_freed));
        Ok(result)
    }
}

impl Cleaner for AppSupportCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Manual
    }

    fn name(&self) -> &'static str {
        NAME
    }

    fn detect(&self) -> ScanResult {
        let base = self.base();
        if !base.exists() {
            return ScanResult::new(NAME, ScanStatus::NotFound);
        }
        let total: u64 = scan_candidates(&base, self.min_app_bytes)
            .iter()
            .map(|c| c.size)
            .sum();
        let mut r = ScanResult::new(
            NAME,
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(base.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        // Plain clean keeps the process-global trash mode (compat path).
        self.clean_with_mode(dry_run, reporter, crate::trash::TrashMode::Global)
    }

    /// Cleans with `opts`, pinning `opts.trash_mode` when set so `uses_trash`
    /// and the actual deletion can never disagree (the `CleanOptions`
    /// contract). `None` keeps the process-global mode (compat).
    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.clean_with_mode(
            dry_run,
            reporter,
            opts.trash_mode.unwrap_or(crate::trash::TrashMode::Global),
        )
    }

    // Deleting app caches may require quitting running apps, so it is never
    // done unattended; `--yes` skips this cleaner even on a terminal.
    fn auto_clean_allowed(&self) -> bool {
        false
    }
}

// ── Stage 2: leftovers of uninstalled apps (display only) ──

const EXCLUDED_EXACT: &[&str] = &[
    "AddressBook",
    "CloudDocs",
    "MobileSync",
    "CallHistoryDB",
    "CallHistoryTransactions",
    "CrashReporter",
    "Dock",
    "FileProvider",
    "Knowledge",
    "SyncServices",
    "iCloud",
    "DifferentialPrivacy",
    "AskPermission",
    "Animoji",
    "Siri",
    "App Store",
    "DiskImages",
    "Spotlight",
    "Quick Look",
    "FaceTime",
    "Notes",
    "networkserviceproxy",
    "icdd",
    "Homebrew",
    "pypoetry",
    "rustup",
];

const EXCLUDED_PREFIXES: &[&str] = &["com.apple.", "Apple"];

/// Folders shared by several apps; the name alone cannot tell if one is installed.
const VENDOR_PARENT_DIRS: &[&str] = &[
    "Google",
    "Microsoft",
    "Adobe",
    "JetBrains",
    "BraveSoftware",
    "Mozilla",
    "Autodesk",
    "Oracle",
    "Blackmagic Design",
];

pub fn normalize_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn is_excluded_dir_name(name: &str) -> bool {
    if name.starts_with('.') {
        return true;
    }
    let lower = name.to_lowercase();
    if EXCLUDED_EXACT
        .iter()
        .chain(VENDOR_PARENT_DIRS.iter())
        .any(|e| e.to_lowercase() == lower)
    {
        return true;
    }
    if EXCLUDED_PREFIXES
        .iter()
        .any(|p| lower.starts_with(&p.to_lowercase()))
    {
        return true;
    }
    // Known apps are handled by the allowlist, not by name matching.
    TARGETS.iter().any(|t| {
        t.root
            .split('/')
            .next()
            .is_some_and(|first| first.to_lowercase() == lower)
    })
}

pub struct InstalledApps {
    names: HashSet<String>,
    bundle_ids: HashSet<String>,
    bundle_tails: HashSet<String>,
}

impl InstalledApps {
    pub fn from_parts(
        names: impl IntoIterator<Item = String>,
        bundle_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        let names = names
            .into_iter()
            .map(|n| normalize_name(&n))
            .filter(|n| !n.is_empty())
            .collect();
        let bundle_ids: HashSet<String> =
            bundle_ids.into_iter().map(|b| b.to_lowercase()).collect();
        let bundle_tails = bundle_ids
            .iter()
            .filter_map(|b| b.rsplit('.').next())
            .map(normalize_name)
            .filter(|t| t.len() >= 3)
            .collect();
        Self {
            names,
            bundle_ids,
            bundle_tails,
        }
    }
}

pub fn matches_installed(dir_name: &str, installed: &InstalledApps) -> bool {
    let n = normalize_name(dir_name);
    if n.is_empty() {
        return true;
    }
    if installed.names.contains(&n)
        || installed.bundle_ids.contains(&dir_name.to_lowercase())
        || (n.len() >= 3 && installed.bundle_tails.contains(&n))
    {
        return true;
    }
    if dir_name.matches('.').count() >= 2 {
        if let Some(last) = dir_name.rsplit('.').next() {
            let l = normalize_name(last);
            if !l.is_empty()
                && (installed.names.contains(&l)
                    || (l.len() >= 3 && installed.bundle_tails.contains(&l)))
            {
                return true;
            }
        }
    }
    false
}

/// An `.app` bundle probe shared with the orphan-containers cleaner:
/// case-insensitive `.app` extension plus a directory check, so a plain file
/// named `Foo.app` is never treated as a bundle.
pub(crate) fn is_app_bundle(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("app"))
        && p.is_dir()
}

/// `.app` bundles directly under each root, plus one level of subfolders
/// (for example `/Applications/Setapp/*.app`). Symlinked folders are not
/// followed: an unreadable file type is treated as a symlink.
pub(crate) fn list_app_bundles(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(root) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if is_app_bundle(&path) {
                found.push(path);
                continue;
            }
            let is_real_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_real_dir {
                continue;
            }
            if let Ok(sub) = fs::read_dir(&path) {
                for s in sub.flatten() {
                    if is_app_bundle(&s.path()) {
                        found.push(s.path());
                    }
                }
            }
        }
    }
    found
}

/// Bundle file names still identify an app whose `Info.plist` cannot be read.
fn app_bundle_stems(roots: &[PathBuf]) -> Vec<String> {
    list_app_bundles(roots)
        .iter()
        .filter_map(|app| app.file_stem().and_then(|s| s.to_str()).map(str::to_string))
        .collect()
}

/// `Info.plist` values come only from the orphan-containers collector, which
/// hands out nothing but the bundle ID and the two app names (README promise).
pub fn collect_installed_apps(roots: &[PathBuf], runner: &dyn CommandRunner) -> InstalledApps {
    let mut names = app_bundle_stems(roots);
    let mut bundle_ids = Vec::new();
    for app in crate::cleaners::orphan_containers::collect_app_identities(roots, runner) {
        names.extend(app.name);
        names.extend(app.display_name);
        bundle_ids.push(app.bundle_id);
    }
    InstalledApps::from_parts(names, bundle_ids)
}

pub const MIN_LEFTOVER_BYTES: u64 = 1024 * 1024;
pub const RECENT_DAYS: u64 = 30;

#[derive(Debug, Clone)]
pub struct Leftover {
    pub path: PathBuf,
    pub size: u64,
}

fn newest_mtime(path: &Path) -> Option<SystemTime> {
    let mut newest = fs::symlink_metadata(path).ok()?.modified().ok()?;
    for e in fs::read_dir(path).ok()?.flatten() {
        if let Ok(m) = fs::symlink_metadata(e.path()).and_then(|m| m.modified()) {
            newest = newest.max(m);
        }
    }
    Some(newest)
}

pub fn find_leftovers(
    base: &Path,
    installed: &InstalledApps,
    now: SystemTime,
    min_bytes: u64,
    recent_days: u64,
) -> Vec<Leftover> {
    let Ok(rd) = fs::read_dir(base) else {
        return vec![];
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if is_excluded_dir_name(&name) {
            continue;
        }
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if matches_installed(&name, installed) {
            continue;
        }
        let path = e.path();
        let size = dir_size(&path);
        if size < min_bytes {
            continue;
        }
        let Some(newest) = newest_mtime(&path) else {
            continue;
        };
        match now.duration_since(newest) {
            Ok(age) if age.as_secs() / 86_400 >= recent_days => {}
            _ => continue,
        }
        out.push(Leftover { path, size });
    }
    out.sort_by_key(|l| std::cmp::Reverse(l.size));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::TrashModeGuard;
    use std::collections::HashSet;
    use std::path::Component;

    #[test]
    fn every_target_has_matching_hint_entry() {
        for t in TARGETS {
            let e = hint_entry(t).unwrap_or_else(|| panic!("no hint entry for {}", t.hint_name));
            assert!(
                e.process_name.is_some(),
                "{} needs process_name",
                t.hint_name
            );
            assert!(e.quit_app.is_some(), "{} needs quit_app", t.hint_name);
            assert!(e.threshold_bytes >= DEFAULT_MIN_APP_BYTES);
        }
    }

    #[test]
    fn every_app_support_hint_entry_has_a_target() {
        for e in KNOWN_ENTRIES
            .iter()
            .filter(|e| e.base_dir == BaseDir::AppSupport)
        {
            assert!(
                TARGETS.iter().any(|t| t.hint_name == e.display_name),
                "hint entry {} has no TARGETS row",
                e.display_name
            );
        }
    }

    #[test]
    fn targets_cover_every_hint_suffix() {
        for e in KNOWN_ENTRIES
            .iter()
            .filter(|e| e.base_dir == BaseDir::AppSupport)
        {
            let t = TARGETS
                .iter()
                .find(|t| t.hint_name == e.display_name)
                .unwrap();
            for s in e.path_suffixes {
                let rel = s
                    .strip_prefix(&format!("{}/", t.root))
                    .unwrap_or_else(|| panic!("{s} is not under root {}", t.root));
                assert!(t.subpaths.contains(&rel), "{s} missing from allowlist");
            }
        }
    }

    #[test]
    fn allowlist_paths_are_relative_normal_components() {
        for t in TARGETS {
            for p in std::iter::once(&t.root).chain(t.subpaths.iter()) {
                assert!(!p.is_empty());
                for c in Path::new(p).components() {
                    assert!(matches!(c, Component::Normal(_)), "bad path {p}");
                }
            }
        }
    }

    #[test]
    fn allowlist_has_no_duplicate_or_nested_paths() {
        for t in TARGETS {
            let mut seen = HashSet::new();
            for s in t.subpaths {
                assert!(seen.insert(*s), "duplicate {s}");
            }
            for a in t.subpaths {
                for b in t.subpaths {
                    if a != b {
                        assert!(
                            !Path::new(a).starts_with(Path::new(b)),
                            "{a} is nested under {b} in {}",
                            t.hint_name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn allowlist_never_contains_user_data_names() {
        const FORBIDDEN: &[&str] = &[
            "Local Storage",
            "IndexedDB",
            "Partitions",
            "Cookies",
            "Session Storage",
            "User",
            "Backups",
            "Workspaces",
            "Login Data",
            "Extensions",
            "Database",
        ];
        for t in TARGETS {
            for s in t.subpaths {
                assert_ne!(
                    *s, "Service Worker",
                    "whole Service Worker dir holds user data"
                );
                for c in Path::new(s).components() {
                    let c = c.as_os_str().to_string_lossy();
                    assert!(
                        !FORBIDDEN.contains(&c.as_ref()),
                        "forbidden component {c} in {s}"
                    );
                }
            }
        }
    }
    use std::fs;
    use tempfile::TempDir;

    fn target(name: &str) -> &'static AppSupportTarget {
        TARGETS.iter().find(|t| t.hint_name == name).unwrap()
    }

    fn base_of(tmp: &TempDir) -> PathBuf {
        let b = tmp.path().join("Library/Application Support");
        fs::create_dir_all(&b).unwrap();
        b
    }

    #[test]
    fn resolve_allows_listed_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        let p = resolve_allowed(&base, target("Slack"), "Cache").unwrap();
        assert_eq!(p, base.join("Slack/Cache"));
    }

    #[test]
    fn resolve_allows_nested_chrome_path() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Google/Chrome/Default/Service Worker/CacheStorage")).unwrap();
        let p = resolve_allowed(
            &base,
            target("Google Chrome"),
            "Default/Service Worker/CacheStorage",
        );
        assert!(p.is_ok());
    }

    #[test]
    fn resolve_rejects_unlisted_user_data_even_if_it_exists() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        for d in ["Local Storage", "IndexedDB", "Partitions", "Service Worker"] {
            fs::create_dir_all(base.join("Slack").join(d)).unwrap();
            assert_eq!(
                resolve_allowed(&base, target("Slack"), d),
                Err(GuardError::NotAllowed),
                "{d}"
            );
        }
    }

    #[test]
    fn resolve_rejects_path_traversal_and_absolute() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        fs::create_dir_all(base.join("Code/Cache")).unwrap();
        for sub in [
            "../Code/Cache",
            "Cache/..",
            "Cache/../Local Storage",
            "../../..",
            "/etc",
            "",
            ".",
            "./Cache",
            "Cache/",
        ] {
            assert_eq!(
                resolve_allowed(&base, target("Slack"), sub),
                Err(GuardError::NotAllowed),
                "sub = {sub:?}"
            );
        }
    }

    #[test]
    fn resolve_rejects_symlinked_root() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("outside");
        fs::create_dir_all(outside.join("Cache")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack")).unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::Symlink)
        );
    }

    #[test]
    fn resolve_rejects_symlinked_leaf() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("Documents");
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir_all(base.join("Slack")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack/Cache")).unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::Symlink)
        );
    }

    #[test]
    fn resolve_rejects_symlinked_middle_component() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("elsewhere");
        fs::create_dir_all(outside.join("CacheStorage")).unwrap();
        fs::create_dir_all(base.join("Slack")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack/Service Worker")).unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Service Worker/CacheStorage"),
            Err(GuardError::Symlink)
        );
    }

    #[test]
    fn resolve_rejects_file_where_dir_expected() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack")).unwrap();
        fs::write(base.join("Slack/Cache"), b"x").unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::NotDirectory)
        );
    }

    #[test]
    fn resolve_reports_not_found_for_missing() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::NotFound)
        );
        let missing_base = tmp.path().join("nope");
        assert_eq!(
            resolve_allowed(&missing_base, target("Slack"), "Cache"),
            Err(GuardError::NotFound)
        );
    }

    #[test]
    fn resolve_reports_unreadable_parent() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        fs::set_permissions(base.join("Slack"), fs::Permissions::from_mode(0o000)).unwrap();
        let r = resolve_allowed(&base, target("Slack"), "Cache");
        fs::set_permissions(base.join("Slack"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(r, Err(GuardError::Unreadable));
    }

    fn put(base: &Path, rel: &str, bytes: usize) {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![1u8; bytes]).unwrap();
    }

    #[test]
    fn scan_returns_only_allowlisted_dirs() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/Local Storage/b.bin", 10_000);
        put(&base, "Slack/IndexedDB/c.bin", 10_000);
        put(&base, "Slack/Partitions/d.bin", 10_000);
        let c = scan_candidates(&base, Some(0));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].path, base.join("Slack/Cache"));
        assert_eq!(c[0].sub, "Cache");
        assert!(c[0].size > 0);
    }

    #[test]
    fn scan_skips_app_below_threshold() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        assert!(scan_candidates(&base, Some(u64::MAX)).is_empty());
    }

    #[test]
    fn scan_uses_hint_threshold_by_default() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        assert!(scan_candidates(&base, None).is_empty());
    }

    #[test]
    fn scan_excludes_symlinked_cache_dir() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("Documents");
        put(&outside, "Cache/big.bin", 50_000);
        fs::create_dir_all(base.join("Slack")).unwrap();
        std::os::unix::fs::symlink(outside.join("Cache"), base.join("Slack/Cache")).unwrap();
        assert!(scan_candidates(&base, Some(0)).is_empty());
    }

    #[test]
    fn scan_skips_empty_dirs_and_missing_base() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        assert!(scan_candidates(&base, Some(0)).is_empty());
        assert!(scan_candidates(&tmp.path().join("nope"), Some(0)).is_empty());
    }

    #[test]
    fn scan_sorts_by_size_descending_across_apps() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 8_192);
        put(&base, "Code/CachedData/b.bin", 81_920);
        put(&base, "Code/logs/c.log", 4_096);
        let c = scan_candidates(&base, Some(0));
        let sizes: Vec<u64> = c.iter().map(|x| x.size).collect();
        let mut sorted = sizes.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(sizes, sorted);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn scan_threshold_is_per_app_total() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Code/Cache/a.bin", 40_000);
        put(&base, "Code/CachedData/b.bin", 40_000);
        put(&base, "Slack/Cache/c.bin", 40_000);
        let one = crate::format::dir_size(&base.join("Slack/Cache"));
        let c = scan_candidates(&base, Some(one + 1));
        assert!(c.iter().all(|x| x.target.hint_name == "VSCode caches"));
        assert_eq!(c.len(), 2);
    }

    use crate::cleaner::{Cleaner, ScanStatus};
    use crate::command::CommandRunner;
    use crate::progress::DeepSuppressReporter;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use std::sync::{Arc, Mutex};

    fn out(code: i32) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code),
            stdout: vec![],
            stderr: vec![],
        }
    }

    #[derive(Default)]
    struct Inner {
        running: Mutex<HashSet<String>>,
        calls: Mutex<Vec<String>>,
    }

    #[derive(Clone, Default)]
    struct Fake(Arc<Inner>);

    impl Fake {
        fn with_running(names: &[&str]) -> Self {
            let f = Fake::default();
            for n in names {
                f.0.running.lock().unwrap().insert(n.to_string());
            }
            f
        }
        fn calls(&self) -> Vec<String> {
            self.0.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<Output> {
            self.0
                .calls
                .lock()
                .unwrap()
                .push(format!("{program} {}", args.join(" ")));
            match program {
                "pgrep" => {
                    let n = args.last().copied().unwrap_or("");
                    let up = self.0.running.lock().unwrap().contains(n);
                    Ok(out(if up { 0 } else { 256 }))
                }
                "osascript" => {
                    self.0.running.lock().unwrap().clear();
                    Ok(out(0))
                }
                "open" | "chflags" => Ok(out(0)),
                _ => anyhow::bail!("unexpected command {program}"),
            }
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    fn cleaner_for(tmp: &TempDir, fake: &Fake) -> AppSupportCleaner {
        AppSupportCleaner::new(tmp.path(), Box::new(fake.clone()))
            .with_min_app_bytes(0)
            .with_app_roots(vec![])
    }

    #[test]
    fn detect_is_not_found_without_application_support() {
        let tmp = TempDir::new().unwrap();
        let c = cleaner_for(&tmp, &Fake::default());
        assert!(matches!(c.detect().status, ScanStatus::NotFound));
        assert_eq!(c.name(), "app-support");
    }

    #[test]
    fn detect_sums_only_allowlisted_dirs() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/Local Storage/b.bin", 99_999);
        let c = cleaner_for(&tmp, &Fake::default());
        let r = c.detect();
        assert_eq!(r.bytes_for_test(), dir_size(&base.join("Slack/Cache")));
    }

    #[test]
    fn detect_is_clean_when_only_user_data_exists() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Local Storage/b.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        assert!(matches!(c.detect().status, ScanStatus::Clean));
    }

    #[test]
    fn dry_run_touches_nothing_and_runs_no_commands() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Slack"]);
        let c = cleaner_for(&tmp, &fake);
        let r = c.clean(true, &DeepSuppressReporter).unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert!(base.join("Slack/Cache/a.bin").exists());
        assert!(fake.calls().is_empty(), "calls: {:?}", fake.calls());
    }

    #[test]
    fn apply_deletes_only_selected_candidates() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/logs/l.log", 10_000);
        put(&base, "Slack/Local Storage/u.bin", 10_000);
        let fake = Fake::default();
        let c = cleaner_for(&tmp, &fake);
        let all = scan_candidates(&base, Some(0));
        let sel: Vec<Candidate> = all.into_iter().filter(|x| x.sub == "Cache").collect();
        let r = c
            .apply_selection(
                &sel,
                &HashSet::new(),
                &DeepSuppressReporter,
                crate::trash::TrashMode::Global,
            )
            .unwrap();
        assert!(!base.join("Slack/Cache").exists());
        assert!(base.join("Slack/logs/l.log").exists());
        assert!(base.join("Slack/Local Storage/u.bin").exists());
        assert_eq!(r.bytes_freed, sel[0].size);
        assert!(r.skipped.is_empty());
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn apply_blocks_running_app_without_approval() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Slack"]);
        let c = cleaner_for(&tmp, &fake);
        let sel = scan_candidates(&base, Some(0));
        let r = c
            .apply_selection(
                &sel,
                &HashSet::new(),
                &DeepSuppressReporter,
                crate::trash::TrashMode::Global,
            )
            .unwrap();
        assert!(base.join("Slack/Cache/a.bin").exists());
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert!(format!("{}", r.skipped[0].reason).contains("running"));
        assert!(!fake.calls().iter().any(|c| c.starts_with("osascript")));
        assert_eq!(r.exit_code(), 1);
    }

    /// Records the totals that `progress_init` receives, so a test can pin
    /// what the progress bar counts.
    #[derive(Default)]
    struct RecordingReporter {
        totals: Mutex<Vec<usize>>,
    }

    impl RecordingReporter {
        fn totals(&self) -> Vec<usize> {
            self.totals.lock().unwrap().clone()
        }
    }

    impl ProgressReporter for RecordingReporter {
        fn show_spinner(&self) -> bool {
            false
        }
        fn progress_init(&self, _label: &str, total: usize) {
            self.totals.lock().unwrap().push(total);
        }
        fn progress_tick(&self, _path: &Path, _current: usize, _size_bytes: u64) {}
        fn progress_finish(&self) {}
    }

    #[test]
    fn apply_progress_total_counts_deletable_items_only() {
        // Pin (PBI 68): the pre-delete filter removes blocked candidates
        // before the delete loop, so the progress total is items.len() —
        // the count of deletable paths only. v0.4.2 counted every selected
        // candidate (selected.len()), which left the bar short of its total
        // whenever an app was blocked. The bar counts what is about to be
        // deleted; blocked candidates surface as skipped entries instead.
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "obsidian/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Slack"]);
        let c = cleaner_for(&tmp, &fake);
        let selected = scan_candidates(&base, Some(0));
        assert_eq!(selected.len(), 2, "fixture: one blocked + one deletable");
        let rep = RecordingReporter::default();
        let r = c
            .apply_selection(
                &selected,
                &HashSet::new(),
                &rep,
                crate::trash::TrashMode::Global,
            )
            .unwrap();
        assert_eq!(r.skipped.len(), 1, "the running app must be blocked");
        assert!(base.join("Slack/Cache/a.bin").exists());
        assert!(!base.join("obsidian/Cache").exists());
        // The total is the deletable count (1), never all selected (2).
        assert_eq!(rep.totals(), vec![1]);
    }

    #[test]
    fn apply_quits_approved_running_app_then_deletes_and_relaunches() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "obsidian/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Obsidian"]);
        let c = cleaner_for(&tmp, &fake);
        let sel = scan_candidates(&base, Some(0));
        let approved: HashSet<&'static str> = HashSet::from(["Obsidian"]);
        let r = c
            .apply_selection(
                &sel,
                &approved,
                &DeepSuppressReporter,
                crate::trash::TrashMode::Global,
            )
            .unwrap();
        assert!(!base.join("obsidian/Cache").exists());
        assert!(r.bytes_freed > 0);
        let calls = fake.calls();
        let q = calls
            .iter()
            .position(|c| c == "osascript -e quit app \"Obsidian\"")
            .expect("quit call");
        let o = calls
            .iter()
            .position(|c| c == "open -a Obsidian")
            .expect("relaunch call");
        assert!(q < o);
    }

    #[test]
    fn apply_does_not_relaunch_app_that_was_not_running() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "obsidian/Cache/a.bin", 10_000);
        let fake = Fake::default();
        let c = cleaner_for(&tmp, &fake);
        let sel = scan_candidates(&base, Some(0));
        c.apply_selection(
            &sel,
            &HashSet::new(),
            &DeepSuppressReporter,
            crate::trash::TrashMode::Global,
        )
        .unwrap();
        assert!(!fake.calls().iter().any(|c| c.starts_with("open")));
    }

    #[test]
    fn apply_never_deletes_forged_unlisted_sub() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Local Storage/u.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        let forged = Candidate {
            target: target("Slack"),
            sub: "Local Storage",
            path: base.join("Slack/Local Storage"),
            size: 1,
        };
        let r = c
            .apply_selection(
                &[forged],
                &HashSet::new(),
                &DeepSuppressReporter,
                crate::trash::TrashMode::Global,
            )
            .unwrap();
        assert!(base.join("Slack/Local Storage/u.bin").exists());
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn apply_ignores_forged_path_field() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/Local Storage/u.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        let forged = Candidate {
            target: target("Slack"),
            sub: "Cache",
            path: base.join("Slack/Local Storage"),
            size: 5,
        };
        c.apply_selection(
            &[forged],
            &HashSet::new(),
            &DeepSuppressReporter,
            crate::trash::TrashMode::Global,
        )
        .unwrap();
        assert!(!base.join("Slack/Cache").exists());
        assert!(base.join("Slack/Local Storage/u.bin").exists());
    }

    #[test]
    fn apply_does_not_follow_symlinks_inside_cache() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("Documents");
        put(&outside, "keep.txt", 100);
        put(&outside, "dir/keep2.txt", 100);
        put(&base, "Slack/Cache/a.bin", 10_000);
        std::os::unix::fs::symlink(outside.join("keep.txt"), base.join("Slack/Cache/f")).unwrap();
        std::os::unix::fs::symlink(outside.join("dir"), base.join("Slack/Cache/d")).unwrap();
        let c = cleaner_for(&tmp, &Fake::default());
        let sel = scan_candidates(&base, Some(0));
        c.apply_selection(
            &sel,
            &HashSet::new(),
            &DeepSuppressReporter,
            crate::trash::TrashMode::Global,
        )
        .unwrap();
        assert!(!base.join("Slack/Cache").exists());
        assert!(outside.join("keep.txt").exists());
        assert!(outside.join("dir/keep2.txt").exists());
    }

    #[test]
    fn apply_rejects_dir_replaced_by_symlink_after_scan() {
        // Panic-safe pin: the guard restores the previous mode on drop.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        let sel = scan_candidates(&base, Some(0));
        let outside = tmp.path().join("Documents");
        put(&outside, "keep.txt", 100);
        fs::remove_dir_all(base.join("Slack/Cache")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack/Cache")).unwrap();
        let r = c
            .apply_selection(
                &sel,
                &HashSet::new(),
                &DeepSuppressReporter,
                crate::trash::TrashMode::Global,
            )
            .unwrap();
        assert!(outside.join("keep.txt").exists());
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
    }

    #[test]
    fn label_marks_running_apps() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let sel = scan_candidates(&base, Some(0));
        assert!(format_candidate_label(&sel[0], true).contains("[running]"));
        assert!(!format_candidate_label(&sel[0], false).contains("[running]"));
        assert!(format_candidate_label(&sel[0], false).contains("Slack/Cache"));
    }

    #[test]
    fn app_support_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!cleaner_for(&tmp, &Fake::default()).auto_clean_allowed());
    }

    #[test]
    fn orphan_containers_identity_collector_is_available() {
        use crate::cleaners::orphan_containers::{collect_app_identities, AppIdentity};
        let apps: Vec<AppIdentity> = collect_app_identities(&[], &Fake::default());
        assert!(apps.is_empty());
        // No `..`: this stage relies on Info.plist giving exactly these three values.
        let AppIdentity {
            bundle_id,
            name,
            display_name,
        } = AppIdentity {
            bundle_id: "com.example.app".to_string(),
            name: None,
            display_name: None,
        };
        assert_eq!(bundle_id, "com.example.app");
        assert!(name.is_none() && display_name.is_none());
    }

    fn installed(names: &[&str], ids: &[&str]) -> InstalledApps {
        InstalledApps::from_parts(
            names.iter().map(|s| s.to_string()),
            ids.iter().map(|s| s.to_string()),
        )
    }

    #[test]
    fn normalize_name_cases() {
        assert_eq!(normalize_name("Google Chrome"), "googlechrome");
        assert_eq!(normalize_name("zoom.us"), "zoomus");
        assert_eq!(normalize_name("1Password 7"), "1password7");
        assert_eq!(normalize_name(" - . "), "");
    }

    #[test]
    fn excluded_names() {
        for n in [
            ".DS_Store",
            "AddressBook",
            "addressbook",
            "CloudDocs",
            "MobileSync",
            "com.apple.sharedfilelist",
            "AppleIntelligence",
            "Google",
            "Microsoft",
            "JetBrains",
            "Slack",
            "obsidian",
            "Code",
            "Claude",
        ] {
            assert!(is_excluded_dir_name(n), "{n}");
        }
        for n in ["Spotify", "Zoom", "Figma", "com.foo.bar"] {
            assert!(!is_excluded_dir_name(n), "{n}");
        }
    }

    #[test]
    fn match_by_app_name() {
        let i = installed(&["Spotify", "Google Chrome"], &[]);
        assert!(matches_installed("Spotify", &i));
        assert!(matches_installed("spotify", &i));
        assert!(matches_installed("Google-Chrome", &i));
        assert!(!matches_installed("Figma", &i));
    }

    #[test]
    fn match_by_full_bundle_id_case_insensitive() {
        let i = installed(&[], &["com.foo.Bar"]);
        assert!(matches_installed("com.foo.bar", &i));
        assert!(matches_installed("COM.FOO.BAR", &i));
    }

    #[test]
    fn match_by_bundle_id_tail() {
        let i = installed(&[], &["com.tinyspeck.slackmacgap"]);
        assert!(matches_installed("slackmacgap", &i));
        assert!(!matches_installed("slack", &i));
    }

    #[test]
    fn match_bundle_id_shaped_folder_by_its_last_component() {
        let i = installed(&["Foo"], &[]);
        assert!(matches_installed("com.example.Foo", &i));
        let j = installed(&[], &["org.other.widget"]);
        assert!(matches_installed("com.example.widget", &j));
    }

    #[test]
    fn unmatchable_empty_normalized_name_is_treated_as_installed() {
        let i = installed(&[], &[]);
        assert!(matches_installed("---", &i));
    }

    #[test]
    fn short_bundle_tail_is_not_used() {
        let i = installed(&[], &["com.foo.ab"]);
        assert!(!matches_installed("ab", &i));
    }

    #[test]
    fn duplicate_app_names_still_match_once_or_more() {
        let i = installed(&["Foo", "Foo"], &["com.a.foo", "com.b.foo"]);
        assert!(matches_installed("Foo", &i));
    }

    fn write_plist(app: &Path, xml: &str) {
        let contents = app.join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(contents.join("Info.plist"), xml).unwrap();
    }

    #[test]
    fn list_app_bundles_finds_top_level_and_one_level_nested() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Foo Bar.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("Setapp/Baz.app")).unwrap();
        fs::create_dir_all(apps.join("Setapp/deep/Qux.app")).unwrap();
        fs::create_dir_all(apps.join("NotAnApp")).unwrap();
        let mut found: Vec<String> = list_app_bundles(&[apps.clone(), tmp.path().join("missing")])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        found.sort();
        assert_eq!(found, vec!["Baz.app", "Foo Bar.app"]);
    }

    #[test]
    fn collect_installed_apps_uses_file_stem_and_app_identity() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        write_plist(
            &apps.join("Visual Studio Code.app"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleIdentifier</key>\n\t<string>com.microsoft.VSCode</string>\n\t<key>CFBundleName</key>\n\t<string>Code</string>\n</dict>\n</plist>\n",
        );
        fs::create_dir_all(apps.join("Plain.app/Contents")).unwrap();
        let fake = Fake::default();
        let installed = collect_installed_apps(&[apps], &fake);
        let mut names: Vec<String> = installed.names.iter().cloned().collect();
        names.sort();
        assert_eq!(names, vec!["code", "plain", "visualstudiocode"]);
        assert!(installed.bundle_ids.contains("com.microsoft.vscode"));
        assert!(matches_installed("Code", &installed));
        assert!(matches_installed("vscode", &installed));
        assert!(fake.calls().is_empty(), "XML plists need no command");
    }

    #[test]
    fn collect_installed_apps_never_takes_other_plist_values() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        write_plist(
            &apps.join("Keeper.app"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>SasurahimeSecretScheme7f3a</string>
            </array>
        </dict>
    </array>
    <key>CFBundleExecutable</key>
    <string>SasurahimeSecretExec7f3a</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.keeper</string>
</dict>
</plist>
"#,
        );
        let installed = collect_installed_apps(&[apps], &Fake::default());
        let names: Vec<String> = installed.names.iter().cloned().collect();
        assert_eq!(names, vec!["keeper"]);
        let ids: Vec<String> = installed.bundle_ids.iter().cloned().collect();
        assert_eq!(ids, vec!["com.example.keeper"]);
        assert!(!matches_installed("SasurahimeSecretScheme7f3a", &installed));
        assert!(!matches_installed("SasurahimeSecretExec7f3a", &installed));
    }

    #[test]
    fn collect_installed_apps_falls_back_to_stem_when_plutil_fails() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        let app = apps.join("Binary Tool.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        let plist = app.join("Contents/Info.plist");
        fs::write(&plist, b"bplist00\x00\x01\x02").unwrap();
        let fake = Fake::default();
        let installed = collect_installed_apps(&[apps], &fake);
        let names: Vec<String> = installed.names.iter().cloned().collect();
        assert_eq!(names, vec!["binarytool"]);
        assert!(installed.bundle_ids.is_empty());
        assert_eq!(
            fake.calls(),
            vec![format!("plutil -convert xml1 -o - {}", plist.display())]
        );
    }

    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    fn age(path: &Path, days: u64) {
        let t = SystemTime::now() - Duration::from_secs(days * 86_400);
        set_file_mtime(path, FileTime::from_system_time(t)).unwrap();
    }

    /// Creates `<base>/<name>/data.bin` of `bytes` bytes, all aged `days` days.
    fn make_folder(base: &Path, name: &str, bytes: usize, days: u64) -> PathBuf {
        let d = base.join(name);
        fs::create_dir_all(&d).unwrap();
        let f = d.join("data.bin");
        fs::write(&f, vec![1u8; bytes]).unwrap();
        age(&f, days);
        age(&d, days);
        d
    }

    const MIB: usize = 1024 * 1024;

    #[test]
    fn leftovers_lists_uninstalled_old_folder() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let d = make_folder(&base, "GoneApp", 2 * MIB, 200);
        let l = find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].path, d);
        assert!(l[0].size >= 2 * MIB as u64);
    }

    #[test]
    fn leftovers_skips_installed_by_name_and_by_bundle_tail() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "Spotify", 2 * MIB, 200);
        make_folder(&base, "slackmacgap", 2 * MIB, 200);
        let i = installed(&["Spotify"], &["com.tinyspeck.slackmacgap"]);
        assert!(find_leftovers(&base, &i, SystemTime::now(), 1 << 20, 30).is_empty());
    }

    #[test]
    fn leftovers_skips_excluded_names() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        for n in [
            "AddressBook",
            "CloudDocs",
            "MobileSync",
            "com.apple.foo",
            "Google",
            "Slack",
            "Code",
            ".hidden",
        ] {
            make_folder(&base, n, 2 * MIB, 200);
        }
        assert!(
            find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30).is_empty()
        );
    }

    #[test]
    fn leftovers_skips_small_and_recent_and_future() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "Small", 100, 200);
        make_folder(&base, "Recent", 2 * MIB, 3);
        let fut = make_folder(&base, "Future", 2 * MIB, 200);
        let t = SystemTime::now() + Duration::from_secs(86_400);
        set_file_mtime(&fut, FileTime::from_system_time(t)).unwrap();
        assert!(
            find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30).is_empty()
        );
    }

    #[test]
    fn leftovers_skips_files_and_symlinks() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let real = make_folder(&tmp.path().join("elsewhere"), "Real", 2 * MIB, 200);
        std::os::unix::fs::symlink(&real, base.join("LinkedApp")).unwrap();
        fs::write(base.join("loose.bin"), vec![1u8; 2 * MIB]).unwrap();
        assert!(
            find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30).is_empty()
        );
    }

    #[test]
    fn leftovers_sorted_by_size_descending_and_missing_base_is_empty() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "Small", 2 * MIB, 200);
        make_folder(&base, "Big", 4 * MIB, 200);
        let l = find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30);
        assert_eq!(l.len(), 2);
        assert!(l[0].size >= l[1].size);
        assert!(l[0].path.ends_with("Big"));
        assert!(find_leftovers(
            &tmp.path().join("nope"),
            &installed(&[], &[]),
            SystemTime::now(),
            1,
            1
        )
        .is_empty());
    }

    #[test]
    fn leftovers_never_modify_anything() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let d = make_folder(&base, "GoneApp", 2 * MIB, 200);
        find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30);
        assert!(d.join("data.bin").exists());
    }

    #[test]
    fn leftover_line_shows_path_and_size() {
        let l = Leftover {
            path: PathBuf::from("/x/Application Support/GoneApp"),
            size: 2 * 1024 * 1024,
        };
        let s = format_leftover_line(&l);
        assert!(s.contains("/x/Application Support/GoneApp"));
        assert!(s.contains("2.0 MB"));
    }

    #[test]
    fn cleaner_leftovers_use_injected_app_roots() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "GoneApp", 2 * MIB, 200);
        make_folder(&base, "Foo", 2 * MIB, 200);
        let apps = tmp.path().join("Apps");
        fs::create_dir_all(apps.join("Foo.app")).unwrap();
        let c = cleaner_for(&tmp, &Fake::default()).with_app_roots(vec![apps]);
        let names: Vec<String> = c
            .leftovers()
            .iter()
            .map(|l| l.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["GoneApp"]);
    }

    #[test]
    fn detect_ignores_leftovers() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "GoneApp", 2 * MIB, 200);
        let c = cleaner_for(&tmp, &Fake::default());
        assert!(matches!(c.detect().status, ScanStatus::Clean));
    }

    #[test]
    fn dry_run_with_leftovers_runs_no_destructive_commands() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let d = make_folder(&base, "GoneApp", 2 * MIB, 200);
        let fake = Fake::default();
        let apps = tmp.path().join("Apps");
        fs::create_dir_all(&apps).unwrap();
        let c = cleaner_for(&tmp, &fake).with_app_roots(vec![apps]);
        let r = c.clean(true, &DeepSuppressReporter).unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert!(d.join("data.bin").exists());
        assert!(fake
            .calls()
            .iter()
            .all(|c| c.starts_with("plutil") || c.starts_with("pgrep")));
    }

    #[test]
    fn open_confirmation_is_needed_only_above_five() {
        assert_eq!(OPEN_CONFIRM_THRESHOLD, 5);
        assert!(!open_needs_confirmation(0));
        assert!(!open_needs_confirmation(1));
        assert!(!open_needs_confirmation(5));
        assert!(open_needs_confirmation(6));
        assert!(open_needs_confirmation(40));
    }

    #[test]
    fn reveal_paths_runs_open_once_per_path_through_runner() {
        let tmp = TempDir::new().unwrap();
        for n in [5usize, 6] {
            let fake = Fake::default();
            let c = cleaner_for(&tmp, &fake);
            let paths: Vec<PathBuf> = (0..n)
                .map(|i| tmp.path().join(format!("Gone{i}")))
                .collect();
            c.reveal_paths(&paths);
            let expected: Vec<String> = paths
                .iter()
                .map(|p| format!("open {}", p.display()))
                .collect();
            assert_eq!(fake.calls(), expected, "n={n}");
        }
    }

    #[test]
    fn reveal_paths_with_nothing_selected_runs_no_command() {
        let tmp = TempDir::new().unwrap();
        let fake = Fake::default();
        let c = cleaner_for(&tmp, &fake);
        c.reveal_paths(&[]);
        assert!(fake.calls().is_empty(), "calls: {:?}", fake.calls());
    }

    // ── trash_mode pin contract ──

    /// One selected allowlisted candidate plus its cleaner. The interactive
    /// select is gated on a real terminal, so the pin tests drive the
    /// deletion body `clean_with_opts` threads its resolved mode into
    /// (`clean_with_opts` resolves `opts.trash_mode.unwrap_or(Global)` and
    /// passes the result to `apply_selection` via `clean_with_mode` and
    /// `clean_cache_dirs`).
    fn selected_candidate_cleaner(tmp: &TempDir) -> (AppSupportCleaner, Vec<Candidate>, PathBuf) {
        let base = base_of(tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let c = cleaner_for(tmp, &Fake::default());
        let sel = scan_candidates(&base, Some(0));
        (c, sel, base.join("Slack/Cache"))
    }

    #[test]
    fn clean_with_opts_pinned_permanent_overrides_the_global() {
        // The CleanOptions.trash_mode pin contract: a pinned Some(Permanent)
        // must delete for real and claim no Trash, even when the
        // process-global mode is trash.
        let _mode = TrashModeGuard::set(true);
        let tmp = TempDir::new().unwrap();
        let (c, sel, dir) = selected_candidate_cleaner(&tmp);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: Some(crate::trash::TrashMode::Permanent),
        };
        // The mode clean_with_opts resolves for this pin.
        let mode = opts.trash_mode.unwrap_or(crate::trash::TrashMode::Global);
        let r = c
            .apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter, mode)
            .unwrap();
        assert!(
            !dir.exists(),
            "the pinned permanent mode must delete for real"
        );
        assert!(
            !r.uses_trash,
            "uses_trash must come from the pinned mode, not the global"
        );
    }

    #[test]
    fn clean_with_opts_pinned_trash_claims_trash() {
        // A pinned Some(Trash) must resolve the deletion seam to trash. The
        // global is pinned to permanent so only the pin can claim Trash.
        // The skip branch drops the seam outcome's claim (uses_trash only
        // accumulates for successful deletions), so the assertion follows
        // the seam truth: a landed Trash move claims Trash, a TCC-skipped
        // one (this sandbox) does not.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let (c, sel, _dir) = selected_candidate_cleaner(&tmp);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: Some(crate::trash::TrashMode::Trash),
        };
        // The mode clean_with_opts resolves for this pin.
        let mode = opts.trash_mode.unwrap_or(crate::trash::TrashMode::Global);
        let r = c
            .apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter, mode)
            .unwrap();
        let deletions_happened = r.skipped.is_empty();
        assert_eq!(
            r.uses_trash, deletions_happened,
            "uses_trash must come from the deletion outcomes' seam truth"
        );
    }

    #[test]
    fn clean_with_opts_none_keeps_the_global_mode() {
        // trash_mode: None is the compat path: the deletion reads the
        // process-global mode. Global permanent deletes for real and claims
        // no Trash.
        let _mode = TrashModeGuard::set(false);
        let tmp = TempDir::new().unwrap();
        let (c, sel, dir) = selected_candidate_cleaner(&tmp);
        let opts = crate::cleaner::CleanOptions {
            all: true,
            include_manual: true,
            older_than_days: None,
            larger_than_mb: None,
            trash_mode: None,
        };
        // The mode clean_with_opts resolves for this pin.
        let mode = opts.trash_mode.unwrap_or(crate::trash::TrashMode::Global);
        let r = c
            .apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter, mode)
            .unwrap();
        assert!(
            !dir.exists(),
            "the global permanent mode must delete for real"
        );
        assert!(!r.uses_trash, "uses_trash must come from the global mode");
    }
}
