use crate::hint::apps::KNOWN_ENTRIES;
use crate::hint::{BaseDir, HintEntry};
use std::path::Path;

pub const DEFAULT_MIN_APP_BYTES: u64 = 64 * 1024 * 1024;

/// One application whose cache subdirectories may be removed.
/// Only paths listed in `subpaths` are ever deletable; everything else under
/// `root` (settings, logins, IndexedDB, Local Storage, workspaces) is user data.
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
}
