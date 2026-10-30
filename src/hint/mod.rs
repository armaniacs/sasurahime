pub mod apps;
mod cleaner;
mod detector;

pub use cleaner::{is_known_hint_name, offer_auto_clean, print_hints, KnownAppHintsCleaner};
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
    use crate::cleaner::Cleaner;
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

    /// Shared call log: clones observe the same commands (the cleaner owns
    /// the runner, the test keeps a watcher for its recordings).
    type CallLog = std::sync::Arc<Mutex<Vec<(String, Vec<String>)>>>;

    #[derive(Clone)]
    struct RecordingRunner {
        calls: CallLog,
        initially_running: Vec<String>,
        gone_after_pgrep_calls: usize,
        /// Programs whose `run` exits non-zero with the given code (PBI 65:
        /// exit-code injection for the quit/relaunch failure branches).
        failure_codes: Vec<(String, i32)>,
    }

    impl RecordingRunner {
        fn new(running: &[&str], gone_after: usize) -> Self {
            Self {
                calls: std::sync::Arc::new(Mutex::new(vec![])),
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
    fn offer_auto_clean_collects_the_accepted_entry_when_user_answers_y() {
        // PBI-83: the offer is selection collection only — a `y` returns the
        // hinted entry in the selection and never executes a deletion
        // command; the execution rides the CleanByName seam through the
        // hints cleaner.
        let runner = RecordingRunner::new(&["Code"], 1);
        let prompt = FakePrompt::new(&["y"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        let selection = offer_auto_clean(&[hint], false, &prompt);
        assert_eq!(selection.len(), 1, "the y answer must collect the entry");
        assert!(
            std::ptr::eq(selection[0], &VSCODE_ENTRY),
            "the selection must carry the hinted entry"
        );
        assert!(
            !runner
                .recorded_calls()
                .iter()
                .any(|(p, _)| p == "osascript"),
            "the offer must never execute: the deletion rides the seam"
        );
    }

    #[test]
    fn offer_auto_clean_collects_nothing_when_user_answers_n() {
        let runner = RecordingRunner::new(&["Code"], 1);
        let prompt = FakePrompt::new(&["n"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: true,
        };

        let selection = offer_auto_clean(&[hint], false, &prompt);
        assert!(selection.is_empty(), "an n answer must collect nothing");
        assert!(
            !runner
                .recorded_calls()
                .iter()
                .any(|(p, _)| p == "osascript"),
            "a declined hint must never be executed"
        );
    }

    #[test]
    fn offer_auto_clean_prompts_for_non_running_hints() {
        // A non-running hint is still offered: the quit branch is a running
        // concern, the offer is a selection concern.
        let runner = RecordingRunner::new(&[], 0);
        let prompt = FakePrompt::new(&["y"]);
        let hint = ProcessHint {
            entry: &VSCODE_ENTRY,
            size_bytes: 600 * 1024 * 1024,
            running: false,
        };

        let selection = offer_auto_clean(&[hint], false, &prompt);
        assert_eq!(selection.len(), 1, "a non-running hint is still offered");
        let calls = runner.recorded_calls();
        assert!(
            !calls.iter().any(|(p, _)| p == "osascript") && !calls.iter().any(|(p, _)| p == "open"),
            "the offer must never execute, got: {calls:?}"
        );
    }

    #[test]
    fn offer_auto_clean_dry_run_collects_nothing_and_invokes_no_deletion_command() {
        // PBI 80 dry-run pin: a dry run is the top safety contract — it never
        // prompts (the `y` sitting in the prompt's answer queue is never
        // read) and never reaches the quit/delete/relaunch seam. The offer
        // collects nothing, so the caller makes no seam call at all.
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

        let selection = offer_auto_clean(&[running_hint, idle_hint], true, &prompt);
        assert!(selection.is_empty(), "a dry run must collect nothing");
        let calls = runner.recorded_calls();
        for program in ["osascript", "rm", "chflags", "open"] {
            assert!(
                !calls.iter().any(|(p, _)| p == program),
                "a dry run must never invoke {program}, got: {calls:?}"
            );
        }
        assert_eq!(
            prompt.responses.borrow().len(),
            1,
            "the y answer must never be consumed: a dry run never prompts"
        );
    }

    // ── KnownAppHintsCleaner (PBI-83) ────────────────────────────────────

    const DIAGNOSTIC_BYTES: usize = 2_097_152;

    #[test]
    fn hints_cleaner_pins_name_risk_and_gate() {
        // PBI-83 registry pins: 1 cleaner = 1 name, Moderate (never Safe),
        // no prompt inside the cleaner (the prompt is the entry point's job).
        let tmp = tempfile::tempdir().unwrap();
        let c =
            cleaner::KnownAppHintsCleaner::new(tmp.path(), Box::new(RecordingRunner::new(&[], 0)));
        assert_eq!(c.name(), "hints");
        assert_eq!(
            c.risk(),
            crate::cleaner::Risk::Moderate,
            "hints must never be Safe"
        );
        assert!(!c.auto_clean_allowed(), "Moderate never runs unattended");
        assert!(
            !c.interactive_gated(),
            "the cleaner must not gate on a prompt"
        );
        // PBI 87: clean() quits and relaunches running apps, so the cleaner
        // must report the quit-based predicate — the registry-driven
        // unattended runs (`auto`, `clean all`) exclude it on that report.
        assert!(
            c.quits_running_apps(),
            "hints must report that it quits running apps"
        );
    }

    #[test]
    fn hints_cleaner_detect_aggregates_the_hints_as_pruneable() {
        // detect() = the collect_hints aggregate: Pruneable(sum), no side
        // effects. 2 MB of DiagnosticReports debris > the 1 MB threshold.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::Logs,
            &["DiagnosticReports"],
            DIAGNOSTIC_BYTES,
        );

        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(RecordingRunner::new(&[], 0)));
        let result = c.detect();
        assert_eq!(result.name, "hints");
        assert_eq!(
            result.bytes_for_test(),
            DIAGNOSTIC_BYTES as u64,
            "detect must aggregate the collect_hints sum"
        );
    }

    #[test]
    fn hints_cleaner_detect_reports_not_found_without_debris() {
        let tmp = tempfile::tempdir().unwrap();
        let c =
            cleaner::KnownAppHintsCleaner::new(tmp.path(), Box::new(RecordingRunner::new(&[], 0)));
        let result = c.detect();
        assert!(
            matches!(result.status, crate::cleaner::ScanStatus::NotFound),
            "no debris above any threshold must report not found, got: {:?}",
            result.status
        );
    }

    #[test]
    fn hints_cleaner_sub_targets_return_display_name_and_size() {
        // sub_targets() = display_name + size, the xcode precedent: the TUI
        // selection list carries the scanned bytes.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::Logs,
            &["DiagnosticReports"],
            DIAGNOSTIC_BYTES,
        );
        make_dirs_with_files(home, BaseDir::Logs, &["CrashReporter"], 1_310_720);

        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(RecordingRunner::new(&[], 0)));
        assert_eq!(
            c.sub_targets(),
            vec![
                ("Diagnostic Reports", DIAGNOSTIC_BYTES as u64),
                ("Crash Reporter logs", 1_310_720),
            ],
            "sub_targets must list display_name + size, largest first"
        );
    }

    #[test]
    fn hints_cleaner_clean_runs_the_quit_delete_relaunch_flow() {
        // clean() = quit → ledger → relaunch (the PBI 80 integration
        // function) per selected hint. A running VSCode hint above the 64 MB
        // threshold: osascript first, then the deletion, then the relaunch.
        const SUFFIX_BYTES: usize = 22 * 1_048_576;
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::AppSupport,
            &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
            SUFFIX_BYTES,
        );

        let runner = RecordingRunner::new(&["Code"], 1);
        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(runner));
        let result = c
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions {
                    trash_mode: crate::trash::TrashMode::Permanent,
                    ..Default::default()
                },
            )
            .expect("clean must succeed");
        assert_eq!(result.name, "hints");
        assert_eq!(
            result.bytes_freed,
            3 * SUFFIX_BYTES as u64,
            "the freed bytes are the ledger's sum"
        );
        assert!(!home.join("Library/Application Support/Code/Cache").exists());
        assert!(!home
            .join("Library/Application Support/Code/CachedExtensionVSIXs")
            .exists());
        assert!(!home
            .join("Library/Application Support/Code/CachedData")
            .exists());
    }

    #[test]
    fn hints_cleaner_clean_dry_run_never_deletes() {
        // The top safety contract inside the cleaner too: a dry run must not
        // quit, delete or relaunch anything.
        const SUFFIX_BYTES: usize = 22 * 1_048_576;
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::AppSupport,
            &["Code/Cache", "Code/CachedExtensionVSIXs", "Code/CachedData"],
            SUFFIX_BYTES,
        );

        let runner = RecordingRunner::new(&["Code"], 1);
        let call_watcher = runner.clone();
        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(runner));
        let result = c
            .clean(
                true,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions {
                    trash_mode: crate::trash::TrashMode::Permanent,
                    ..Default::default()
                },
            )
            .expect("dry run must succeed");
        let calls = call_watcher.recorded_calls();
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
            result.bytes_freed, 0,
            "a dry run must not report freed bytes"
        );
    }

    #[test]
    fn hints_cleaner_clean_with_sub_cleans_only_that_entry() {
        // --sub / the TUI's sub-target selection: only the named entry is
        // cleaned, the other candidate survives.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::Logs,
            &["DiagnosticReports"],
            DIAGNOSTIC_BYTES,
        );
        make_dirs_with_files(home, BaseDir::Logs, &["CrashReporter"], 1_310_720);

        let runner = RecordingRunner::new(&[], 0);
        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(runner))
            .with_sub_names(vec!["Diagnostic Reports".to_string()]);
        let result = c
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions {
                    trash_mode: crate::trash::TrashMode::Permanent,
                    ..Default::default()
                },
            )
            .expect("clean must succeed");
        assert_eq!(
            result.bytes_freed, DIAGNOSTIC_BYTES as u64,
            "only the selected entry's bytes are freed"
        );
        assert!(
            !home.join("Library/Logs/DiagnosticReports").exists(),
            "the selected entry must be cleaned"
        );
        assert!(
            home.join("Library/Logs/CrashReporter").exists(),
            "the other candidate must survive"
        );
    }

    #[test]
    fn hints_cleaner_clean_returns_the_aggregate_for_history() {
        // PBI 80 history contract, moved to the cleaner: clean() returns what
        // the hint path actually freed, named "hints" (1 cleaner = 1 name),
        // so the seam's run_clean_target rides the shared history guard.
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::Logs,
            &["DiagnosticReports"],
            DIAGNOSTIC_BYTES,
        );

        let runner = RecordingRunner::new(&[], 0);
        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(runner));
        let result = c
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions {
                    trash_mode: crate::trash::TrashMode::Permanent,
                    ..Default::default()
                },
            )
            .expect("clean must succeed");
        assert_eq!(result.name, "hints", "the result names the hints cleaner");
        assert_eq!(
            result.bytes_freed, DIAGNOSTIC_BYTES as u64,
            "the freed bytes are returned for the history write"
        );
        assert!(
            !result.uses_trash,
            "a permanent resolve must never claim Trash"
        );
        assert!(result.skipped.is_empty());
        assert_eq!(result.exit_code(), 0);
    }

    #[test]
    fn hints_cleaner_clean_reports_exit_code_one_when_every_delete_is_skipped() {
        // PBI 70/80 ledger contract, moved to the cleaner: a deletion that
        // fails with a skippable error is recorded as a skip, and an
        // all-skipped result reports failure through CleanResult::exit_code
        // — the seam's run_and_exit and run_hint_flow exit 1 on it.
        let _guard = crate::trash::TrashBackendOverrideGuard::set();
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        make_dirs_with_files(
            home,
            BaseDir::Logs,
            &["DiagnosticReports"],
            DIAGNOSTIC_BYTES,
        );

        let runner = RecordingRunner::new(&[], 0);
        let c = cleaner::KnownAppHintsCleaner::new(home, Box::new(runner));
        let result = c
            .clean(
                false,
                &crate::progress::DeepSuppressReporter,
                &crate::cleaner::CleanOptions {
                    trash_mode: crate::trash::TrashMode::Trash,
                    ..Default::default()
                },
            )
            .expect("clean must not error on a skipped delete");
        assert_eq!(result.bytes_freed, 0, "a skipped entry is never summed");
        assert!(!result.uses_trash, "a skipped move never claims Trash");
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.exit_code(), 1, "all-skipped reports failure");
        assert!(
            home.join("Library/Logs/DiagnosticReports").exists(),
            "the skipped dir survives"
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
