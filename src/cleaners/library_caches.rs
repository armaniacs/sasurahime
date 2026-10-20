use crate::hint::{apps::KNOWN_ENTRIES, BaseDir, HintEntry};
use std::path::PathBuf;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
