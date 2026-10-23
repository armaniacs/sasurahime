use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// Vendors whose containers are never reported as orphans. Apple data is
/// managed by the OS, and Docker has its own cleaner.
const PROTECTED_VENDORS: &[&str] = &["com.apple", "com.docker"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Containers,
    GroupContainers,
}

impl ContainerKind {
    /// Directory name under `~/Library`, also used as the display label.
    pub fn label(self) -> &'static str {
        match self {
            ContainerKind::Containers => "Containers",
            ContainerKind::GroupContainers => "Group Containers",
        }
    }
}

/// Returns the leading team ID when `name` starts with `"<10 chars of A-Z0-9>."`.
pub fn team_id_prefix(name: &str) -> Option<&str> {
    let (head, _rest) = name.split_once('.')?;
    let is_team_id = head.len() == 10
        && head
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if is_team_id {
        Some(head)
    } else {
        None
    }
}

// `get` instead of slicing: folder names may contain multi-byte characters,
// and slicing at a non-boundary would panic.
fn strip_group_prefix(s: &str) -> &str {
    for prefix in ["groups.", "group."] {
        if let Some(head) = s.get(..prefix.len()) {
            if s.len() > prefix.len() && head.eq_ignore_ascii_case(prefix) {
                return &s[prefix.len()..];
            }
        }
    }
    s
}

/// Turns a folder name into a lowercase bundle ID for matching.
/// Group Containers lose a leading team ID and then a leading `group.` /
/// `groups.`. Containers names are only lowercased.
pub fn normalize_container_id(name: &str, kind: ContainerKind) -> String {
    let stripped = match kind {
        ContainerKind::Containers => name,
        ContainerKind::GroupContainers => {
            let without_team = match team_id_prefix(name) {
                Some(team) => &name[team.len() + 1..],
                None => name,
            };
            strip_group_prefix(without_team)
        }
    };
    stripped.to_ascii_lowercase()
}

fn is_same_or_child(id: &str, parent: &str) -> bool {
    id == parent
        || (id.len() > parent.len()
            && id.starts_with(parent)
            && id.as_bytes()[parent.len()] == b'.')
}

/// `id` must already be normalized (lowercase).
pub fn is_protected_id(id: &str) -> bool {
    PROTECTED_VENDORS.iter().any(|v| is_same_or_child(id, v))
}

/// True when `id` equals an installed ID or is a dot-separated descendant of
/// one (`com.foo.app.helper` belongs to `com.foo.app`; `com.foo.appx` does not).
/// Both `id` and the set members must be lowercase.
pub fn is_owned_by_installed(id: &str, installed: &HashSet<String>) -> bool {
    if installed.contains(id) {
        return true;
    }
    id.bytes()
        .enumerate()
        .any(|(i, b)| b == b'.' && installed.contains(&id[..i]))
}

/// Returns the folder names (original spelling, input order) that look orphaned.
/// For Group Containers, a team ID that owns at least one installed app keeps
/// all of its folders, because the same team often uses unrelated group names
/// (`TEAM.slack` next to `TEAM.com.tinyspeck.slackmacgap`).
pub fn find_orphan_names(
    kind: ContainerKind,
    names: &[String],
    installed: &HashSet<String>,
) -> Vec<String> {
    let entries: Vec<(&String, String)> = names
        .iter()
        .map(|n| (n, normalize_container_id(n, kind)))
        .collect();

    let alive_teams: HashSet<&str> = if kind == ContainerKind::GroupContainers {
        entries
            .iter()
            .filter(|(_, id)| !id.is_empty() && is_owned_by_installed(id, installed))
            .filter_map(|(n, _)| team_id_prefix(n.as_str()))
            .collect()
    } else {
        HashSet::new()
    };

    entries
        .into_iter()
        .filter(|(n, id)| {
            if id.is_empty() || n.starts_with('.') {
                return false;
            }
            if is_protected_id(id) || is_owned_by_installed(id, installed) {
                return false;
            }
            match team_id_prefix(n.as_str()) {
                Some(team) => !alive_teams.contains(team),
                None => true,
            }
        })
        .map(|(n, _)| n.clone())
        .collect()
}

/// The only values taken from an app's `Info.plist`. README promises that
/// nothing else is read out of it, so do not add fields here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub bundle_id: String,
    pub name: Option<String>,
    pub display_name: Option<String>,
}

const PLIST_KEY_BUNDLE_ID: &str = "CFBundleIdentifier";
const PLIST_KEY_NAME: &str = "CFBundleName";
const PLIST_KEY_DISPLAY_NAME: &str = "CFBundleDisplayName";

/// Value of `<key>{key}</key>` when it is directly followed by a non-blank
/// `<string>`. A different `<key>` in between means the value is not a string.
fn xml_string_value(xml: &str, key: &str) -> Option<String> {
    const OPEN: &str = "<string>";
    const CLOSE: &str = "</string>";
    let key_tag = format!("<key>{key}</key>");
    let after = &xml[xml.find(&key_tag)? + key_tag.len()..];
    let open_at = after.find(OPEN)?;
    if after.find("<key>").is_some_and(|k| k < open_at) {
        return None;
    }
    let rest = &after[open_at + OPEN.len()..];
    let value = rest[..rest.find(CLOSE)?].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// `None` when `CFBundleIdentifier` is missing; the two names are optional.
pub fn parse_app_identity_from_xml(xml: &str) -> Option<AppIdentity> {
    Some(AppIdentity {
        bundle_id: xml_string_value(xml, PLIST_KEY_BUNDLE_ID)?,
        name: xml_string_value(xml, PLIST_KEY_NAME),
        display_name: xml_string_value(xml, PLIST_KEY_DISPLAY_NAME),
    })
}

/// Reads an `Info.plist`. Binary plists (magic `bplist00`) are turned into XML
/// on stdout by `plutil`, which ships with macOS, so no plist crate is needed.
/// `-o -` matters: without it `plutil -convert` rewrites the file in place.
pub fn read_app_identity(plist: &Path, runner: &dyn CommandRunner) -> Option<AppIdentity> {
    let bytes = fs::read(plist).ok()?;
    if bytes.starts_with(b"bplist00") {
        let path = plist.to_str()?;
        let out = runner
            .run("plutil", &["-convert", "xml1", "-o", "-", path])
            .ok()?;
        if !out.status.success() {
            return None;
        }
        return parse_app_identity_from_xml(std::str::from_utf8(&out.stdout).ok()?);
    }
    parse_app_identity_from_xml(std::str::from_utf8(&bytes).ok()?)
}

/// Lists `*.app` under each root (top level plus one sub-folder level) with
/// the shared `.app` bundle probe from the app-support cleaner, which also
/// owns the traversal rules.
/// Identities of the apps found under `app_dirs`, in directory order.
/// Bundles whose `Info.plist` cannot be read are skipped.
pub fn collect_app_identities(
    app_dirs: &[PathBuf],
    runner: &dyn CommandRunner,
) -> Vec<AppIdentity> {
    app_dirs
        .iter()
        .flat_map(|dir| crate::cleaners::app_support::list_app_bundles(std::slice::from_ref(dir)))
        .filter_map(|app| read_app_identity(&app.join("Contents/Info.plist"), runner))
        .collect()
}

/// Lowercase bundle IDs of the apps found under `app_dirs`.
pub fn collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String> {
    collect_app_identities(app_dirs, runner)
        .into_iter()
        .map(|app| app.bundle_id.to_ascii_lowercase())
        .collect()
}

pub fn default_app_dirs(home: &Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications"),
        home.join("Applications"),
        PathBuf::from("/System/Applications"),
    ]
}

#[derive(Debug, Clone)]
pub struct OrphanCandidate {
    pub path: PathBuf,
    pub name: String,
    pub kind: ContainerKind,
    pub size: u64,
}

#[derive(Debug, Default)]
pub struct OrphanScan {
    /// Sorted by size, largest first.
    pub candidates: Vec<OrphanCandidate>,
    /// Folders (or whole roots) that could not be listed. Without Full Disk
    /// Access, macOS denies access to other apps' containers.
    pub unreadable: usize,
}

/// Names of real sub-directories. Symlinks and plain files are skipped:
/// `DirEntry::file_type` does not follow symlinks.
fn child_dir_names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)?.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

pub fn scan_roots(roots: &[(ContainerKind, PathBuf)], installed: &HashSet<String>) -> OrphanScan {
    let mut scan = OrphanScan::default();
    for (kind, root) in roots {
        let names = match child_dir_names(root) {
            Ok(names) => names,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                scan.unreadable += 1;
                continue;
            }
        };
        for name in find_orphan_names(*kind, &names, installed) {
            let path = root.join(&name);
            if fs::read_dir(&path).is_err() {
                scan.unreadable += 1;
                continue;
            }
            let size = dir_size(&path);
            if size == 0 {
                continue;
            }
            scan.candidates.push(OrphanCandidate {
                path,
                name,
                kind: *kind,
                size,
            });
        }
    }
    scan.candidates
        .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    scan
}

pub fn status_from_scan(scan: &OrphanScan) -> crate::cleaner::ScanStatus {
    let total: u64 = scan.candidates.iter().map(|c| c.size).sum();
    if total > 0 {
        crate::cleaner::ScanStatus::Pruneable(total)
    } else {
        crate::cleaner::ScanStatus::Clean
    }
}

pub struct OrphanContainersCleaner {
    home: PathBuf,
    app_dirs: Vec<PathBuf>,
    runner: Box<dyn CommandRunner>,
}

impl OrphanContainersCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            app_dirs: default_app_dirs(home),
            runner,
        }
    }

    /// Replaces the install locations searched for `.app` bundles, so tests
    /// never read the real `/Applications`.
    #[cfg(test)]
    pub fn with_app_dirs(mut self, app_dirs: Vec<PathBuf>) -> Self {
        self.app_dirs = app_dirs;
        self
    }

    fn roots(&self) -> Vec<(ContainerKind, PathBuf)> {
        vec![
            (
                ContainerKind::Containers,
                self.home.join("Library/Containers"),
            ),
            (
                ContainerKind::GroupContainers,
                self.home.join("Library/Group Containers"),
            ),
        ]
    }

    fn any_root_exists(&self) -> bool {
        self.roots().iter().any(|(_, dir)| dir.exists())
    }

    pub fn scan(&self) -> OrphanScan {
        let installed = collect_installed_ids(&self.app_dirs, self.runner.as_ref());
        scan_roots(&self.roots(), &installed)
    }
}

pub fn format_item(c: &OrphanCandidate) -> String {
    format!(
        "{:<56}  {:>10}  [{}]",
        c.name,
        format_bytes(c.size),
        c.kind.label()
    )
}

/// Message shown when some folders could not be examined. `None` when all were readable.
pub fn unreadable_hint(count: usize) -> Option<String> {
    if count == 0 {
        return None;
    }
    Some(format!(
        "[orphan-containers] {count} folder(s) could not be read and were not checked. \
         Grant Full Disk Access to your terminal \
         (System Settings > Privacy & Security > Full Disk Access) to check all of them."
    ))
}

fn empty_result(name: &'static str) -> CleanResult {
    CleanResult::empty(name)
}

fn print_unreadable_hint(count: usize) {
    if let Some(hint) = unreadable_hint(count) {
        eprintln!("{hint}");
    }
}

impl OrphanContainersCleaner {
    fn interactive_select(candidates: &[OrphanCandidate]) -> Result<Vec<OrphanCandidate>> {
        use dialoguer::MultiSelect;

        let items: Vec<String> = candidates.iter().map(format_item).collect();
        println!("\nContainers of apps that are not installed (nothing is selected by default):\n");
        let selections = MultiSelect::new()
            .with_prompt("Select folders to move to Trash  [space to toggle, enter to confirm]")
            .items(&items)
            .interact()?;
        Ok(selections
            .into_iter()
            .map(|i| candidates[i].clone())
            .collect())
    }

    fn delete_selected(
        &self,
        selected: &[OrphanCandidate],
        reporter: &dyn ProgressReporter,
    ) -> Result<CleanResult> {
        let mut freed: u64 = 0;
        let mut uses_trash = false;
        let mut skipped = Vec::new();
        let mut deleted_paths = Vec::new();
        reporter.progress_init(self.name(), selected.len());
        for (i, c) in selected.iter().enumerate() {
            reporter.progress_tick(&c.path, i + 1, c.size);
            let outcome = match crate::trash::safe_delete(
                self.runner.as_ref(),
                &c.path,
                crate::trash::TrashMode::Global,
            ) {
                Ok(outcome) => outcome,
                // A fatal per-entry error (e.g. the path vanished between
                // scan and delete) must not abort the whole run: report it
                // and continue with the remaining entries.
                Err(err) => {
                    eprintln!(
                        "[orphan-containers] error removing {}: {err:#}",
                        c.path.display()
                    );
                    skipped.push(SkippedEntry {
                        path: c.path.clone(),
                        reason: format!("{err:#}"),
                    });
                    continue;
                }
            };
            if let Some(reason) = outcome.skip_reason {
                eprintln!(
                    "[orphan-containers] error removing {}: {reason}",
                    c.path.display()
                );
                skipped.push(SkippedEntry {
                    path: c.path.clone(),
                    reason,
                });
                continue;
            }
            uses_trash |= outcome.uses_trash;
            freed += c.size;
            deleted_paths.push(c.path.clone());
            println!(
                "[orphan-containers] removed: {} (freed {})",
                c.path.display(),
                format_bytes(c.size)
            );
        }
        reporter.progress_finish();
        println!("[orphan-containers] total freed: {}", format_bytes(freed));
        Ok(CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash,
            skipped,
            deleted_paths,
        })
    }
}

impl Cleaner for OrphanContainersCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Manual
    }

    fn name(&self) -> &'static str {
        "orphan-containers"
    }

    fn detect(&self) -> ScanResult {
        if !self.any_root_exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let mut r = ScanResult::new(self.name(), status_from_scan(&self.scan()));
        if crate::context::is_verbose() {
            r = r.with_target(self.home.join("Library/Containers").to_string_lossy());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.any_root_exists() {
            println!("[orphan-containers] nothing to clean");
            return Ok(empty_result(self.name()));
        }

        let scan = self.scan();
        if scan.candidates.is_empty() {
            println!("[orphan-containers] nothing to clean");
            print_unreadable_hint(scan.unreadable);
            return Ok(empty_result(self.name()));
        }

        if dry_run {
            println!(
                "[orphan-containers] dry-run: {} orphan candidate(s)",
                scan.candidates.len()
            );
            for c in &scan.candidates {
                println!(
                    "  would remove: {}  ({}, {})",
                    c.path.display(),
                    format_bytes(c.size),
                    c.kind.label()
                );
            }
            print_unreadable_hint(scan.unreadable);
            return Ok(empty_result(self.name()));
        }

        // Which folders are safe to remove is a judgement call that only the
        // user can make, so there is no non-interactive path (no `--all`).
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "[orphan-containers] not a terminal - skipping. Run \
                 `sasurahime clean orphan-containers` to choose folders interactively."
            );
            return Ok(empty_result(self.name()));
        }

        let selected = Self::interactive_select(&scan.candidates)?;
        if selected.is_empty() {
            println!("[orphan-containers] nothing selected");
            print_unreadable_hint(scan.unreadable);
            return Ok(empty_result(self.name()));
        }

        let result = self.delete_selected(&selected, reporter)?;
        print_unreadable_hint(scan.unreadable);
        Ok(result)
    }

    // Which folders are leftovers is a human judgement, so `--yes` must skip this
    // cleaner even on a terminal where the prompt would otherwise appear.
    fn auto_clean_allowed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn strings(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn team_id_prefix_accepts_only_ten_upper_alnum() {
        assert_eq!(team_id_prefix("S8EX82NJP6.com.foo"), Some("S8EX82NJP6"));
        assert_eq!(team_id_prefix("S8EX82NJP.com.foo"), None);
        assert_eq!(team_id_prefix("S8EX82NJP66.com.foo"), None);
        assert_eq!(team_id_prefix("s8ex82njp6.com.foo"), None);
        assert_eq!(team_id_prefix("com.foo.App"), None);
        assert_eq!(team_id_prefix("S8EX82NJP6"), None);
    }

    #[test]
    fn normalize_container_id_table() {
        use ContainerKind::{Containers as C, GroupContainers as G};
        let cases: &[(&str, ContainerKind, &str)] = &[
            ("com.foo.App", C, "com.foo.app"),
            ("S8EX82NJP6.com.foo", C, "s8ex82njp6.com.foo"),
            (
                "S8EX82NJP6.com.macpaw.CleanMyMac-mas.HealthMonitor",
                G,
                "com.macpaw.cleanmymac-mas.healthmonitor",
            ),
            (
                "243LU875E5.groups.com.apple.podcasts",
                G,
                "com.apple.podcasts",
            ),
            ("BQR82RBBHL.slack", G, "slack"),
            ("group.com.foo.App", G, "com.foo.app"),
            ("S8EX82NJP6.group.com.foo", G, "com.foo"),
            ("com.apple.Home.group", G, "com.apple.home.group"),
            ("ABCDE1234.foo", G, "abcde1234.foo"),
            ("abcde12345.foo", G, "abcde12345.foo"),
            ("S8EX82NJP6.", G, ""),
            ("group.", G, "group."),
            ("\u{3042}.com", G, "\u{3042}.com"),
        ];
        for (name, kind, expected) in cases {
            assert_eq!(
                normalize_container_id(name, *kind),
                *expected,
                "name={name} kind={kind:?}"
            );
        }
    }

    #[test]
    fn is_protected_id_table() {
        let cases: &[(&str, bool)] = &[
            ("com.apple.safari", true),
            ("com.apple", true),
            ("com.applesauce.x", false),
            ("com.docker.docker", true),
            ("com.docker", true),
            ("com.dockerx", false),
            ("com.foo.apple", false),
        ];
        for (id, expected) in cases {
            assert_eq!(is_protected_id(id), *expected, "id={id}");
        }
    }

    #[test]
    fn is_owned_by_installed_uses_dot_boundary() {
        let installed = set(&["com.foo.app"]);
        assert!(is_owned_by_installed("com.foo.app", &installed));
        assert!(is_owned_by_installed("com.foo.app.helper", &installed));
        assert!(is_owned_by_installed("com.foo.app.a.b", &installed));
        assert!(!is_owned_by_installed("com.foo.appx", &installed));
        assert!(!is_owned_by_installed("com.foo", &installed));
        assert!(!is_owned_by_installed("com.bar.app", &installed));
    }

    #[test]
    fn find_orphan_names_containers() {
        let names = strings(&[
            "com.foo.App",
            "com.foo.App.Helper",
            "com.foo.AppX",
            "com.apple.Safari",
            "com.docker.docker",
            "com.bar.Gone",
            ".hidden",
        ]);
        let got = find_orphan_names(ContainerKind::Containers, &names, &set(&["com.foo.app"]));
        assert_eq!(got, strings(&["com.foo.AppX", "com.bar.Gone"]));
    }

    #[test]
    fn find_orphan_names_group_containers() {
        let names = strings(&[
            "BQR82RBBHL.com.tinyspeck.slackmacgap",
            "BQR82RBBHL.slack",
            "243LU875E5.groups.com.apple.podcasts",
            "ZZZZZ99999.group.com.example.gone",
            "group.com.foo.App",
            "com.apple.Home.group",
            "S8EX82NJP6.",
            "UBF8T346G9.com.microsoft.oneauth",
        ]);
        let installed = set(&["com.tinyspeck.slackmacgap", "com.foo.app"]);
        let got = find_orphan_names(ContainerKind::GroupContainers, &names, &installed);
        assert_eq!(
            got,
            strings(&[
                "ZZZZZ99999.group.com.example.gone",
                "UBF8T346G9.com.microsoft.oneauth"
            ])
        );
    }

    #[test]
    fn find_orphan_names_group_exact_id_with_team_prefix_is_kept() {
        let names = strings(&["BQR82RBBHL.com.tinyspeck.slackmacgap"]);
        let installed = set(&["com.tinyspeck.slackmacgap"]);
        assert!(find_orphan_names(ContainerKind::GroupContainers, &names, &installed).is_empty());
    }

    #[test]
    fn find_orphan_names_empty_input() {
        assert!(find_orphan_names(ContainerKind::Containers, &[], &set(&[])).is_empty());
    }

    use crate::test_helpers::MockRunner;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Mutex;
    use tempfile::TempDir;

    const SECRET_URL_SCHEME: &str = "sasurahime-secret-url-scheme-7f3a";
    const SECRET_VALUE: &str = "SasurahimeSecretValue7f3a";

    /// Answers every `plutil` call with the same stdout and records the arguments.
    struct PlutilStub {
        stdout: String,
        exit_code: i32,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl PlutilStub {
        fn new(stdout: &str, exit_code: i32) -> Self {
            Self {
                stdout: stdout.to_string(),
                exit_code,
                calls: Mutex::new(vec![]),
            }
        }
    }

    impl CommandRunner for PlutilStub {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            if program != "plutil" {
                anyhow::bail!("unexpected program {program}");
            }
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(self.exit_code << 8),
                stdout: self.stdout.as_bytes().to_vec(),
                stderr: vec![],
            })
        }

        fn exists(&self, program: &str) -> bool {
            program == "plutil"
        }
    }

    fn plist_xml(bundle_id: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleName</key>\n\t<string>X</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>{bundle_id}</string>\n</dict>\n</plist>\n"
        )
    }

    /// A plist whose other keys hold values that must never come out of the
    /// reader. They are placed before the three keys on purpose.
    fn plist_xml_with_other_keys(bundle_id: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>{SECRET_URL_SCHEME}</string>
            </array>
        </dict>
    </array>
    <key>SasurahimeDummyKey</key>
    <string>{SECRET_VALUE}</string>
    <key>CFBundleIdentifier</key>
    <string>{bundle_id}</string>
    <key>CFBundleName</key>
    <string>Keeper</string>
    <key>CFBundleDisplayName</key>
    <string>Keeper Display</string>
</dict>
</plist>
"#
        )
    }

    fn write_app(apps_dir: &Path, rel: &str, bundle_id: &str) {
        let contents = apps_dir.join(rel).join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(contents.join("Info.plist"), plist_xml(bundle_id)).unwrap();
    }

    #[test]
    fn parse_app_identity_from_xml_reads_three_keys() {
        assert_eq!(
            parse_app_identity_from_xml(&plist_xml("com.foo.App")),
            Some(AppIdentity {
                bundle_id: "com.foo.App".to_string(),
                name: Some("X".to_string()),
                display_name: None,
            })
        );
    }

    #[test]
    fn parse_app_identity_from_xml_missing_bundle_id_is_none() {
        let xml = "<dict><key>CFBundleName</key><string>X</string></dict>";
        assert_eq!(parse_app_identity_from_xml(xml), None);
    }

    #[test]
    fn parse_app_identity_from_xml_key_without_string_is_none() {
        let xml =
            "<dict><key>CFBundleIdentifier</key><true/><key>Other</key><string>y</string></dict>";
        assert_eq!(parse_app_identity_from_xml(xml), None);
    }

    #[test]
    fn parse_app_identity_from_xml_empty_bundle_id_is_none() {
        let xml = "<dict><key>CFBundleIdentifier</key><string>  </string></dict>";
        assert_eq!(parse_app_identity_from_xml(xml), None);
    }

    #[test]
    fn parse_app_identity_from_xml_ignores_other_keys() {
        let identity =
            parse_app_identity_from_xml(&plist_xml_with_other_keys("com.foo.Keeper")).unwrap();
        assert_eq!(
            identity,
            AppIdentity {
                bundle_id: "com.foo.Keeper".to_string(),
                name: Some("Keeper".to_string()),
                display_name: Some("Keeper Display".to_string()),
            }
        );
        let shown = format!("{identity:?}");
        assert!(!shown.contains(SECRET_URL_SCHEME), "{shown}");
        assert!(!shown.contains(SECRET_VALUE), "{shown}");
    }

    #[test]
    fn app_identity_has_exactly_three_fields() {
        // No `..` in the pattern: adding a field to AppIdentity breaks this
        // test on purpose, because README promises only these three values.
        let AppIdentity {
            bundle_id,
            name,
            display_name,
        } = AppIdentity {
            bundle_id: "com.foo.App".to_string(),
            name: None,
            display_name: None,
        };
        assert_eq!(bundle_id, "com.foo.App");
        assert!(name.is_none() && display_name.is_none());
    }

    #[test]
    fn read_app_identity_reads_xml_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, plist_xml("com.foo.App")).unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(
            read_app_identity(&p, &runner).map(|a| a.bundle_id),
            Some("com.foo.App".to_string())
        );
    }

    #[test]
    fn read_app_identity_garbage_is_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(read_app_identity(&p, &runner), None);
    }

    #[test]
    fn read_app_identity_missing_file_is_none() {
        let tmp = TempDir::new().unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(
            read_app_identity(&tmp.path().join("nope.plist"), &runner),
            None
        );
    }

    #[test]
    fn read_app_identity_binary_plist_converts_with_plutil() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, b"bplist00\x00\x01\x02").unwrap();
        let runner = PlutilStub::new(&plist_xml_with_other_keys("com.example.bin"), 0);
        let identity = read_app_identity(&p, &runner).unwrap();
        assert_eq!(identity.bundle_id, "com.example.bin");
        assert_eq!(identity.name.as_deref(), Some("Keeper"));
        let shown = format!("{identity:?}");
        assert!(!shown.contains(SECRET_URL_SCHEME), "{shown}");
        assert!(!shown.contains(SECRET_VALUE), "{shown}");
        let path = p.to_str().unwrap().to_string();
        assert_eq!(
            *runner.calls.lock().unwrap(),
            vec![vec![
                "-convert".to_string(),
                "xml1".to_string(),
                "-o".to_string(),
                "-".to_string(),
                path,
            ]]
        );
    }

    #[test]
    fn read_app_identity_binary_plist_plutil_failure_is_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, b"bplist00\x00\x01\x02").unwrap();
        let failing = PlutilStub::new("", 1);
        assert_eq!(read_app_identity(&p, &failing), None);
        let not_found = MockRunner::new().with_not_found();
        assert_eq!(read_app_identity(&p, &not_found), None);
    }

    #[test]
    fn collect_installed_ids_scans_top_level_and_one_subfolder() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        write_app(&apps, "Foo.app", "com.foo.App");
        write_app(&apps, "Utilities/Bar.app", "com.bar.Tool");
        write_app(&apps, "Utilities/Deep/Baz.app", "com.baz.TooDeep");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[apps], &runner);
        assert_eq!(ids, set(&["com.foo.app", "com.bar.tool"]));
    }

    #[test]
    fn collect_installed_ids_ignores_broken_and_non_app_entries() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Broken.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("NotAnApp/Contents")).unwrap();
        fs::write(
            apps.join("NotAnApp/Contents/Info.plist"),
            plist_xml("com.no.App"),
        )
        .unwrap();
        fs::write(apps.join("file.app"), b"plain file").unwrap();
        write_app(&apps, "Good.app", "com.good.App");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[apps], &runner);
        assert_eq!(ids, set(&["com.good.app"]));
    }

    #[test]
    fn collect_installed_ids_handles_missing_dirs_and_multiple_roots() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("A");
        let b = tmp.path().join("B");
        write_app(&a, "One.app", "com.one.App");
        write_app(&b, "Two.app", "com.two.App");
        let missing = tmp.path().join("missing");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[a, missing, b], &runner);
        assert_eq!(ids, set(&["com.one.app", "com.two.app"]));
    }

    #[test]
    fn collect_app_identities_returns_only_the_three_values() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        let contents = apps.join("Keeper.app/Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(
            contents.join("Info.plist"),
            plist_xml_with_other_keys("com.foo.Keeper"),
        )
        .unwrap();
        let runner = MockRunner::new().with_not_found();
        let found = collect_app_identities(&[apps], &runner);
        assert_eq!(
            found,
            vec![AppIdentity {
                bundle_id: "com.foo.Keeper".to_string(),
                name: Some("Keeper".to_string()),
                display_name: Some("Keeper Display".to_string()),
            }]
        );
        let shown = format!("{found:?}");
        assert!(!shown.contains(SECRET_URL_SCHEME), "{shown}");
        assert!(!shown.contains(SECRET_VALUE), "{shown}");
    }

    #[test]
    fn default_app_dirs_lists_three_roots() {
        let dirs = default_app_dirs(Path::new("/Users/x"));
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/Applications"),
                PathBuf::from("/Users/x/Applications"),
                PathBuf::from("/System/Applications"),
            ]
        );
    }

    fn write_container(home: &Path, sub: &str, name: &str) -> PathBuf {
        let dir = home.join("Library").join(sub).join(name);
        fs::create_dir_all(dir.join("Data")).unwrap();
        fs::write(dir.join("Data/blob.bin"), vec![1u8; 8192]).unwrap();
        dir
    }

    fn cleaner_for(tmp: &TempDir) -> OrphanContainersCleaner {
        OrphanContainersCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()))
            .with_app_dirs(vec![tmp.path().join("Applications")])
    }

    #[test]
    fn scan_reports_only_orphans_in_containers() {
        let tmp = TempDir::new().unwrap();
        write_app(
            &tmp.path().join("Applications"),
            "Keep.app",
            "com.example.keep",
        );
        write_container(tmp.path(), "Containers", "com.example.keep");
        write_container(tmp.path(), "Containers", "com.example.keep.Helper");
        write_container(tmp.path(), "Containers", "com.apple.Safari");
        let gone = write_container(tmp.path(), "Containers", "com.example.gone");

        let scan = cleaner_for(&tmp).scan();

        assert_eq!(scan.unreadable, 0);
        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].path, gone);
        assert_eq!(scan.candidates[0].name, "com.example.gone");
        assert_eq!(scan.candidates[0].kind, ContainerKind::Containers);
        assert!(scan.candidates[0].size > 0);
    }

    #[test]
    fn scan_keeps_group_containers_of_a_team_with_an_installed_app() {
        let tmp = TempDir::new().unwrap();
        write_app(
            &tmp.path().join("Applications"),
            "Slack.app",
            "com.tinyspeck.slackmacgap",
        );
        write_container(
            tmp.path(),
            "Group Containers",
            "BQR82RBBHL.com.tinyspeck.slackmacgap",
        );
        write_container(tmp.path(), "Group Containers", "BQR82RBBHL.slack");
        write_container(
            tmp.path(),
            "Group Containers",
            "243LU875E5.groups.com.apple.podcasts",
        );
        write_container(tmp.path(), "Group Containers", "com.apple.Home.group");
        let gone = write_container(
            tmp.path(),
            "Group Containers",
            "ZZZZZ99999.group.com.example.gone",
        );

        let scan = cleaner_for(&tmp).scan();

        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].path, gone);
        assert_eq!(scan.candidates[0].kind, ContainerKind::GroupContainers);
    }

    #[test]
    fn scan_of_missing_or_empty_roots_is_empty() {
        let tmp = TempDir::new().unwrap();
        let scan = cleaner_for(&tmp).scan();
        assert!(scan.candidates.is_empty());
        assert_eq!(scan.unreadable, 0);

        fs::create_dir_all(tmp.path().join("Library/Containers")).unwrap();
        fs::create_dir_all(tmp.path().join("Library/Group Containers")).unwrap();
        let scan = cleaner_for(&tmp).scan();
        assert!(scan.candidates.is_empty());
        assert_eq!(scan.unreadable, 0);
    }

    #[test]
    fn scan_skips_symlinks_files_and_zero_size_folders() {
        let tmp = TempDir::new().unwrap();
        let real = write_container(tmp.path(), "Containers", "com.example.real");
        let containers = tmp.path().join("Library/Containers");
        std::os::unix::fs::symlink(&real, containers.join("com.example.link")).unwrap();
        fs::write(containers.join("com.example.file"), vec![1u8; 8192]).unwrap();
        fs::create_dir_all(containers.join("com.example.empty")).unwrap();

        let scan = cleaner_for(&tmp).scan();

        let names: Vec<&str> = scan.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.real"]);
    }

    #[test]
    fn scan_counts_unreadable_folders_without_listing_them() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let locked = write_container(tmp.path(), "Containers", "com.example.locked");
        write_container(tmp.path(), "Containers", "com.example.open");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let scan = cleaner_for(&tmp).scan();

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(scan.unreadable, 1);
        let names: Vec<&str> = scan.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.open"]);
    }

    #[test]
    fn scan_sorts_candidates_by_size_descending() {
        let tmp = TempDir::new().unwrap();
        let small = write_container(tmp.path(), "Containers", "com.example.small");
        let big = write_container(tmp.path(), "Containers", "com.example.big");
        fs::write(big.join("Data/more.bin"), vec![1u8; 65536]).unwrap();
        assert!(small.exists());

        let scan = cleaner_for(&tmp).scan();

        let names: Vec<&str> = scan.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.big", "com.example.small"]);
    }

    #[test]
    fn status_from_scan_maps_total_size() {
        use crate::cleaner::ScanStatus;
        let empty = OrphanScan::default();
        assert!(matches!(status_from_scan(&empty), ScanStatus::Clean));
        let one = OrphanScan {
            candidates: vec![OrphanCandidate {
                path: PathBuf::from("/x"),
                name: "x".into(),
                kind: ContainerKind::Containers,
                size: 4096,
            }],
            unreadable: 3,
        };
        assert!(matches!(
            status_from_scan(&one),
            ScanStatus::Pruneable(4096)
        ));
    }

    #[test]
    fn scan_counts_an_unreadable_root_once() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        write_container(tmp.path(), "Containers", "com.example.gone");
        let root = tmp.path().join("Library/Containers");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();

        let scan = cleaner_for(&tmp).scan();

        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(scan.unreadable, 1);
        assert!(scan.candidates.is_empty());
    }

    use crate::progress::SuppressReporter;

    #[test]
    fn format_item_contains_name_size_and_kind() {
        let c = OrphanCandidate {
            path: PathBuf::from("/x/com.example.gone"),
            name: "com.example.gone".into(),
            kind: ContainerKind::GroupContainers,
            size: 2048,
        };
        let s = format_item(&c);
        assert!(s.contains("com.example.gone"), "{s}");
        assert!(s.contains("2.0 KB"), "{s}");
        assert!(s.contains("[Group Containers]"), "{s}");
    }

    #[test]
    fn unreadable_hint_is_none_for_zero_and_mentions_full_disk_access() {
        assert_eq!(unreadable_hint(0), None);
        let hint = unreadable_hint(3).unwrap();
        assert!(hint.contains("3 folder(s)"), "{hint}");
        assert!(hint.contains("Full Disk Access"), "{hint}");
    }

    #[test]
    fn name_is_orphan_containers() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(cleaner_for(&tmp).name(), "orphan-containers");
    }

    #[test]
    fn detect_without_roots_is_not_found() {
        let tmp = TempDir::new().unwrap();
        let r = cleaner_for(&tmp).detect();
        assert!(matches!(r.status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_with_only_installed_containers_is_clean() {
        let tmp = TempDir::new().unwrap();
        write_app(
            &tmp.path().join("Applications"),
            "Keep.app",
            "com.example.keep",
        );
        write_container(tmp.path(), "Containers", "com.example.keep");
        let r = cleaner_for(&tmp).detect();
        assert!(matches!(r.status, ScanStatus::Clean));
    }

    #[test]
    fn detect_with_orphans_is_pruneable_with_total_size() {
        let tmp = TempDir::new().unwrap();
        write_container(tmp.path(), "Containers", "com.example.gone");
        write_container(
            tmp.path(),
            "Group Containers",
            "ZZZZZ99999.group.com.example.gone",
        );
        let c = cleaner_for(&tmp);
        let expected: u64 = c.scan().candidates.iter().map(|x| x.size).sum();
        let r = c.detect();
        assert!(expected > 0);
        assert_eq!(r.bytes_for_test(), expected);
    }

    #[test]
    fn dry_run_lists_without_deleting() {
        let tmp = TempDir::new().unwrap();
        let gone = write_container(tmp.path(), "Containers", "com.example.gone");
        let result = cleaner_for(&tmp).clean(true, &SuppressReporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert!(result.skipped.is_empty());
        assert!(gone.join("Data/blob.bin").exists());
    }

    #[test]
    fn clean_without_roots_is_empty_success() {
        let tmp = TempDir::new().unwrap();
        let result = cleaner_for(&tmp).clean(false, &SuppressReporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(result.exit_code(), 0);
    }

    #[test]
    fn delete_selected_records_failures_as_skipped() {
        // The seam classifies a Trash-mode delete failure as skippable; pin
        // the global mode to Trash under the lock so the classification is
        // deterministic against parallel tests that flip the global pin.
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // Panic-safe pin: the guard restores the previous mode on drop, even
        // when an assert fails mid-test.
        let _mode = crate::test_helpers::TrashModeGuard::set(true);
        let tmp = TempDir::new().unwrap();
        let missing = OrphanCandidate {
            path: tmp.path().join("does-not-exist"),
            name: "does-not-exist".into(),
            kind: ContainerKind::Containers,
            size: 4096,
        };
        let result = cleaner_for(&tmp)
            .delete_selected(&[missing], &SuppressReporter)
            .unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.exit_code(), 1);
    }

    #[test]
    fn orphan_containers_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!cleaner_for(&tmp).auto_clean_allowed());
    }
}
