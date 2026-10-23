use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::format_bytes;
use crate::hint::{apps::KNOWN_ENTRIES, BaseDir, HintEntry};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

pub const EXCLUDED_NAMES: &[(&str, &str)] = &[
    ("Homebrew", "brew"),
    ("node-gyp", "node-gyp"),
    ("org.swift.swiftpm", "spm"),
    ("JetBrains", "jetbrains"),
    ("CocoaPods", "cocoapods"),
    ("Yarn", "yarn"),
    ("pip", "pip"),
    ("go-build", "go"),
    ("pypoetry", "poetry"),
    ("deno", "deno"),
];

pub const EXCLUDED_PREFIXES: &[(&str, &str)] =
    &[("ms-playwright", "browsers"), ("com.apple.", "os-managed")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Deletable,
    BelowThreshold,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub name: String,
    pub display_name: String,
    pub size: u64,
    pub disposition: Disposition,
    pub process_name: Option<&'static str>,
    pub running: bool,
}

/// Returns the clean target that owns `name`, or None when it is not excluded.
pub fn excluded_by(name: &str) -> Option<&'static str> {
    if let Some((_, target)) = EXCLUDED_NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
    {
        return Some(target);
    }
    let lower = name.to_ascii_lowercase();
    EXCLUDED_PREFIXES
        .iter()
        .find(|(p, _)| lower.starts_with(&p.to_ascii_lowercase()))
        .map(|(_, target)| *target)
}

// `skip: true` entries (GeoServices) are OS-managed and must not become deletable.
pub fn known_entry_for(name: &str) -> Option<&'static HintEntry> {
    KNOWN_ENTRIES
        .iter()
        .find(|e| e.base_dir == BaseDir::Caches && !e.skip && e.path_suffixes == [name])
}

pub fn classify(
    known: Option<&HintEntry>,
    size: u64,
    threshold_override: Option<u64>,
) -> Disposition {
    match known {
        None => Disposition::Unknown,
        Some(e) => {
            let threshold = threshold_override.unwrap_or(e.threshold_bytes);
            if size > threshold {
                Disposition::Deletable
            } else {
                Disposition::BelowThreshold
            }
        }
    }
}

/// Lists first-level entries of `dir`, largest first.
/// Sizes are computed only for entries that will be shown: excluded, hidden and
/// symlinked entries are never measured, and unknown apps are measured only when
/// `include_unknown` is true (detect() passes false to avoid walking the whole tree).
pub fn scan_caches(
    dir: &Path,
    include_unknown: bool,
    threshold_override: Option<u64>,
) -> Vec<CacheEntry> {
    let Ok(read) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut entries = Vec::new();
    for item in read.flatten() {
        let path = item.path();
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if name.starts_with('.') || excluded_by(&name).is_some() {
            continue;
        }
        // An unreadable file type is treated as a symlink: never touch what we cannot classify.
        if item.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
            continue;
        }
        let known = known_entry_for(&name);
        if known.is_none() && !include_unknown {
            continue;
        }
        let size = crate::scan_cache::cached_dir_size(&path);
        if size == 0 {
            continue;
        }
        entries.push(CacheEntry {
            display_name: known
                .map(|e| e.display_name.to_string())
                .unwrap_or_else(|| name.clone()),
            process_name: known.and_then(|e| e.process_name),
            disposition: classify(known, size, threshold_override),
            path,
            name,
            size,
            running: false,
        });
    }
    entries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    entries
}

pub fn mark_running(entries: &mut [CacheEntry], runner: &dyn CommandRunner) {
    for e in entries.iter_mut() {
        if e.disposition != Disposition::Deletable {
            continue;
        }
        if let Some(p) = e.process_name {
            e.running = crate::hint::is_running(p, runner);
        }
    }
}

pub fn render_listing(entries: &[CacheEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| {
            let tag = match (e.disposition, e.running) {
                (Disposition::Deletable, true) => "  [running]",
                (Disposition::Deletable, false) => "",
                (Disposition::BelowThreshold, _) => "  [below threshold]",
                (Disposition::Unknown, _) => "  [display only]",
            };
            format!("  {:<32} {:>10}{}", e.name, format_bytes(e.size), tag)
        })
        .collect()
}

pub fn render_item(entry: &CacheEntry) -> String {
    let running = if entry.running {
        "  [running: will be skipped]"
    } else {
        ""
    };
    format!(
        "{:<32} {:>10}  {}{}",
        entry.name,
        format_bytes(entry.size),
        entry.display_name,
        running
    )
}

pub fn split_running(selected: Vec<CacheEntry>) -> (Vec<CacheEntry>, Vec<SkippedEntry>) {
    let mut to_delete = Vec::new();
    let mut skipped = Vec::new();
    for e in selected {
        if e.running {
            skipped.push(SkippedEntry {
                reason: format!("{} is running: quit it first", e.display_name),
                path: e.path,
            });
        } else {
            to_delete.push(e);
        }
    }
    (to_delete, skipped)
}

fn empty_result(name: &'static str) -> CleanResult {
    CleanResult {
        name,
        bytes_freed: 0,
        uses_trash: false,
        skipped: vec![],
        deleted_paths: vec![],
    }
}

pub struct LibraryCachesCleaner {
    pub home: PathBuf,
    pub runner: Box<dyn CommandRunner>,
    pub threshold_override: Option<u64>,
}

impl LibraryCachesCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            threshold_override: None,
        }
    }

    fn caches_dir(&self) -> PathBuf {
        self.home.join("Library/Caches")
    }

    pub(crate) fn scan(&self, include_unknown: bool) -> Vec<CacheEntry> {
        scan_caches(&self.caches_dir(), include_unknown, self.threshold_override)
    }

    fn delete_entries(
        &self,
        entries: &[CacheEntry],
        reporter: &dyn ProgressReporter,
    ) -> CleanResult {
        let mut result = empty_result(self.name());
        result.uses_trash = true;
        reporter.progress_init(self.name(), entries.len());
        for (i, e) in entries.iter().enumerate() {
            reporter.progress_tick(&e.path, i + 1, e.size);
            let path_str = e.path.to_string_lossy();
            let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
            match crate::trash::delete_path(&e.path) {
                Ok(()) => {
                    result.bytes_freed += e.size;
                    println!(
                        "[library-caches] removed: {} (freed {})",
                        e.path.display(),
                        format_bytes(e.size)
                    );
                }
                Err(err) => {
                    eprintln!(
                        "[library-caches] error removing {}: {err}",
                        e.path.display()
                    );
                    result.skipped.push(SkippedEntry {
                        path: e.path.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        reporter.progress_finish();
        result
    }

    fn interactive_select(&self, entries: &[CacheEntry]) -> Result<Vec<CacheEntry>> {
        use dialoguer::MultiSelect;
        let items: Vec<String> = entries.iter().map(render_item).collect();
        let defaults = vec![false; entries.len()];
        println!("\nSelect known app caches to move to Trash:\n");
        let picked = MultiSelect::new()
            .items(&items)
            .defaults(&defaults)
            .interact()?;
        Ok(picked.into_iter().map(|i| entries[i].clone()).collect())
    }
}

impl Cleaner for LibraryCachesCleaner {
    fn risk(&self) -> crate::cleaner::Risk {
        crate::cleaner::Risk::Manual
    }

    fn name(&self) -> &'static str {
        "library-caches"
    }

    fn detect(&self) -> ScanResult {
        let dir = self.caches_dir();
        if !dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let total: u64 = self
            .scan(false)
            .iter()
            .filter(|e| e.disposition == Disposition::Deletable)
            .map(|e| e.size)
            .sum();
        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.caches_dir().exists() {
            return Ok(empty_result(self.name()));
        }
        let mut entries = self.scan(true);
        if entries.is_empty() {
            println!("[library-caches] nothing to clean");
            return Ok(empty_result(self.name()));
        }
        mark_running(&mut entries, self.runner.as_ref());

        println!("\n~/Library/Caches/ by size:\n");
        for line in render_listing(&entries) {
            println!("{line}");
        }

        let deletable: Vec<CacheEntry> = entries
            .into_iter()
            .filter(|e| e.disposition == Disposition::Deletable)
            .collect();

        if dry_run {
            println!(
                "\n[library-caches] dry-run: {} deletable known cache(s)",
                deletable.len()
            );
            for e in &deletable {
                println!("  would remove: {}", e.path.display());
            }
            return Ok(empty_result(self.name()));
        }
        if deletable.is_empty() {
            println!("\n[library-caches] no deletable known caches");
            return Ok(empty_result(self.name()));
        }
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "[library-caches] not a terminal - skipping. \
                 Run `sasurahime clean library-caches` interactively."
            );
            return Ok(empty_result(self.name()));
        }

        let selected = self.interactive_select(&deletable)?;
        if selected.is_empty() {
            println!("[library-caches] nothing selected");
            return Ok(empty_result(self.name()));
        }
        let (to_delete, mut skipped) = split_running(selected);
        let mut result = self.delete_entries(&to_delete, reporter);
        result.skipped.append(&mut skipped);
        println!(
            "[library-caches] total freed: {}",
            format_bytes(result.bytes_freed)
        );
        Ok(result)
    }

    // Removing an app cache is an explicit per-app choice; `--yes` must never pick it.
    fn auto_clean_allowed(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn excluded_by_exact_name() {
        assert_eq!(excluded_by("Homebrew"), Some("brew"));
        assert_eq!(excluded_by("homebrew"), Some("brew"));
        assert_eq!(excluded_by("go-build"), Some("go"));
        assert_eq!(excluded_by("node-gyp"), Some("node-gyp"));
    }

    #[test]
    fn excluded_by_prefix() {
        assert_eq!(excluded_by("ms-playwright"), Some("browsers"));
        assert_eq!(excluded_by("ms-playwright-go"), Some("browsers"));
        assert_eq!(excluded_by("com.apple.Safari"), Some("os-managed"));
    }

    #[test]
    fn excluded_by_returns_none_for_ordinary_apps() {
        assert_eq!(excluded_by("typescript"), None);
        assert_eq!(excluded_by("SomeUnknownApp"), None);
        assert_eq!(excluded_by("com.applesauce.Foo"), None);
    }

    #[test]
    fn known_entry_for_finds_caches_entries() {
        let e = known_entry_for("typescript").unwrap();
        assert_eq!(e.display_name, "TypeScript server cache");
        assert!(known_entry_for("Microsoft Edge").is_some());
    }

    #[test]
    fn known_entry_for_ignores_skip_and_other_base_dirs() {
        assert!(known_entry_for("GeoServices").is_none());
        assert!(known_entry_for("Slack/Cache").is_none());
        assert!(known_entry_for("Claude").is_none());
    }

    #[test]
    fn managed_hint_entries_are_all_excluded() {
        for name in ["Homebrew", "ms-playwright", "ms-playwright-go"] {
            assert!(excluded_by(name).is_some(), "{name} must be excluded");
        }
    }

    #[test]
    fn classify_respects_threshold() {
        let e = known_entry_for("typescript");
        let t = e.unwrap().threshold_bytes;
        assert_eq!(classify(e, t + 1, None), Disposition::Deletable);
        assert_eq!(classify(e, t, None), Disposition::BelowThreshold);
        assert_eq!(classify(None, t * 10, None), Disposition::Unknown);
    }

    #[test]
    fn classify_threshold_override() {
        let e = known_entry_for("typescript");
        assert_eq!(classify(e, 2, Some(1)), Disposition::Deletable);
        assert_eq!(classify(e, 1, Some(1)), Disposition::BelowThreshold);
    }

    fn caches(tmp: &TempDir) -> PathBuf {
        let d = tmp.path().join("Library/Caches");
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(dir: &Path, name: &str, bytes: usize) {
        let d = dir.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("data.bin"), vec![1u8; bytes]).unwrap();
    }

    #[test]
    fn scan_sorts_by_size_desc_and_classifies() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        put(&c, "SomeUnknownApp", 40_000);
        put(&c, "gopls", 1_000);
        let r = scan_caches(&c, true, Some(10_000));
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["SomeUnknownApp", "typescript", "gopls"]);
        assert_eq!(r[0].disposition, Disposition::Unknown);
        assert_eq!(r[1].disposition, Disposition::Deletable);
        assert_eq!(r[2].disposition, Disposition::BelowThreshold);
        assert_eq!(r[1].display_name, "TypeScript server cache");
    }

    #[test]
    fn scan_skips_excluded_hidden_empty_and_symlinks() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "Homebrew", 5_000);
        put(&c, "ms-playwright-go", 5_000);
        put(&c, "com.apple.Safari", 5_000);
        put(&c, ".hidden", 5_000);
        fs::create_dir_all(c.join("emptyapp")).unwrap();
        put(&c, "real", 5_000);
        std::os::unix::fs::symlink(c.join("real"), c.join("linked")).unwrap();
        let r = scan_caches(&c, true, None);
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["real"]);
    }

    #[test]
    fn scan_without_unknown_returns_only_known() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 5_000);
        put(&c, "SomeUnknownApp", 5_000);
        let r = scan_caches(&c, false, Some(1));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "typescript");
    }

    #[test]
    fn scan_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_caches(&tmp.path().join("nope"), true, None).is_empty());
    }

    #[test]
    fn scan_keeps_process_name_of_known_entry() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "Microsoft Edge", 5_000);
        let r = scan_caches(&c, false, Some(1));
        assert_eq!(r[0].process_name, Some("Microsoft Edge Helper"));
        assert!(!r[0].running);
    }

    use crate::cleaner::{Cleaner, ScanStatus};
    use crate::test_helpers::MockRunner;

    fn entry(name: &str, size: u64, d: Disposition, proc_name: Option<&'static str>) -> CacheEntry {
        CacheEntry {
            path: PathBuf::from("/x").join(name),
            name: name.to_string(),
            display_name: name.to_string(),
            size,
            disposition: d,
            process_name: proc_name,
            running: false,
        }
    }

    #[test]
    fn render_listing_tags_each_disposition() {
        let mut running = entry("A", 3 << 20, Disposition::Deletable, Some("A"));
        running.running = true;
        let v = vec![
            running,
            entry("B", 2 << 20, Disposition::Deletable, None),
            entry("C", 1 << 20, Disposition::BelowThreshold, None),
            entry("D", 512, Disposition::Unknown, None),
        ];
        let lines = render_listing(&v);
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("[running]"), "{}", lines[0]);
        assert!(lines[0].contains("3.0 MB"));
        assert!(!lines[1].contains('['), "{}", lines[1]);
        assert!(lines[2].contains("[below threshold]"));
        assert!(lines[3].contains("[display only]"));
    }

    #[test]
    fn render_item_mentions_running() {
        let mut e = entry("A", 1 << 20, Disposition::Deletable, Some("A"));
        assert!(!render_item(&e).contains("running"));
        e.running = true;
        assert!(render_item(&e).contains("running"));
    }

    #[test]
    fn mark_running_uses_pgrep_for_entries_with_process_name() {
        let mut v = vec![
            entry("A", 10, Disposition::Deletable, Some("A")),
            entry("B", 10, Disposition::Deletable, None),
            entry("C", 10, Disposition::Unknown, Some("C")),
        ];
        // MockRunner: a program registered with with_success exits 0 (= running).
        mark_running(&mut v, &MockRunner::new().with_success("pgrep"));
        assert!(v[0].running);
        assert!(!v[1].running, "no process_name => cannot be judged");
        assert!(!v[2].running, "only deletable entries are checked");
    }

    #[test]
    fn mark_running_false_when_pgrep_unavailable() {
        let mut v = vec![entry("A", 10, Disposition::Deletable, Some("A"))];
        mark_running(&mut v, &MockRunner::new().with_not_found());
        assert!(!v[0].running);
    }

    #[test]
    fn split_running_moves_running_entries_to_skipped() {
        let mut r = entry("A", 10, Disposition::Deletable, Some("A"));
        r.running = true;
        let ok = entry("B", 10, Disposition::Deletable, None);
        let (to_delete, skipped) = split_running(vec![r, ok]);
        assert_eq!(to_delete.len(), 1);
        assert_eq!(to_delete[0].name, "B");
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].reason.contains("running"));
    }

    fn cleaner(tmp: &TempDir) -> LibraryCachesCleaner {
        LibraryCachesCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()))
    }

    #[test]
    fn name_is_library_caches() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(cleaner(&tmp).name(), "library-caches");
    }

    #[test]
    fn excluded_targets_are_registered_clean_targets() {
        // Correspondence table test: every cache dir in EXCLUDED_NAMES maps to
        // a registered clean target (single source of truth: the main.rs macro
        // table plus extra_targets). EXCLUDED_PREFIXES also carries the
        // "os-managed" marker, which is never dispatched and checked separately.
        let registered: std::collections::HashSet<&str> = crate::SUPPORTED_TARGETS
            .iter()
            .chain(crate::extra_targets())
            .map(|(n, _)| *n)
            .collect();
        for (_, target) in EXCLUDED_NAMES {
            assert!(
                registered.contains(target),
                "cache dir excluded name maps to unregistered target {target:?}"
            );
        }
        for (prefix, target) in EXCLUDED_PREFIXES {
            if *target == "os-managed" {
                continue;
            }
            assert!(
                registered.contains(target),
                "cache dir excluded prefix {prefix} maps to unregistered target {target:?}"
            );
        }
        assert_eq!(excluded_by("CocoaPods"), Some("cocoapods"));
    }

    #[test]
    fn detect_not_found_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(matches!(
            cleaner(&tmp).detect().status,
            ScanStatus::NotFound
        ));
    }

    #[test]
    fn detect_clean_when_only_unknown_apps() {
        let tmp = TempDir::new().unwrap();
        put(&caches(&tmp), "SomeUnknownApp", 50_000);
        assert!(matches!(cleaner(&tmp).detect().status, ScanStatus::Clean));
    }

    #[test]
    fn detect_sums_only_deletable_entries() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        put(&c, "gopls", 1_000);
        put(&c, "SomeUnknownApp", 90_000);
        put(&c, "Homebrew", 90_000);
        let mut cl = cleaner(&tmp);
        cl.threshold_override = Some(10_000);
        let expected = scan_caches(&c, false, Some(10_000))
            .iter()
            .filter(|e| e.disposition == Disposition::Deletable)
            .map(|e| e.size)
            .sum::<u64>();
        assert!(expected > 0);
        assert_eq!(cl.detect().bytes_for_test(), expected);
        assert_eq!(
            cl.detect().bytes_for_test(),
            scan_caches(&c, false, Some(10_000))[0].size
        );
    }

    #[test]
    fn detect_bytes_match_explore_metric() {
        // 数値突合: scan と explore は同じ format::dir_size metric で測る。
        // 20_480 bytes = 5 APFS pages, measured exactly.
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_480);
        let mut cl = cleaner(&tmp);
        cl.threshold_override = Some(10_000);
        assert_eq!(
            cl.detect().bytes_for_test(),
            crate::format::dir_size(&c.join("typescript")),
            "scan bytes must equal the explore metric on the same path"
        );
    }

    #[test]
    fn sub_targets_is_empty() {
        let tmp = TempDir::new().unwrap();
        put(&caches(&tmp), "typescript", 20_000);
        assert!(cleaner(&tmp).sub_targets().is_empty());
    }

    #[test]
    fn library_caches_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!cleaner(&tmp).auto_clean_allowed());
    }

    #[test]
    fn delete_entries_removes_and_sums_bytes() {
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        let mut cl = LibraryCachesCleaner::new(
            tmp.path(),
            Box::new(MockRunner::new().with_success("chflags")),
        );
        cl.threshold_override = Some(1);
        let entries = cl.scan(false);
        let size = entries[0].size;
        let r = cl.delete_entries(&entries, &crate::progress::DeepSuppressReporter);
        assert!(!c.join("typescript").exists());
        assert_eq!(r.bytes_freed, size);
        assert!(r.skipped.is_empty());
        assert!(r.uses_trash);
        crate::trash::set_trash_mode(true);
    }

    #[test]
    fn delete_entries_records_failures_as_skipped() {
        let _lock = crate::context::TEST_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let ghost = entry("ghost", 100, Disposition::Deletable, None);
        let mut ghost = ghost;
        ghost.path = tmp.path().join("does-not-exist");
        let cl = cleaner(&tmp);
        let r = cl.delete_entries(&[ghost], &crate::progress::DeepSuppressReporter);
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.exit_code(), 1);
        crate::trash::set_trash_mode(true);
    }

    #[test]
    fn clean_dry_run_deletes_nothing() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        put(&c, "SomeUnknownApp", 20_000);
        let mut cl = cleaner(&tmp);
        cl.threshold_override = Some(1);
        let r = cl
            .clean(true, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert!(c.join("typescript/data.bin").exists());
        assert!(c.join("SomeUnknownApp/data.bin").exists());
    }

    #[test]
    fn clean_without_dir_is_ok_and_empty() {
        let tmp = TempDir::new().unwrap();
        let r = cleaner(&tmp)
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.exit_code(), 0);
    }
}
