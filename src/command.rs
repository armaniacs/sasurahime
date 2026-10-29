use anyhow::Result;
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Output, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

/// Default timeout for external subprocess calls.
/// Applies to every `run()` invocation unless overridden.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output>;
    /// Returns true if `program` is available in PATH.
    fn exists(&self, program: &str) -> bool;
}

/// Opens `path` in Finder through `runner`. Non-zero exit is an error; a
/// non-UTF-8 path is rejected because `open` takes a UTF-8 argument.
pub fn open_path(runner: &dyn CommandRunner, path: &std::path::Path) -> Result<()> {
    let path = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))?;
    let output = runner.run("open", &[path])?;
    anyhow::ensure!(
        output.status.success(),
        "open returned non-zero exit: {:?}",
        output.status.code()
    );
    Ok(())
}

/// One owner of the tool-failure warning wording (PBI 81): the warning line
/// with the exit code, the stderr relay, and the optional remedy. Only the
/// wording is shared — the outcome policy stays at the call site
/// (warn+continue for a generic cache prune, bail for the CLI-fallback
/// adapter), because those are real domain policies, not drift. A remedy
/// prints when the caller has an actionable fix to offer. Writer-parameterized
/// so the wording is unit-testable, the same pattern as
/// `write_not_a_terminal_remedy` (prompt.rs).
pub fn warn_tool_failure(
    out: &mut dyn Write,
    display_name: &str,
    program: &str,
    args: &[&str],
    output: &Output,
    remedy: Option<&str>,
) -> std::io::Result<()> {
    writeln!(
        out,
        "[{display_name}] warning: `{program} {}` exited with code {:?}",
        args.join(" "),
        output.status.code()
    )?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        writeln!(out, "[{display_name}] stderr: {stderr}")?;
    }
    if let Some(remedy) = remedy {
        writeln!(out, "[{display_name}] {remedy}")?;
    }
    Ok(())
}

pub struct SystemCommandRunner;

impl SystemCommandRunner {
    fn run_with_timeout(&self, program: &str, args: &[&str], timeout: Duration) -> Result<Output> {
        let mut child = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow::anyhow!("failed to spawn `{}`: {}", program, e))?;

        match child.wait_timeout(timeout)? {
            Some(status) => {
                // Read remaining piped output.
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                if let Some(ref mut out) = child.stdout {
                    out.read_to_end(&mut stdout)?;
                }
                if let Some(ref mut err) = child.stderr {
                    err.read_to_end(&mut stderr)?;
                }
                Ok(Output {
                    status,
                    stdout,
                    stderr,
                })
            }
            None => {
                // Timed out — kill the child process and clean up.
                let _ = child.kill();
                let _ = child.wait();
                let cmd_str = if args.is_empty() {
                    program.to_string()
                } else {
                    format!("{} {}", program, args.join(" "))
                };
                anyhow::bail!(
                    "command `{cmd_str}` did not complete within {}s and was killed.\n\
You can run this command manually in another terminal:\n  $ {cmd_str}",
                    timeout.as_secs()
                );
            }
        }
    }
}

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        self.run_with_timeout(program, args, COMMAND_TIMEOUT)
    }

    fn exists(&self, program: &str) -> bool {
        // Resolve program against PATH natively instead of spawning `which`.
        // Avoids external process overhead and eliminates `which`-availability risk.
        std::env::var_os("PATH")
            .as_ref()
            .and_then(|path| {
                std::env::split_paths(path).find_map(|dir| {
                    let candidate = dir.join(program);
                    if candidate.is_file() {
                        // Also verify it's executable on Unix.
                        std::fs::metadata(&candidate).ok().and_then(|m| {
                            if m.permissions().mode() & 0o111 != 0 {
                                Some(())
                            } else {
                                None
                            }
                        })
                    } else {
                        None
                    }
                })
            })
            .is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_tool_not_found_returns_error() {
        let runner = SystemCommandRunner;
        let result = runner.run("this-tool-does-not-exist-12345", &[]);
        assert!(result.is_err(), "non-existent tool must error");
    }

    #[test]
    fn run_successful_command_returns_output() {
        let runner = SystemCommandRunner;
        let result = runner.run("echo", &["hello"]).unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout.as_slice(), b"hello\n");
    }

    #[test]
    fn run_captures_stderr() {
        let runner = SystemCommandRunner;
        let result = runner.run("sh", &["-c", "echo errmsg >&2"]).unwrap();
        assert!(result.status.success());
        assert_eq!(result.stderr.as_slice(), b"errmsg\n");
    }

    #[test]
    fn run_long_command_respects_timeout() {
        // Verify the timeout constant is finite and reasonable.
        assert!(
            COMMAND_TIMEOUT <= Duration::from_secs(60),
            "timeout should be at most 60s, got {}s",
            COMMAND_TIMEOUT.as_secs()
        );
        assert!(
            COMMAND_TIMEOUT >= Duration::from_secs(1),
            "timeout should be at least 1s, got {}s",
            COMMAND_TIMEOUT.as_secs()
        );
    }

    #[test]
    fn timeout_error_includes_manual_command_hint() {
        let runner = SystemCommandRunner;
        // Use a 10ms timeout so this test completes instantly.
        let result = runner.run_with_timeout("sleep", &["60"], Duration::from_millis(10));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("You can run this command manually"),
            "expected hint about manual command, got: {err}"
        );
        assert!(
            err.contains("sleep 60"),
            "expected command name in error, got: {err}"
        );
        assert!(
            err.contains("did not complete within"),
            "expected timeout message, got: {err}"
        );
    }

    use std::os::unix::process::ExitStatusExt;

    struct OpenRunner {
        status: i32,
        calls: std::sync::Mutex<Vec<(String, Vec<String>)>>,
    }

    impl OpenRunner {
        fn new(status: i32) -> Self {
            Self {
                status,
                calls: std::sync::Mutex::new(vec![]),
            }
        }
    }

    impl CommandRunner for OpenRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
            self.calls.lock().unwrap().push((
                program.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(self.status << 8),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    struct BrokenRunner;

    impl CommandRunner for BrokenRunner {
        fn run(&self, program: &str, _: &[&str]) -> Result<std::process::Output> {
            anyhow::bail!("failed to spawn `{program}`: No such file or directory")
        }
        fn exists(&self, _: &str) -> bool {
            false
        }
    }

    #[test]
    fn open_path_calls_open_with_the_path() {
        let runner = OpenRunner::new(0);
        let path = std::path::PathBuf::from("/tmp/Gone App");
        open_path(&runner, &path).unwrap();
        assert_eq!(
            *runner.calls.lock().unwrap(),
            vec![("open".to_string(), vec!["/tmp/Gone App".to_string()])]
        );
    }

    #[test]
    fn open_path_errors_on_non_zero_exit_and_mentions_the_code() {
        let runner = OpenRunner::new(1);
        let err = open_path(&runner, std::path::Path::new("/tmp/x")).unwrap_err();
        assert!(err.to_string().contains("non-zero"), "{err}");
        assert!(err.to_string().contains("1"), "{err}");
    }

    #[test]
    fn open_path_propagates_a_runner_error() {
        let err = open_path(&BrokenRunner, std::path::Path::new("/tmp/x")).unwrap_err();
        assert!(err.to_string().contains("failed to spawn"), "{err}");
    }

    #[test]
    fn open_path_passes_spaces_and_non_ascii_as_one_argument() {
        let runner = OpenRunner::new(0);
        let path = std::path::PathBuf::from("/tmp/a b/\u{65e5}\u{672c}\u{8a9e}.app");
        open_path(&runner, &path).unwrap();
        let calls = runner.calls.lock().unwrap();
        assert_eq!(calls[0].1.len(), 1, "must be a single argument");
        assert_eq!(calls[0].1[0], path.to_string_lossy());
    }

    #[cfg(unix)]
    #[test]
    fn open_path_rejects_a_non_utf8_path() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;
        let runner = OpenRunner::new(0);
        let path = std::path::PathBuf::from(OsString::from_vec(vec![b'/', 0xff, b'x']));
        assert!(open_path(&runner, &path).is_err());
        assert!(runner.calls.lock().unwrap().is_empty());
    }

    #[cfg(unix)]
    fn failed_output(code: i32, stderr: &[u8]) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: vec![],
            stderr: stderr.to_vec(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn warn_tool_failure_prints_the_warning_line_with_the_exit_code() {
        // The wording has one owner (PBI 81): the warning line names the
        // cleaner and the exact command, then relays the exit code.
        let output = failed_output(1, b"boom");
        let mut buf: Vec<u8> = Vec::new();
        warn_tool_failure(
            &mut buf,
            "bun",
            "bun",
            &["pm", "cache", "rm"],
            &output,
            None,
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.contains("[bun] warning:"), "{text}");
        assert!(text.contains("`bun pm cache rm`"), "{text}");
        assert!(text.contains("Some(1)"), "the code must be relayed: {text}");
    }

    #[cfg(unix)]
    #[test]
    fn warn_tool_failure_relays_non_empty_stderr_once() {
        let output = failed_output(1, b"boom");
        let mut buf: Vec<u8> = Vec::new();
        warn_tool_failure(
            &mut buf,
            "bun",
            "bun",
            &["pm", "cache", "rm"],
            &output,
            None,
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            text.matches("[bun] stderr: boom").count() == 1,
            "non-empty stderr is relayed exactly once: {text}"
        );
        assert_eq!(
            text.lines().count(),
            2,
            "warning + stderr, no remedy: {text}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn warn_tool_failure_omits_the_stderr_relay_when_stderr_is_empty() {
        let output = failed_output(1, b"");
        let mut buf: Vec<u8> = Vec::new();
        warn_tool_failure(
            &mut buf,
            "bun",
            "bun",
            &["pm", "cache", "rm"],
            &output,
            None,
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            !text.contains("stderr:"),
            "empty stderr must not be relayed: {text}"
        );
        assert_eq!(text.lines().count(), 1, "the warning line only: {text}");
    }

    #[cfg(unix)]
    #[test]
    fn warn_tool_failure_prints_the_remedy_when_present() {
        let output = failed_output(1, b"");
        let mut buf: Vec<u8> = Vec::new();
        warn_tool_failure(
            &mut buf,
            "colima",
            "colima",
            &["ssh", "--", "sudo", "fstrim", "-av"],
            &output,
            Some("start the VM to trim"),
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            text.contains("[colima] start the VM to trim"),
            "the remedy must print: {text}"
        );
        assert_eq!(text.lines().count(), 2, "warning + remedy: {text}");
    }

    #[cfg(unix)]
    #[test]
    fn warn_tool_failure_omits_the_remedy_when_none() {
        let output = failed_output(1, b"");
        let mut buf: Vec<u8> = Vec::new();
        warn_tool_failure(
            &mut buf,
            "bun",
            "bun",
            &["pm", "cache", "rm"],
            &output,
            None,
        )
        .unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert_eq!(text.lines().count(), 1, "no remedy line: {text}");
    }

    #[cfg(unix)]
    #[test]
    fn warn_tool_failure_relays_a_missing_exit_code_as_none() {
        // A signal-terminated tool has no exit code: the raw wait status
        // carries the signal instead, and code() returns None.
        let output = Output {
            status: std::process::ExitStatus::from_raw(9),
            stdout: vec![],
            stderr: vec![],
        };
        let mut buf: Vec<u8> = Vec::new();
        warn_tool_failure(&mut buf, "bun", "bun", &[], &output, None).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(
            text.contains("None"),
            "a signal-terminated tool must relay the missing code: {text}"
        );
    }

    #[test]
    fn tool_failure_warning_wording_has_one_owner() {
        // PBI 81: the tool-failure warning vocabulary is owned by
        // warn_tool_failure in this file. The copies in generic.rs and the
        // cli_fallback print part used to hand-write the line, so the
        // outcome could drift while the wording drifted with it; the pin
        // keeps the wording out of the cleaner layer. uv.rs and brew.rs
        // keep their own variants by design (log-only, exit-to-empty — the
        // PBI's scope fence leaves them out). The wording is assembled from
        // parts so this pin's own source never carries the literal twice.
        let wording = ["exited", "with", "code"].join(" ");
        let cleaner_sources = [
            include_str!("cleaners/generic.rs"),
            include_str!("cli_fallback.rs"),
            include_str!("cleaners/uv.rs"),
        ];
        for src in cleaner_sources {
            assert!(
                !src.contains(&wording),
                "a hand-written tool-failure warning appeared in a cleaner — \
                 the wording is owned by warn_tool_failure"
            );
        }
        // The owner: the wording appears exactly once, inside
        // warn_tool_failure itself.
        let me = include_str!("command.rs");
        assert_eq!(
            me.matches(&wording).count(),
            1,
            "the warning wording must have exactly one owner"
        );
    }
}
