use crate::format::dir_size;
use crate::hint::{apps::KNOWN_ENTRIES, BaseDir, HintEntry};
use std::fs;
use std::path::{Path, PathBuf};

pub const EXCLUDED_NAMES: &[(&str, &str)] = &[
    ("Homebrew", "brew"),
    ("node-gyp", "node-gyp"),
    ("org.swift.swiftpm", "spm"),
    ("JetBrains", "jetbrains"),
    ("CocoaPods", "cocoa-pods"),
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
        let size = dir_size(&path);
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
}
