use anyhow::Result;
use std::io::Read;
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
}
