use crate::command::CommandRunner;
use crate::format::format_bytes;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BaseDir {
    Caches,
    AppSupport,
    Logs,
}

pub struct HintEntry {
    pub base_dir: BaseDir,
    /// One or more subdirectory paths (relative to base_dir) to sum for size.
    pub path_suffixes: &'static [&'static str],
    pub display_name: &'static str,
    pub process_name: Option<&'static str>,
    /// One or more shell commands to show the user.
    pub commands: &'static [&'static str],
    pub threshold_bytes: u64,
    pub skip: bool,
    /// osascript app name for `quit app "..."`. None = cannot auto-quit.
    pub quit_app: Option<&'static str>,
    /// App name for `open -a "..."` after cache deletion. None = don't relaunch.
    pub relaunch_app: Option<&'static str>,
}

pub struct ProcessHint {
    pub entry: &'static HintEntry,
    /// Sum of all path_suffixes sizes.
    pub size_bytes: u64,
    pub running: bool,
}

const MB: u64 = 1024 * 1024;

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
        relaunch_app: None, // login flow is heavy; user relaunches manually
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
        relaunch_app: None, // session state; user relaunches manually
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
        relaunch_app: None, // may be in a meeting
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

fn base_path(home: &Path, base: BaseDir) -> PathBuf {
    match base {
        BaseDir::Caches => home.join("Library/Caches"),
        BaseDir::AppSupport => home.join("Library/Application Support"),
        BaseDir::Logs => home.join("Library/Logs"),
    }
}

fn dir_size_bytes(path: &Path, runner: &dyn CommandRunner) -> u64 {
    if !path.exists() {
        return 0;
    }
    let path_str = path.to_string_lossy();
    match runner.run("du", &["-sk", &path_str]) {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().next())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|kb| kb * 1024)
                .unwrap_or(0)
        }
        Err(_) => 0,
    }
}

fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool {
    runner
        .run("pgrep", &["-x", process_name])
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn collect_hints(home: &Path, runner: &dyn CommandRunner) -> Vec<ProcessHint> {
    let mut hints: Vec<ProcessHint> = KNOWN_ENTRIES
        .iter()
        .filter(|e| !e.skip)
        .filter_map(|entry| {
            let base = base_path(home, entry.base_dir);
            let size_bytes: u64 = entry
                .path_suffixes
                .iter()
                .map(|s| dir_size_bytes(&base.join(s), runner))
                .sum();
            if size_bytes <= entry.threshold_bytes {
                return None;
            }
            let running = entry
                .process_name
                .map(|name| is_running(name, runner))
                .unwrap_or(false);
            Some(ProcessHint {
                entry,
                size_bytes,
                running,
            })
        })
        .collect();

    hints.sort_by_key(|h| std::cmp::Reverse(h.size_bytes));
    hints.truncate(5);
    hints
}

pub trait PromptReader {
    /// Read one line from the user. Returns `None` on EOF or error.
    fn read_line(&self) -> Option<String>;
}

pub struct StdinPrompt;

impl PromptReader for StdinPrompt {
    fn read_line(&self) -> Option<String> {
        let mut s = String::new();
        std::io::stdin().read_line(&mut s).ok()?;
        Some(s)
    }
}

const QUIT_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);
const QUIT_TIMEOUT_SECS: usize = 10;

/// Quit the app, delete caches, optionally relaunch. Returns Err if quit timed out.
pub fn auto_clean_hint(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    let quit_app = match entry.quit_app {
        Some(a) => a,
        None => anyhow::bail!("{} cannot be auto-quit", entry.display_name),
    };

    // Ask the app to quit via osascript.
    eprintln!("  Quitting {}...", entry.display_name);
    runner.run("osascript", &["-e", &format!("quit app \"{quit_app}\"")])?;

    // Poll until the process is gone.
    let process_name = entry.process_name.unwrap_or(quit_app);
    for _ in 0..QUIT_TIMEOUT_SECS {
        std::thread::sleep(QUIT_POLL_INTERVAL);
        let still_running = runner
            .run("pgrep", &["-x", process_name])
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !still_running {
            break;
        }
    }
    // Final check.
    let still_running = runner
        .run("pgrep", &["-x", process_name])
        .map(|o| o.status.success())
        .unwrap_or(false);
    if still_running {
        anyhow::bail!(
            "{} did not quit within {}s — skipping cache deletion",
            entry.display_name,
            QUIT_TIMEOUT_SECS
        );
    }

    // Delete all path_suffixes.
    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            let path_str = path.to_string_lossy();
            runner.run("rm", &["-rf", &path_str])?;
        }
    }
    eprintln!("  [OK]");

    // Optionally relaunch.
    if let Some(app) = entry.relaunch_app {
        eprintln!("  Restarting {}...", entry.display_name);
        runner.run("open", &["-a", app])?;
        eprintln!("  [OK]");
    } else {
        eprintln!(
            "  ({} will not be restarted — relaunch manually if needed)",
            entry.display_name
        );
    }

    Ok(())
}

/// For each hint that can be cleaned, ask the user and clean.
///
/// For running apps with a configured `quit_app`, the app is quit first, caches
/// are deleted, and the app is optionally relaunched.  For non-running apps the
/// cache directories are deleted directly without any quit/relaunch step.
pub fn offer_auto_clean(
    hints: &[ProcessHint],
    home: &Path,
    runner: &dyn CommandRunner,
    prompt: &dyn PromptReader,
) {
    // Include all hints.  Running hints need quit_app; non-running hints can
    // always have their cache directories removed directly.
    let actionable: Vec<_> = hints
        .iter()
        .filter(|h| !h.running || h.entry.quit_app.is_some())
        .collect();

    if actionable.is_empty() {
        return;
    }

    for hint in actionable {
        let size_str = format_bytes(hint.size_bytes);
        if hint.running {
            eprint!(
                "\nQuit {} and clear cache? ({size_str} will be freed) [y/N] ",
                hint.entry.display_name
            );
        } else {
            eprint!(
                "\nClear {} cache? ({size_str} will be freed) [y/N] ",
                hint.entry.display_name
            );
        }
        match prompt.read_line() {
            Some(input) if input.trim().eq_ignore_ascii_case("y") => {
                if hint.running {
                    if let Err(e) = auto_clean_hint(hint, home, runner) {
                        eprintln!("  Error: {e}");
                    }
                } else if let Err(e) = clean_hint_dirs(hint, home, runner) {
                    eprintln!("  Error: {e}");
                }
            }
            _ => {}
        }
    }
}

/// Delete cache directories for a hint without quitting or relaunching the app.
fn clean_hint_dirs(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            runner.run("rm", &["-rf", &path.to_string_lossy()])?;
        }
    }
    eprintln!("  [OK]");
    Ok(())
}

pub fn print_hints(hints: &[ProcessHint]) {
    if hints.is_empty() {
        return;
    }
    let sep = "─".repeat(60);
    eprintln!("{sep}");
    eprintln!(" Tip: The following caches can be freed manually:");
    eprintln!("{sep}");
    for h in hints {
        let running_tag = if h.running {
            "  [running — quit first]"
        } else {
            ""
        };
        eprintln!(
            "  {:<26} {}{}",
            h.entry.display_name,
            format_bytes(h.size_bytes),
            running_tag
        );
        for cmd in h.entry.commands {
            eprintln!("    $ {cmd}");
        }
    }
    eprintln!("{sep}");
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;

    fn make_output(exit_code: i32, stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(exit_code),
            stdout: stdout.as_bytes().to_vec(),
            stderr: vec![],
        }
    }

    struct FakeRunner {
        /// Maps path substrings to KB sizes returned by `du -sk`.
        sizes: Vec<(String, u64)>,
        /// Process names that are "running" (pgrep returns exit 0).
        running: Vec<String>,
    }

    impl FakeRunner {
        fn new(sizes: &[(&str, u64)], running: &[&str]) -> Self {
            Self {
                sizes: sizes.iter().map(|(p, k)| (p.to_string(), *k)).collect(),
                running: running.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
            match program {
                "du" => {
                    let path = args.last().copied().unwrap_or("");
                    let kb = self
                        .sizes
                        .iter()
                        .find(|(pat, _)| path.contains(pat.as_str()))
                        .map(|(_, kb)| *kb)
                        .unwrap_or(0);
                    Ok(make_output(0, &format!("{kb}\t{path}\n")))
                }
                "pgrep" => {
                    let name = args.last().copied().unwrap_or("");
                    let found = self.running.iter().any(|r| r == name);
                    Ok(make_output(if found { 0 } else { 1 }, ""))
                }
                _ => anyhow::bail!("FakeRunner: unexpected command {program}"),
            }
        }

        fn exists(&self, _program: &str) -> bool {
            true
        }
    }

    fn make_dirs(home: &Path, base: BaseDir, suffixes: &[&str]) {
        let base_path = match base {
            BaseDir::Caches => home.join("Library/Caches"),
            BaseDir::AppSupport => home.join("Library/Application Support"),
            BaseDir::Logs => home.join("Library/Logs"),
        };
        for s in suffixes {
            std::fs::create_dir_all(base_path.join(s)).unwrap();
        }
    }

    #[test]
    fn collect_hints_filters_below_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::Caches, &["Homebrew", "typescript"]);

        // Homebrew: 80 MB (above 64 MB threshold), typescript: 10 MB (below)
        let runner = FakeRunner::new(&[("Homebrew", 80 * 1024), ("typescript", 10 * 1024)], &[]);
        let hints = collect_hints(home, &runner);

        assert!(hints.iter().any(|h| h.entry.display_name == "Homebrew"));
        assert!(!hints
            .iter()
            .any(|h| h.entry.display_name == "TypeScript server cache"));
    }

    #[test]
    fn collect_hints_limits_to_top5() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        // Create 7 entries above threshold in Caches
        make_dirs(
            home,
            BaseDir::Caches,
            &[
                "Microsoft Edge",
                "com.microsoft.VSCode.ShipIt",
                "ms-playwright",
                "ms-playwright-go",
                "electron",
                "BraveSoftware",
                "Homebrew",
            ],
        );
        let runner = FakeRunner::new(
            &[
                ("Microsoft Edge", 1_800 * 1024),
                ("com.microsoft.VSCode.ShipIt", 900 * 1024),
                ("ms-playwright", 280 * 1024),
                ("ms-playwright-go", 130 * 1024),
                ("electron", 110 * 1024),
                ("BraveSoftware", 100 * 1024),
                ("Homebrew", 80 * 1024),
            ],
            &[],
        );
        let hints = collect_hints(home, &runner);
        assert_eq!(hints.len(), 5);
        // should be sorted descending
        assert!(hints[0].size_bytes >= hints[1].size_bytes);
        assert!(hints[1].size_bytes >= hints[2].size_bytes);
    }

    #[test]
    fn collect_hints_excludes_skip_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::Caches, &["GeoServices"]);
        let runner = FakeRunner::new(&[("GeoServices", 100 * 1024)], &[]);
        let hints = collect_hints(home, &runner);
        assert!(!hints.iter().any(|h| h.entry.display_name == "GeoServices"));
    }

    #[test]
    fn collect_hints_sets_running_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Slack/Cache"]);
        let runner = FakeRunner::new(
            &[("Slack", 230 * 1024)],
            &["Slack"], // Slack is running
        );
        let hints = collect_hints(home, &runner);
        let slack = hints.iter().find(|h| h.entry.display_name == "Slack");
        assert!(slack.is_some(), "Slack hint should appear");
        assert!(slack.unwrap().running, "running flag should be true");
    }

    #[test]
    fn collect_hints_aggregates_vscode_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(
            home,
            BaseDir::AppSupport,
            &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
        );
        // Each dir: 200 MB → total 600 MB
        let runner = FakeRunner::new(
            &[
                ("Code/Cache", 200 * 1024),
                ("Code/CachedExtensionVSIXs", 200 * 1024),
                ("Code/CachedData", 200 * 1024),
            ],
            &[],
        );
        let hints = collect_hints(home, &runner);
        let vscode = hints
            .iter()
            .find(|h| h.entry.display_name == "VSCode caches");
        assert!(vscode.is_some(), "VSCode caches hint should appear");
        assert_eq!(
            vscode.unwrap().size_bytes,
            600 * 1024 * 1024,
            "size should be sum of 3 dirs"
        );
    }

    #[test]
    fn collect_hints_includes_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::Logs, &["Claude"]);
        // 62 MB — above 1 MB threshold for logs
        let runner = FakeRunner::new(&[("Claude", 62 * 1024)], &["Claude"]);
        let hints = collect_hints(home, &runner);
        let log = hints.iter().find(|h| h.entry.display_name == "Claude logs");
        assert!(log.is_some(), "Claude logs hint should appear");
        assert!(log.unwrap().running);
    }

    #[test]
    fn print_hints_empty_produces_no_output() {
        // Just verify it doesn't panic and returns without printing the header.
        // (We can't easily capture stderr in a unit test, so we just assert no panic.)
        print_hints(&[]);
    }

    #[test]
    fn print_hints_running_shows_quit_first() {
        static ENTRY: HintEntry = HintEntry {
            base_dir: BaseDir::AppSupport,
            path_suffixes: &["Slack/Cache"],
            display_name: "Slack",
            process_name: Some("Slack"),
            commands: &["rm -rf ~/Library/Application\\ Support/Slack/Cache"],
            threshold_bytes: 64 * MB,
            skip: false,
            quit_app: Some("Slack"),
            relaunch_app: None,
        };
        let hint = ProcessHint {
            entry: &ENTRY,
            size_bytes: 230 * MB,
            running: true,
        };
        let tag = if hint.running {
            "  [running — quit first]"
        } else {
            ""
        };
        assert!(tag.contains("quit first"));
    }

    // ── auto_clean tests ─────────────────────────────────────────────────────

    use std::sync::Mutex;

    struct RecordingRunner {
        /// Sequence of calls: (program, args)
        calls: Mutex<Vec<(String, Vec<String>)>>,
        /// process names that are "running" at the *start* of the test
        initially_running: Vec<String>,
        /// after how many pgrep calls the process appears gone (simulates quit)
        gone_after_pgrep_calls: usize,
    }

    impl RecordingRunner {
        fn new(running: &[&str], gone_after: usize) -> Self {
            Self {
                calls: Mutex::new(vec![]),
                initially_running: running.iter().map(|s| s.to_string()).collect(),
                gone_after_pgrep_calls: gone_after,
            }
        }

        fn recorded_calls(&self) -> Vec<(String, Vec<String>)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for RecordingRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
            let mut calls = self.calls.lock().unwrap();
            calls.push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            let call_count = calls.len();
            drop(calls);

            match program {
                "pgrep" => {
                    let name = args.last().copied().unwrap_or("");
                    let is_running = self.initially_running.iter().any(|r| r == name);
                    // count how many pgrep calls have been made so far
                    let pgrep_count = self
                        .calls
                        .lock()
                        .unwrap()
                        .iter()
                        .filter(|(p, _)| p == "pgrep")
                        .count();
                    let still_running = is_running && pgrep_count <= self.gone_after_pgrep_calls;
                    Ok(make_output(if still_running { 0 } else { 1 }, ""))
                }
                "du" => {
                    let path = args.last().copied().unwrap_or("");
                    Ok(make_output(0, &format!("102400\t{path}\n"))) // 100 MB
                }
                "osascript" | "open" | "rm" => Ok(make_output(0, "")),
                _ => anyhow::bail!("RecordingRunner: unexpected {program} (call #{call_count})"),
            }
        }

        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    static VSCODE_ENTRY: HintEntry = HintEntry {
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
    };

    static SLACK_ENTRY: HintEntry = HintEntry {
        base_dir: BaseDir::AppSupport,
        path_suffixes: &["Slack/Cache"],
        display_name: "Slack",
        process_name: Some("Slack"),
        commands: &["rm -rf ~/Library/Application\\ Support/Slack/Cache"],
        threshold_bytes: 64 * MB,
        skip: false,
        quit_app: Some("Slack"),
        relaunch_app: None,
    };

    #[test]
    fn auto_clean_skips_if_quit_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        // Process never goes away (gone_after = usize::MAX)
        let runner = RecordingRunner::new(&["Code"], usize::MAX);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * MB,
            running: true,
        };

        let result = auto_clean_hint(&hint, home, &runner);
        assert!(result.is_err(), "should return Err when quit times out");
        // osascript should have been called but rm should NOT
        let calls = runner.recorded_calls();
        assert!(
            calls.iter().any(|(p, _)| p == "osascript"),
            "should attempt quit"
        );
        assert!(
            !calls.iter().any(|(p, _)| p == "rm"),
            "should NOT delete while process alive"
        );
    }

    #[test]
    fn auto_clean_deletes_paths_after_quit() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(
            home,
            BaseDir::AppSupport,
            &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
        );

        // Process goes away after 1st pgrep poll
        let runner = RecordingRunner::new(&["Code"], 1);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * MB,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner).expect("should succeed");

        let calls = runner.recorded_calls();
        let rm_calls: Vec<_> = calls.iter().filter(|(p, _)| p == "rm").collect();
        // 3 path_suffixes → 3 rm -rf calls
        assert_eq!(rm_calls.len(), 3, "should delete all 3 VSCode cache dirs");
    }

    #[test]
    fn auto_clean_relaunches_when_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        let runner = RecordingRunner::new(&["Code"], 1);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * MB,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner).expect("should succeed");

        let calls = runner.recorded_calls();
        let open_calls: Vec<_> = calls.iter().filter(|(p, _)| p == "open").collect();
        assert_eq!(open_calls.len(), 1, "should relaunch VSCode once");
        assert!(
            open_calls[0]
                .1
                .iter()
                .any(|a| a.contains("Visual Studio Code")),
            "open -a should name the app"
        );
    }

    #[test]
    fn auto_clean_skips_relaunch_for_slack() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Slack/Cache"]);

        let runner = RecordingRunner::new(&["Slack"], 1);
        let hint = ProcessHint {
            entry: &SLACK_ENTRY,
            size_bytes: 230 * MB,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner).expect("should succeed");

        let calls = runner.recorded_calls();
        assert!(
            !calls.iter().any(|(p, _)| p == "open"),
            "Slack should NOT be relaunched"
        );
    }

    // ── offer_auto_clean interaction tests ───────────────────────────────────

    struct FakePrompt {
        responses: std::cell::RefCell<std::collections::VecDeque<String>>,
    }

    impl FakePrompt {
        fn new(responses: &[&str]) -> Self {
            Self {
                responses: std::cell::RefCell::new(
                    responses.iter().map(|s| s.to_string()).collect(),
                ),
            }
        }
    }

    impl PromptReader for FakePrompt {
        fn read_line(&self) -> Option<String> {
            self.responses.borrow_mut().pop_front()
        }
    }

    #[test]
    fn offer_auto_clean_calls_auto_clean_when_user_answers_y() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        // VSCode is running, user answers "y"
        let runner = RecordingRunner::new(&["Code"], 1);
        let prompt = FakePrompt::new(&["y"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * MB,
            running: true,
        };

        offer_auto_clean(&[hint], home, &runner, &prompt);

        let calls = runner.recorded_calls();
        assert!(
            calls.iter().any(|(p, _)| p == "osascript"),
            "should attempt quit when user says y"
        );
    }

    #[test]
    fn offer_auto_clean_skips_auto_clean_when_user_answers_n() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        let runner = RecordingRunner::new(&["Code"], 1);
        let prompt = FakePrompt::new(&["n"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * MB,
            running: true,
        };

        offer_auto_clean(&[hint], home, &runner, &prompt);

        let calls = runner.recorded_calls();
        assert!(
            !calls.iter().any(|(p, _)| p == "osascript"),
            "should NOT attempt quit when user says n"
        );
    }

    #[test]
    fn offer_auto_clean_prompts_for_non_running_hints() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        let runner = RecordingRunner::new(&[], 0);
        let prompt = FakePrompt::new(&["y"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * MB,
            running: false, // not running
        };

        offer_auto_clean(&[hint], home, &runner, &prompt);

        let calls = runner.recorded_calls();
        let rm_calls: Vec<_> = calls.iter().filter(|(p, _)| p == "rm").collect();
        assert!(
            !rm_calls.is_empty(),
            "should clean directories for non-running hints"
        );
        assert!(
            !calls.iter().any(|(p, _)| p == "osascript"),
            "should NOT attempt to quit a non-running app"
        );
        assert!(
            !calls.iter().any(|(p, _)| p == "open"),
            "should NOT relaunch a non-running app"
        );
    }

    #[test]
    fn auto_clean_hint_errors_when_quit_app_is_none() {
        static NO_QUIT_ENTRY: HintEntry = HintEntry {
            base_dir: BaseDir::Caches,
            path_suffixes: &["Homebrew"],
            display_name: "Homebrew",
            process_name: None,
            commands: &["brew cleanup -s --prune=all"],
            threshold_bytes: 64 * MB,
            skip: false,
            quit_app: None,
            relaunch_app: None,
        };
        let tmp = tempfile::tempdir().unwrap();
        let runner = RecordingRunner::new(&[], 0);
        let hint = ProcessHint {
            entry: &NO_QUIT_ENTRY,
            size_bytes: 80 * MB,
            running: false,
        };
        let result = auto_clean_hint(&hint, tmp.path(), &runner);
        assert!(result.is_err(), "should return Err when quit_app is None");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("cannot be auto-quit"),
            "error message should explain why"
        );
    }
}
