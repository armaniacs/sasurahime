use crate::format::dir_size;
use crate::hint::apps::KNOWN_ENTRIES;
use crate::hint::{BaseDir, HintEntry};
use std::fs;
use std::path::{Component, Path, PathBuf};

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
    if !target.subpaths.iter().any(|s| *s == sub) {
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
                sub: *sub,
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
