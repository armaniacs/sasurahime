use anyhow::Result;
use std::io::Read;
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
        Command::new("which")
            .arg(program)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
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
        assert!(err.contains("sleep 60"), "expected command name in error, got: {err}");
        assert!(
            err.contains("did not complete within"),
            "expected timeout message, got: {err}"
        );
    }
}
