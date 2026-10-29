pub mod apps;
mod cleaner;
mod detector;

pub use cleaner::{offer_auto_clean, print_hints};
pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};
pub use detector::collect_hints;
pub(crate) use detector::is_running;

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

    /// 1.25 MB, a multiple of the 4096-byte APFS page so that
    /// `format::dir_size` (st_blocks × 512) measures it exactly. Above the
    /// 1 MB log threshold.
    const ABOVE_LOG_THRESHOLD: usize = 1_310_720;

    struct FakeRunner {
        /// Process names that are "running" (pgrep returns exit 0).
        running: Vec<String>,
    }

    impl FakeRunner {
        fn new(running: &[&str]) -> Self {
            Self {
                running: running.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
            match program {
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

    /// Creates dirs and writes a `file_bytes`-byte file in each so that
    /// `format::dir_size` measures exactly `file_bytes`. Byte counts must be
    /// multiples of 4096 (the APFS page size).
    fn make_dirs_with_files(home: &Path, base: BaseDir, suffixes: &[&str], file_bytes: usize) {
        let base_path = match base {
            BaseDir::Caches => home.join("Library/Caches"),
            BaseDir::AppSupport => home.join("Library/Application Support"),
            BaseDir::Logs => home.join("Library/Logs"),
        };
        for s in suffixes {
            let dir = base_path.join(s);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("data.bin"), vec![0u8; file_bytes]).unwrap();
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
    fn collect_hints_uses_format_dir_size_metric() {
        // Unification test: hint sizes must be measured by the same
        // `format::dir_size` metric as scan, not by `du -sk`.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(home, BaseDir::Logs, &["Claude"], ABOVE_LOG_THRESHOLD);

        let runner = FakeRunner::new(&[]);
        let hints = collect_hints(home, &runner);

        let claude = hints.iter().find(|h| h.entry.display_name == "Claude logs");
        assert!(claude.is_some(), "hint above threshold must be found");
        assert_eq!(
            claude.unwrap().size_bytes,
            crate::format::dir_size(&home.join("Library/Logs/Claude")),
            "hint size must equal format::dir_size on the same path"
        );
    }

    #[test]
    fn collect_hints_filters_below_threshold() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(home, BaseDir::Logs, &["Claude"], ABOVE_LOG_THRESHOLD);
        make_dirs_with_files(home, BaseDir::Logs, &["zoom.us"], 4_096);

        let runner = FakeRunner::new(&[]);
        let hints = collect_hints(home, &runner);

        assert!(hints.iter().any(|h| h.entry.display_name == "Claude logs"));
        assert!(!hints.iter().any(|h| h.entry.display_name == "Zoom logs"));
    }

    #[test]
    fn collect_hints_limits_to_top5() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::Logs,
            &[
                "Claude",
                "zoom.us",
                "DiagnosticReports",
                "LM Studio",
                "CrashReporter",
                "fsck_hfs.log",
            ],
            ABOVE_LOG_THRESHOLD,
        );
        let runner = FakeRunner::new(&[]);
        let hints = collect_hints(home, &runner);
        assert_eq!(hints.len(), 5);
        assert!(hints[0].size_bytes >= hints[1].size_bytes);
    }

    #[test]
    fn collect_hints_excludes_skip_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(home, BaseDir::Caches, &["GeoServices"]);
        let runner = FakeRunner::new(&[]);
        let hints = collect_hints(home, &runner);
        assert!(!hints.iter().any(|h| h.entry.display_name == "GeoServices"));
    }

    #[test]
    fn collect_hints_sets_running_flag() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(home, BaseDir::Logs, &["Claude"], ABOVE_LOG_THRESHOLD);
        let runner = FakeRunner::new(&["Claude"]);
        let hints = collect_hints(home, &runner);
        let claude = hints.iter().find(|h| h.entry.display_name == "Claude logs");
        assert!(claude.is_some());
        assert!(claude.unwrap().running);
    }

    #[test]
    fn collect_hints_aggregates_vscode_dirs() {
        // 22 MB per suffix (multiple of 4096); 3 × 22 MB > the 64 MB threshold.
        const SUFFIX_BYTES: usize = 22 * 1_048_576;
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::AppSupport,
            &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
            SUFFIX_BYTES,
        );
        let runner = FakeRunner::new(&[]);
        let hints = collect_hints(home, &runner);
        let vscode = hints
            .iter()
            .find(|h| h.entry.display_name == "VSCode caches");
        assert!(vscode.is_some());
        assert_eq!(vscode.unwrap().size_bytes, 3 * SUFFIX_BYTES as u64);
    }

    #[test]
    fn collect_hints_includes_logs() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(home, BaseDir::Logs, &["Claude"], ABOVE_LOG_THRESHOLD);
        let runner = FakeRunner::new(&["Claude"]);
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
        /// Programs whose `run` exits non-zero with the given code (PBI 65:
        /// exit-code injection for the quit/relaunch failure branches).
        failure_codes: Vec<(String, i32)>,
    }

    impl RecordingRunner {
        fn new(running: &[&str], gone_after: usize) -> Self {
            Self {
                calls: Mutex::new(vec![]),
                initially_running: running.iter().map(|s| s.to_string()).collect(),
                gone_after_pgrep_calls: gone_after,
                failure_codes: vec![],
            }
        }

        /// Make every call to `program` exit with `code` (a real exit code,
        /// encoded as the raw wait status).
        fn with_exit_code(mut self, program: &str, code: i32) -> Self {
            self.failure_codes.push((program.to_string(), code));
            self
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
                "osascript" | "open" | "rm" => {
                    let code = self
                        .failure_codes
                        .iter()
                        .find(|(p, _)| p == program)
                        .map(|(_, c)| *c)
                        .unwrap_or(0);
                    Ok(make_output(code << 8, ""))
                }
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

        let result = auto_clean_hint(&hint, home, &runner, crate::trash::TrashMode::Permanent);
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

        // Pin permanent: the seam's Trash move is TCC-environment-dependent,
        // but the permanent path (uchg clear + fs removal) is deterministic
        // here.
        let runner = RecordingRunner::new(&["Code"], 1);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        auto_clean_hint(&hint, home, &runner, crate::trash::TrashMode::Permanent)
            .expect("should succeed");
        // The seam attempted one uchg clear per deleted path (the recording
        // runner rejects `chflags`, which exercises the warn-and-continue
        // contract), and the permanent removal actually deleted the dirs.
        let calls = runner.recorded_calls();
        let chflags_calls = calls.iter().filter(|(p, _)| p == "chflags").count();
        assert_eq!(chflags_calls, 3, "one uchg clear attempt per deleted path");
        assert!(
            !home.join("Library/Application Support/Code/Cache").exists(),
            "Code/Cache must be deleted"
        );
        assert!(
            !home
                .join("Library/Application Support/Code/CachedExtensionVSIXs")
                .exists(),
            "Code/CachedExtensionVSIXs must be deleted"
        );
        assert!(
            !home
                .join("Library/Application Support/Code/CachedData")
                .exists(),
            "Code/CachedData must be deleted"
        );
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

        auto_clean_hint(&hint, home, &runner, crate::trash::TrashMode::Permanent)
            .expect("should succeed");
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

        auto_clean_hint(&hint, home, &runner, crate::trash::TrashMode::Permanent)
            .expect("should succeed");
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

        offer_auto_clean(
            &[hint],
            home,
            &runner,
            &prompt,
            crate::trash::TrashMode::Permanent,
            false, // dry_run
        );
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

        offer_auto_clean(
            &[hint],
            home,
            &runner,
            &prompt,
            crate::trash::TrashMode::Permanent,
            false, // dry_run
        );
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

        offer_auto_clean(
            &[hint],
            home,
            &runner,
            &prompt,
            crate::trash::TrashMode::Permanent,
            false, // dry_run
        );
        let calls = runner.recorded_calls();
        assert!(!calls.iter().any(|(p, _)| p == "osascript"));
        assert!(!calls.iter().any(|(p, _)| p == "open"));
    }

    #[test]
    fn offer_auto_clean_dry_run_prompts_nothing_and_invokes_no_deletion_command() {
        // PBI 80 dry-run pin: a dry run is the top safety contract — it never
        // prompts (the `y` sitting in the prompt's answer queue is never
        // read) and never reaches the quit/delete/relaunch seam. Recording
        // Runner must record no osascript / rm / chflags / open, and no file
        // may change.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs(
            home,
            BaseDir::AppSupport,
            &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
        );

        let runner = RecordingRunner::new(&["Code"], 1);
        let prompt = FakePrompt::new(&["y"]);
        let running_hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };
        let idle_hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: false,
        };

        offer_auto_clean(
            &[running_hint, idle_hint],
            home,
            &runner,
            &prompt,
            crate::trash::TrashMode::Permanent,
            true, // dry_run
        );
        let calls = runner.recorded_calls();
        for program in ["osascript", "rm", "chflags", "open"] {
            assert!(
                !calls.iter().any(|(p, _)| p == program),
                "a dry run must never invoke {program}, got: {calls:?}"
            );
        }
        assert!(
            home.join("Library/Application Support/Code/Cache").exists()
                && home
                    .join("Library/Application Support/Code/CachedExtensionVSIXs")
                    .exists()
                && home
                    .join("Library/Application Support/Code/CachedData")
                    .exists(),
            "nothing may be deleted in dry-run mode"
        );
        assert_eq!(
            prompt.responses.borrow().len(),
            1,
            "the y answer must never be consumed: a dry run never prompts"
        );
    }

    static NO_QUIT_ENTRY: HintEntry = HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["x"],
        display_name: "NoQuit",
        process_name: None,
        commands: &[],
        threshold_bytes: 0,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    };

    #[test]
    fn quit_app_and_wait_errors_when_entry_has_no_quit_app() {
        let runner = RecordingRunner::new(&[], 0);
        let err = cleaner::quit_app_and_wait(&NO_QUIT_ENTRY, &runner).unwrap_err();
        assert!(err.to_string().contains("cannot be auto-quit"));
        assert!(runner.recorded_calls().is_empty());
    }

    static QUOTED_QUIT_ENTRY: HintEntry = HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["x"],
        display_name: "QuotedApp",
        process_name: Some("QuotedApp"),
        commands: &[],
        threshold_bytes: 0,
        skip: false,
        quit_app: Some("Foo\"Bar"),
        relaunch_app: None,
    };

    #[test]
    fn quit_app_and_wait_escapes_quotes_in_quit_app() {
        // A `"` in the app name must not close the `quit app "..."` literal
        // early: the recorded osascript argument carries it escaped.
        let runner = RecordingRunner::new(&[], 0);
        cleaner::quit_app_and_wait(&QUOTED_QUIT_ENTRY, &runner).expect("quit should succeed");
        let calls = runner.recorded_calls();
        let osascript = calls
            .iter()
            .find(|(p, _)| p == "osascript")
            .expect("osascript must be invoked");
        assert_eq!(osascript.1[0], "-e");
        assert_eq!(osascript.1[1], "quit app \"Foo\\\"Bar\"");
    }

    #[test]
    fn relaunch_app_opens_configured_app() {
        let runner = RecordingRunner::new(&[], 0);
        cleaner::relaunch_app(&VSCODE_ENTRY, &runner).unwrap();
        let calls = runner.recorded_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "open");
        assert_eq!(
            calls[0].1,
            vec!["-a".to_string(), "Visual Studio Code".to_string()]
        );
    }

    #[test]
    fn relaunch_app_does_nothing_without_relaunch_app() {
        let runner = RecordingRunner::new(&[], 0);
        cleaner::relaunch_app(&SLACK_ENTRY, &runner).unwrap();
        assert!(runner.recorded_calls().is_empty());
    }

    #[test]
    fn quit_app_and_wait_bails_when_osascript_exits_nonzero() {
        // PBI 53 pin: a failed osascript is the real reason the app will not
        // quit, so the bail must surface it instead of falling through to the
        // 10s quit poll (the pgrep fixtures report the app as still running).
        let runner = RecordingRunner::new(&["Code"], usize::MAX).with_exit_code("osascript", 1);
        let err = cleaner::quit_app_and_wait(&VSCODE_ENTRY, &runner).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("osascript failed to quit"),
            "the osascript failure must surface in the reason, got: {msg}"
        );
        assert!(
            msg.contains("(exit 1)"),
            "the osascript exit code must surface in the reason, got: {msg}"
        );
        let calls = runner.recorded_calls();
        assert!(
            calls.iter().any(|(p, _)| p == "osascript"),
            "osascript must have been invoked"
        );
        assert!(
            !calls.iter().any(|(p, _)| p == "pgrep"),
            "the osascript failure must bail before the quit poll, got: {calls:?}"
        );
    }

    #[test]
    fn relaunch_app_warns_without_ok_when_open_exits_nonzero() {
        // PBI 53 pin: a failed `open -a` relaunch must not fail the run and
        // must take the warn branch (no success line) instead of bubbling up
        // as an error. The open call itself must still have been attempted.
        let runner = RecordingRunner::new(&[], 0).with_exit_code("open", 1);
        cleaner::relaunch_app(&VSCODE_ENTRY, &runner)
            .expect("a failed relaunch must not fail the run");
        let calls = runner.recorded_calls();
        let open_calls: Vec<_> = calls.iter().filter(|(p, _)| p == "open").collect();
        assert_eq!(
            open_calls.len(),
            1,
            "the relaunch must still be attempted, got: {calls:?}"
        );
    }
}
