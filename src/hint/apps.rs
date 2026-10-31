use super::{BaseDir, HintEntry};

const MB: u64 = 1024 * 1024;

/// Known application cache directories that can be suggested for manual cleanup.
pub static KNOWN_ENTRIES: &[HintEntry] = &[
    // ── ~/Library/Caches/ ────────────────────────────────────────────────────
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["Microsoft Edge"],
        display_name: "Microsoft Edge",
        process_name: Some("Microsoft Edge Helper"),
        commands: &["rm -rf ~/Library/Caches/Microsoft\\ Edge"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Microsoft Edge"),
        relaunch_app: Some("Microsoft Edge"),
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["com.microsoft.VSCode.ShipIt"],
        display_name: "VSCode ShipIt cache",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/com.microsoft.VSCode.ShipIt"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["ms-playwright"],
        display_name: "Playwright (Node)",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/ms-playwright"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["ms-playwright-go"],
        display_name: "Playwright (Go)",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/ms-playwright-go"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["electron"],
        display_name: "Electron",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/electron"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["BraveSoftware"],
        display_name: "Brave Browser",
        process_name: Some("Brave Browser"),
        commands: &["rm -rf ~/Library/Caches/BraveSoftware"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Brave Browser"),
        relaunch_app: Some("Brave Browser"),
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["typescript"],
        display_name: "TypeScript server cache",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/typescript"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["gopls"],
        display_name: "gopls cache",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/gopls"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["ort.pyke.io"],
        display_name: "ONNX Runtime cache",
        process_name: None,
        commands: &["rm -rf ~/Library/Caches/ort.pyke.io"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["GeoServices"],
        display_name: "GeoServices",
        process_name: Some("locationd"),
        commands: &[],
        threshold_bytes: 64 * MB,
        skip: true, // OS-managed
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["Homebrew"],
        display_name: "Homebrew",
        process_name: None,
        commands: &["brew cleanup -s --prune=all"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    // ── ~/Library/Application Support/ ───────────────────────────────────────
    HintEntry {
        base_dir: BaseDir::AppSupport,
        path_suffixes: &["Slack/Cache"],
        display_name: "Slack",
        process_name: Some("Slack"),
        commands: &["rm -rf ~/Library/Application\\ Support/Slack/Cache"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Slack"),
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::AppSupport,
        path_suffixes: &["Claude/Cache"],
        display_name: "Claude (desktop)",
        process_name: Some("Claude"),
        commands: &["rm -rf ~/Library/Application\\ Support/Claude/Cache"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Claude"),
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::AppSupport,
        path_suffixes: &["obsidian/Cache"],
        display_name: "Obsidian",
        process_name: Some("Obsidian"),
        commands: &["rm -rf ~/Library/Application\\ Support/obsidian/Cache"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Obsidian"),
        relaunch_app: Some("Obsidian"),
    },
    HintEntry {
        base_dir: BaseDir::AppSupport,
        path_suffixes: &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
        display_name: "VSCode caches",
        process_name: Some("Code"),
        commands: &[
            "rm -rf ~/Library/Application\\ Support/Code/Cache",
            "rm -rf ~/Library/Application\\ Support/Code/CachedExtensionVSIXs",
            "rm -rf ~/Library/Application\\ Support/Code/CachedData",
        ],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Visual Studio Code"),
        relaunch_app: Some("Visual Studio Code"),
    },
    HintEntry {
        base_dir: BaseDir::AppSupport,
        path_suffixes: &["Google/Chrome/Default/Cache_Data"],
        display_name: "Google Chrome",
        process_name: Some("Google Chrome"),
        commands: &["rm -rf ~/Library/Application\\ Support/Google/Chrome/Default/Cache_Data"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Google Chrome"),
        relaunch_app: Some("Google Chrome"),
    },
    // ── ~/Library/Logs/ ───────────────────────────────────────────────────────
    HintEntry {
        base_dir: BaseDir::Logs,
        path_suffixes: &["Claude"],
        display_name: "Claude logs",
        process_name: Some("Claude"),
        commands: &["rm -rf ~/Library/Logs/Claude"],
        threshold_bytes: MB,
        skip: false,
        quit_app: Some("Claude"),
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Logs,
        path_suffixes: &["zoom.us"],
        display_name: "Zoom logs",
        process_name: Some("zoom.us"),
        commands: &["rm -rf ~/Library/Logs/zoom.us"],
        threshold_bytes: MB,
        skip: false,
        quit_app: Some("zoom.us"),
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Logs,
        path_suffixes: &["DiagnosticReports"],
        display_name: "Diagnostic Reports",
        process_name: None,
        commands: &["rm -rf ~/Library/Logs/DiagnosticReports"],
        threshold_bytes: MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Logs,
        path_suffixes: &["LM Studio"],
        display_name: "LM Studio logs",
        process_name: Some("LM Studio"),
        commands: &["rm -rf ~/Library/Logs/LM\\ Studio"],
        threshold_bytes: MB,
        skip: false,
        quit_app: Some("LM Studio"),
        relaunch_app: Some("LM Studio"),
    },
    HintEntry {
        base_dir: BaseDir::Logs,
        path_suffixes: &["CrashReporter"],
        display_name: "Crash Reporter logs",
        process_name: None,
        commands: &["rm -rf ~/Library/Logs/CrashReporter"],
        threshold_bytes: MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
    HintEntry {
        base_dir: BaseDir::Logs,
        path_suffixes: &["fsck_hfs.log"],
        display_name: "fsck_hfs log",
        process_name: None,
        commands: &["rm -f ~/Library/Logs/fsck_hfs.log"],
        threshold_bytes: MB,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    },
];

/// The paths the hint cleaner owns: non-skip KNOWN_ENTRIES suffixes paired
/// with their base dir. One owner for the "who else claims this dir"
/// question, shared by the app-support / library-caches candidate tags and
/// the cross-reference tests. Exact-match membership only -- prefixes must
/// never match (Default/Cache vs Default/Cache_Data).
pub fn hint_owned_paths() -> Vec<(BaseDir, &'static str)> {
    KNOWN_ENTRIES
        .iter()
        .filter(|e| !e.skip)
        .flat_map(|e| e.path_suffixes.iter().map(move |s| (e.base_dir, *s)))
        .collect()
}

/// Render tag for a cleaner candidate that the hint cleaner also claims.
pub const MANAGED_BY_HINTS_TAG: &str = "  [managed by hints]";

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_owned_paths_expands_the_multi_suffix_code_entry() {
        let owned = hint_owned_paths();
        for suffix in ["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"] {
            assert!(
                owned.contains(&(BaseDir::AppSupport, suffix)),
                "the VSCode multi-suffix entry must expand every suffix, missing: {suffix}"
            );
        }
    }

    #[test]
    fn hint_owned_paths_distinguishes_chrome_cache_from_cache_data() {
        // Prefix-bomb defense pin: `Default/Cache_Data` is hint-owned while
        // `Default/Cache` is not in KNOWN_ENTRIES — membership must be
        // exact-match only, never a prefix match.
        let owned = hint_owned_paths();
        assert!(
            owned.contains(&(BaseDir::AppSupport, "Google/Chrome/Default/Cache_Data")),
            "Cache_Data is the hint-owned Chrome path"
        );
        assert!(
            !owned.contains(&(BaseDir::AppSupport, "Google/Chrome/Default/Cache")),
            "Default/Cache is not in KNOWN_ENTRIES and must never match by prefix"
        );
    }

    #[test]
    fn hint_owned_paths_excludes_skip_entries() {
        let owned = hint_owned_paths();
        assert!(
            !owned.iter().any(|(_, s)| *s == "GeoServices"),
            "the GeoServices skip entry must not be hint-owned"
        );
    }

    #[test]
    fn hint_owned_paths_includes_homebrew_and_ms_playwright() {
        // library-caches delegates these names, but the hint cleaner owns them.
        let owned = hint_owned_paths();
        assert!(owned.contains(&(BaseDir::Caches, "Homebrew")));
        assert!(owned.contains(&(BaseDir::Caches, "ms-playwright")));
    }

    #[test]
    fn managed_by_hints_tag_wording() {
        assert_eq!(MANAGED_BY_HINTS_TAG, "  [managed by hints]");
    }
}
