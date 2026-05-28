pub mod apps;
mod cleaner;
mod detector;

pub use cleaner::{offer_auto_clean, print_hints};
pub use detector::collect_hints;

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

/// Returns the resolved base directory path for a given `BaseDir` variant.
fn base_path(home: &Path, base: BaseDir) -> PathBuf {
    match base {
        BaseDir::Caches => home.join("Library/Caches"),
        BaseDir::AppSupport => home.join("Library/Application Support"),
        BaseDir::Logs => home.join("Library/Logs"),
    }
}

// ─── tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandRunner;
    use anyhow::Result;
    use cleaner::auto_clean_hint;
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
    fn base_path_caches() {
        let home = Path::new("/Users/test");
        assert_eq!(
            base_path(home, BaseDir::Caches),
            Path::new("/Users/test/Library/Caches")
        );
    }

    #[test]
    fn base_path_app_support() {
        let home = Path::new("/Users/test");
        assert_eq!(
            base_path(home, BaseDir::AppSupport),
            Path::new("/Users/test/Library/Application Support")
        );
    }

    #[test]
    fn base_path_logs() {
        let home = Path::new("/Users/test");
        assert_eq!(
            base_path(home, BaseDir::Logs),
            Path::new("/Users/test/Library/Logs")
        );
    }

    #[test]
    fn collect_hints_filters_below_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::Caches, &["Homebrew", "typescript"]);

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
        assert!(hints[0].size_bytes >= hints[1].size_bytes);
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
        let runner = FakeRunner::new(&[("Slack", 230 * 1024)], &["Slack"]);
        let hints = collect_hints(home, &runner);
        let slack = hints.iter().find(|h| h.entry.display_name == "Slack");
        assert!(slack.is_some());
        assert!(slack.unwrap().running);
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
        assert!(vscode.is_some());
        assert_eq!(vscode.unwrap().size_bytes, 600 * 1024 * 1024);
    }

    #[test]
    fn collect_hints_includes_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::Logs, &["Claude"]);
        let runner = FakeRunner::new(&[("Claude", 62 * 1024)], &["Claude"]);
        let hints = collect_hints(home, &runner);
        let log = hints.iter().find(|h| h.entry.display_name == "Claude logs");
        assert!(log.is_some());
        assert!(log.unwrap().running);
    }

    #[test]
    fn print_hints_empty_produces_no_output() {
        print_hints(&[]);
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
        threshold_bytes: 64 * 1024 * 1024,
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
        threshold_bytes: 64 * 1024 * 1024,
        skip: false,
        quit_app: Some("Slack"),
        relaunch_app: None,
    };

    use std::sync::Mutex;

    struct RecordingRunner {
        calls: Mutex<Vec<(String, Vec<String>)>>,
        initially_running: Vec<String>,
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
                    Ok(make_output(0, &format!("102400\t{path}\n")))
                }
                "osascript" | "open" | "rm" => Ok(make_output(0, "")),
                _ => anyhow::bail!("RecordingRunner: unexpected {program} (call #{call_count})"),
            }
        }

        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn auto_clean_skips_if_quit_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        let runner = RecordingRunner::new(&["Code"], usize::MAX);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        let result = auto_clean_hint(&hint, home, &runner);
        assert!(result.is_err());
        let calls = runner.recorded_calls();
        assert!(calls.iter().any(|(p, _)| p == "osascript"));
        assert!(!calls.iter().any(|(p, _)| p == "rm"));
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

        let runner = RecordingRunner::new(&["Code"], 1);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner).expect("should succeed");
        let calls = runner.recorded_calls();
        let rm_calls: Vec<_> = calls.iter().filter(|(p, _)| p == "rm").collect();
        assert_eq!(rm_calls.len(), 3);
    }

    #[test]
    fn auto_clean_relaunches_when_configured() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Code/Cache"]);

        let runner = RecordingRunner::new(&["Code"], 1);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner).expect("should succeed");
        let calls = runner.recorded_calls();
        let open_calls: Vec<_> = calls.iter().filter(|(p, _)| p == "open").collect();
        assert_eq!(open_calls.len(), 1);
        assert!(open_calls[0]
            .1
            .iter()
            .any(|a| a.contains("Visual Studio Code")));
    }

    #[test]
    fn auto_clean_skips_relaunch_for_slack() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::AppSupport, &["Slack/Cache"]);

        let runner = RecordingRunner::new(&["Slack"], 1);
        let hint = ProcessHint {
            entry: &SLACK_ENTRY,
            size_bytes: 230 * 1024 * 1024,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner).expect("should succeed");
        let calls = runner.recorded_calls();
        assert!(!calls.iter().any(|(p, _)| p == "open"));
    }

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

        let runner = RecordingRunner::new(&["Code"], 1);
        let prompt = FakePrompt::new(&["y"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        offer_auto_clean(&[hint], home, &runner, &prompt);
        let calls = runner.recorded_calls();
        assert!(calls.iter().any(|(p, _)| p == "osascript"));
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
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        offer_auto_clean(&[hint], home, &runner, &prompt);
        let calls = runner.recorded_calls();
        assert!(!calls.iter().any(|(p, _)| p == "osascript"));
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
            size_bytes: 600 * 1024 * 1024,
            running: false,
        };

        offer_auto_clean(&[hint], home, &runner, &prompt);
        let calls = runner.recorded_calls();
        assert!(!calls.iter().any(|(p, _)| p == "osascript"));
        assert!(!calls.iter().any(|(p, _)| p == "open"));
    }
}
