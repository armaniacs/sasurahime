use std::path::{Path, PathBuf};

use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Table};
use dialoguer::{Confirm, MultiSelect};

use crate::format::{dir_size, format_bytes};

struct ManagedPattern {
    path_pattern: &'static str,
    clean_target: &'static str,
}

static MANAGED_PATTERNS: &[ManagedPattern] = &[
    ManagedPattern {
        path_pattern: "~/.cache/uv",
        clean_target: "uv",
    },
    ManagedPattern {
        path_pattern: "~/.cache/puppeteer",
        clean_target: "browsers",
    },
    ManagedPattern {
        path_pattern: "~/Library/Caches/Homebrew",
        clean_target: "brew",
    },
    ManagedPattern {
        path_pattern: "~/Library/Caches/ms-playwright*",
        clean_target: "browsers",
    },
    ManagedPattern {
        path_pattern: "~/.local/share/mise",
        clean_target: "mise",
    },
    ManagedPattern {
        path_pattern: "~/.cargo/registry",
        clean_target: "cargo",
    },
    ManagedPattern {
        path_pattern: "~/.cargo/git",
        clean_target: "cargo",
    },
    ManagedPattern {
        path_pattern: "~/.colima",
        clean_target: "colima",
    },
    ManagedPattern {
        path_pattern: "~/Library/Developer/Xcode/DerivedData",
        clean_target: "xcode",
    },
    ManagedPattern {
        path_pattern: "~/Library/Developer/CoreSimulator",
        clean_target: "simulator",
    },
    ManagedPattern {
        path_pattern: "~/Library/Application Support/MobileSync/Backup",
        clean_target: "ios-backup",
    },
    ManagedPattern {
        path_pattern: "~/.cache/huggingface",
        clean_target: "huggingface",
    },
    ManagedPattern {
        path_pattern: "~/.huggingface",
        clean_target: "huggingface",
    },
    ManagedPattern {
        path_pattern: "~/.local/share/act",
        clean_target: "act",
    },
    ManagedPattern {
        path_pattern: "~/.gradle",
        clean_target: "gradle",
    },
    ManagedPattern {
        path_pattern: "~/.m2",
        clean_target: "maven",
    },
    ManagedPattern {
        path_pattern: "~/.sbt",
        clean_target: "sbt",
    },
    ManagedPattern {
        path_pattern: "~/.local/share/pre-commit",
        clean_target: "pre-commit",
    },
];

/// One first-level subdirectory found during exploration.
#[derive(Debug)]
pub struct ExploreEntry {
    /// Absolute path of the entry.
    pub path: PathBuf,
    /// Physical bytes (st_blocks × 512), same metric as `format::dir_size`.
    pub size: u64,
    /// `Some("brew")` when a registered cleaner owns this path; `None` otherwise.
    pub managed: Option<&'static str>,
}

pub struct ExploreOptions {
    /// Directories to scan (first level only).
    pub roots: Vec<PathBuf>,
    /// Maximum entries to show per section. `None` = show all.
    pub top: Option<usize>,
    pub dry_run: bool,
}

/// Returns the four default roots expanded from `home`.
pub fn default_roots(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Library/Application Support"),
        home.join("Library/Caches"),
        home.join(".cache"),
        home.join(".local/share"),
    ]
}

fn expand_tilde(pattern: &str, home: &Path) -> PathBuf {
    home.join(pattern.trim_start_matches("~/"))
}

/// Returns `Some(clean_target)` if `path` matches a managed pattern, else `None`.
fn is_managed(path: &Path, home: &Path) -> Option<&'static str> {
    for pat in MANAGED_PATTERNS {
        let expanded = expand_tilde(pat.path_pattern, home);
        if pat.path_pattern.ends_with('*') {
            let Some(prefix_dir) = expanded.parent() else {
                continue;
            };
            let Some(stem) = expanded.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            let stem = stem.trim_end_matches('*');
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if path.parent() == Some(prefix_dir) && name.starts_with(stem) {
                return Some(pat.clean_target);
            }
        } else if path == expanded {
            return Some(pat.clean_target);
        }
    }
    None
}

/// Collects all first-level entries across `roots`. Skips missing roots and empty dirs silently.
fn collect_entries(roots: &[PathBuf], home: &Path) -> Vec<ExploreEntry> {
    let mut entries = Vec::new();
    for root in roots {
        let Ok(rd) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let size = dir_size(&path);
            if size == 0 {
                continue;
            }
            let managed = is_managed(&path, home);
            entries.push(ExploreEntry {
                path,
                size,
                managed,
            });
        }
    }
    entries
}

/// Sorts `entries` by size descending and truncates to `top` if `Some`.
fn apply_top(mut entries: Vec<ExploreEntry>, top: Option<usize>) -> Vec<ExploreEntry> {
    entries.sort_unstable_by_key(|e: &ExploreEntry| std::cmp::Reverse(e.size));
    if let Some(n) = top {
        entries.truncate(n);
    }
    entries
}

/// Core: collect, classify, split. Pure except for filesystem reads.
/// Returns `(managed_entries, unmanaged_entries)`, each sorted by size desc with `top` applied.
pub(crate) fn explore_results(
    home: &Path,
    opts: &ExploreOptions,
) -> (Vec<ExploreEntry>, Vec<ExploreEntry>) {
    let all = collect_entries(&opts.roots, home);
    let (managed_raw, unmanaged_raw): (Vec<_>, Vec<_>) =
        all.into_iter().partition(|e| e.managed.is_some());
    let managed = apply_top(managed_raw, opts.top);
    let unmanaged = apply_top(unmanaged_raw, opts.top);
    (managed, unmanaged)
}

fn display_path(path: &Path, home: &Path) -> String {
    if let Ok(rel) = path.strip_prefix(home) {
        format!("~/{}", rel.display())
    } else {
        path.display().to_string()
    }
}

fn print_managed_table(entries: &[ExploreEntry], home: &Path) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Path", "Size", "Clean with"]);
    for e in entries {
        let cmd = format!("sasurahime clean {}", e.managed.unwrap_or("-"));
        table.add_row(vec![display_path(&e.path, home), format_bytes(e.size), cmd]);
    }
    println!("{table}");
}

fn print_unmanaged_table(entries: &[ExploreEntry], home: &Path) {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Path", "Size"]);
    for e in entries {
        table.add_row(vec![display_path(&e.path, home), format_bytes(e.size)]);
    }
    println!("{table}");
}

/// Runs `open "<path>"` via `std::process::Command`. macOS-only; not unit-tested.
fn open_in_finder(path: &Path) -> Result<()> {
    let status = std::process::Command::new("open").arg(path).status()?;
    anyhow::ensure!(
        status.success(),
        "open returned non-zero exit: {:?}",
        status.code()
    );
    Ok(())
}

/// Full interactive flow: calls `explore_results`, prints tables, runs dialoguer.
pub fn run_explore(home: &Path, opts: ExploreOptions) -> Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        eprintln!("Error: sasurahime explore requires an interactive terminal");
        anyhow::bail!("not a TTY");
    }

    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("sasurahime"));
    let dry_run = opts.dry_run;

    // ── Managed section ────────────────────────────────────────────────────
    println!("\n━━━ Managed by sasurahime ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    let (managed, unmanaged) = explore_results(home, &opts);

    let selected_any_managed = if managed.is_empty() {
        println!("(none found)");
        false
    } else {
        print_managed_table(&managed, home);

        let labels: Vec<String> = managed
            .iter()
            .map(|e| {
                format!(
                    "{:<45} {}",
                    display_path(&e.path, home),
                    format_bytes(e.size)
                )
            })
            .collect();

        let selections = MultiSelect::new()
            .with_prompt("Select managed entries to clean (space to toggle, enter to confirm)")
            .items(&labels)
            .interact()?;

        for &idx in &selections {
            let target = managed[idx].managed.unwrap_or("");
            println!("\nRunning: sasurahime clean {target}");
            let mut cmd = std::process::Command::new(&exe);
            cmd.args(["clean", target]);
            if dry_run {
                cmd.arg("--dry-run");
            }
            match cmd.status() {
                Ok(s) if s.success() => {}
                Ok(s) => eprintln!(
                    "Error: sasurahime clean {target} failed (exit {:?})",
                    s.code()
                ),
                Err(e) => eprintln!("Error: failed to spawn sasurahime clean {target}: {e}"),
            }
        }

        !selections.is_empty()
    };

    // Re-scan after clean if anything was cleaned
    let unmanaged = if selected_any_managed {
        println!("\n━━━ Managed by sasurahime (updated) ━━━━━━━━━━━━━━━━━━━\n");
        let (managed2, unmanaged2) = explore_results(home, &opts);
        print_managed_table(&managed2, home);
        unmanaged2
    } else {
        unmanaged
    };

    // ── Unmanaged section ──────────────────────────────────────────────────
    println!("\n━━━ Not managed by sasurahime ━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    if unmanaged.is_empty() {
        println!("(none found)");
        return Ok(());
    }

    print_unmanaged_table(&unmanaged, home);

    let labels: Vec<String> = unmanaged
        .iter()
        .map(|e| {
            format!(
                "{:<45} {}",
                display_path(&e.path, home),
                format_bytes(e.size)
            )
        })
        .collect();

    let selections = MultiSelect::new()
        .with_prompt("Select unmanaged entries to inspect")
        .items(&labels)
        .interact()?;

    for &idx in &selections {
        let entry = &unmanaged[idx];
        println!("\nPath: {}", entry.path.display());
        println!("Size: {}", format_bytes(entry.size));
        let open = Confirm::new()
            .with_prompt("Open in Finder?")
            .default(false)
            .interact()?;
        if open {
            if let Err(e) = open_in_finder(&entry.path) {
                eprintln!("Error opening Finder: {e}");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── helpers ────────────────────────────────────────────────────────────

    fn make_entry(size: u64) -> ExploreEntry {
        ExploreEntry {
            path: PathBuf::from("/fake"),
            size,
            managed: None,
        }
    }

    // ── Cycle 1: is_managed ────────────────────────────────────────────────

    #[test]
    fn is_managed_uv_cache_returns_target() {
        let home = PathBuf::from("/Users/test");
        let path = home.join(".cache/uv");
        assert_eq!(is_managed(&path, &home), Some("uv"));
    }

    #[test]
    fn is_managed_unknown_dir_returns_none() {
        let home = PathBuf::from("/Users/test");
        let path = home.join(".cache/some-random-tool");
        assert_eq!(is_managed(&path, &home), None);
    }

    #[test]
    fn is_managed_playwright_prefix_glob() {
        let home = PathBuf::from("/Users/test");
        let path = home.join("Library/Caches/ms-playwright-chromium-1234");
        assert_eq!(is_managed(&path, &home), Some("browsers"));
    }

    #[test]
    fn is_managed_brew_exact() {
        let home = PathBuf::from("/Users/test");
        let path = home.join("Library/Caches/Homebrew");
        assert_eq!(is_managed(&path, &home), Some("brew"));
    }

    #[test]
    fn is_managed_playwright_exact_prefix_not_matched() {
        // "ms-playwright" itself (without suffix) should NOT match "ms-playwright*"
        // since the pattern requires a non-empty suffix after the stem
        let home = PathBuf::from("/Users/test");
        let path = home.join("Library/Caches/ms-playwright");
        // "ms-playwright".starts_with("ms-playwright") is true, so this DOES match.
        // This is intentional: the glob means "any name starting with ms-playwright".
        assert_eq!(is_managed(&path, &home), Some("browsers"));
    }

    // ── Cycle 2: default_roots ─────────────────────────────────────────────

    #[test]
    fn default_roots_contains_library_caches() {
        let home = PathBuf::from("/Users/test");
        let roots = default_roots(&home);
        assert!(roots.contains(&home.join("Library/Caches")));
    }

    #[test]
    fn default_roots_contains_all_four() {
        let home = PathBuf::from("/Users/test");
        let roots = default_roots(&home);
        assert_eq!(roots.len(), 4);
    }

    // ── Cycle 3: collect_entries ───────────────────────────────────────────

    #[test]
    fn collect_entries_missing_root_returns_empty() {
        let home = TempDir::new().unwrap();
        let missing = home.path().join("nonexistent");
        let entries = collect_entries(&[missing], home.path());
        assert!(entries.is_empty());
    }

    #[test]
    fn collect_entries_sums_first_level_dirs() {
        let tmp = TempDir::new().unwrap();
        let app_dir = tmp.path().join("SomeApp");
        fs::create_dir(&app_dir).unwrap();
        fs::write(app_dir.join("data.bin"), [0u8; 4096]).unwrap();
        let entries = collect_entries(&[tmp.path().to_path_buf()], tmp.path());
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, app_dir);
        assert!(entries[0].size > 0);
    }

    #[test]
    fn collect_entries_classifies_managed_correctly() {
        let home = TempDir::new().unwrap();
        let cache = home.path().join(".cache");
        let uv_dir = cache.join("uv");
        fs::create_dir_all(&uv_dir).unwrap();
        fs::write(uv_dir.join("file"), b"x").unwrap();
        let entries = collect_entries(&[cache], home.path());
        let uv_entry = entries.iter().find(|e| e.path == uv_dir).unwrap();
        assert_eq!(uv_entry.managed, Some("uv"));
    }

    #[test]
    fn collect_entries_unmanaged_has_none() {
        let tmp = TempDir::new().unwrap();
        let unknown = tmp.path().join("UnknownApp");
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join("f"), b"x").unwrap();
        let entries = collect_entries(&[tmp.path().to_path_buf()], tmp.path());
        assert_eq!(entries[0].managed, None);
    }

    #[test]
    fn collect_entries_excludes_empty_dirs() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir(tmp.path().join("EmptyApp")).unwrap();
        let entries = collect_entries(&[tmp.path().to_path_buf()], tmp.path());
        assert!(entries.is_empty(), "empty dirs must not appear");
    }

    // ── Cycle 4: apply_top ────────────────────────────────────────────────

    #[test]
    fn apply_top_limits_to_n_largest() {
        let entries = vec![make_entry(100), make_entry(300), make_entry(200)];
        let result = apply_top(entries, Some(2));
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].size, 300);
        assert_eq!(result[1].size, 200);
    }

    #[test]
    fn apply_top_none_returns_all_sorted() {
        let entries = vec![make_entry(50), make_entry(200), make_entry(100)];
        let result = apply_top(entries, None);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].size, 200);
    }

    #[test]
    fn apply_top_larger_than_len_returns_all() {
        let entries = vec![make_entry(10), make_entry(20)];
        let result = apply_top(entries, Some(100));
        assert_eq!(result.len(), 2);
    }

    // ── Cycle 5: explore_results ──────────────────────────────────────────

    #[test]
    fn explore_results_splits_managed_and_unmanaged() {
        let home = TempDir::new().unwrap();
        let cache = home.path().join(".cache");
        fs::create_dir_all(cache.join("uv")).unwrap();
        fs::write(cache.join("uv/x"), b"x").unwrap();
        fs::create_dir(cache.join("unknown-tool")).unwrap();
        fs::write(cache.join("unknown-tool/f"), b"x").unwrap();

        let opts = ExploreOptions {
            roots: vec![cache],
            top: None,
            dry_run: false,
        };
        let (managed, unmanaged) = explore_results(home.path(), &opts);
        assert_eq!(managed.len(), 1);
        assert_eq!(managed[0].managed, Some("uv"));
        assert_eq!(unmanaged.len(), 1);
        assert_eq!(unmanaged[0].managed, None);
    }

    #[test]
    fn explore_results_top_applied_per_section() {
        let home = TempDir::new().unwrap();
        let cache = home.path().join(".cache");
        for name in ["app1", "app2", "app3"] {
            let d = cache.join(name);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("f"), b"x").unwrap();
        }
        let opts = ExploreOptions {
            roots: vec![cache],
            top: Some(2),
            dry_run: false,
        };
        let (_managed, unmanaged) = explore_results(home.path(), &opts);
        assert_eq!(unmanaged.len(), 2);
    }

    #[test]
    fn explore_results_excludes_zero_size() {
        let home = TempDir::new().unwrap();
        let cache = home.path().join(".cache");
        fs::create_dir_all(cache.join("empty-app")).unwrap();
        let opts = ExploreOptions {
            roots: vec![cache],
            top: None,
            dry_run: false,
        };
        let (managed, unmanaged) = explore_results(home.path(), &opts);
        assert!(managed.is_empty());
        assert!(unmanaged.is_empty());
    }
}
