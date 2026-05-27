use crate::command::CommandRunner;
use filetime::{set_file_mtime, FileTime};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::time::{Duration, SystemTime};

// ── Mock CommandRunner ──

enum MockBehavior {
    Output {
        program: String,
        output: std::process::Output,
    },
    ExitCode {
        program: String,
        code: i32,
    },
}

/// A configurable mock for [`CommandRunner`].
///
/// # Examples
///
/// ```ignore
/// // Tool not found
/// let runner = MockRunner::new().with_not_found();
///
/// // Tool succeeds with empty output
/// let runner = MockRunner::new().with_success("brew");
///
/// // Tool returns specific output
/// let runner = MockRunner::new().with_output("go", ok_output(b"OK"));
///
/// // Tool exits with failure
/// let runner = MockRunner::new().with_exit_code("rustup", 1);
/// ```
#[derive(Default)]
pub struct MockRunner {
    behaviors: Vec<MockBehavior>,
    not_found: bool,
}

impl MockRunner {
    pub fn new() -> Self {
        Self::default()
    }

    /// All tools report as not found (exists() returns false for everything).
    pub fn with_not_found(mut self) -> Self {
        self.not_found = true;
        self
    }

    /// A specific tool exists and succeeds with empty output.
    pub fn with_success(mut self, program: &str) -> Self {
        self.behaviors.push(MockBehavior::Output {
            program: program.to_string(),
            output: std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            },
        });
        self
    }

    /// A specific tool exists and returns the given output.
    pub fn with_output(mut self, program: &str, output: std::process::Output) -> Self {
        self.behaviors.push(MockBehavior::Output {
            program: program.to_string(),
            output,
        });
        self
    }

    /// A specific tool exists but exits with the given code.
    pub fn with_exit_code(mut self, program: &str, code: i32) -> Self {
        self.behaviors.push(MockBehavior::ExitCode {
            program: program.to_string(),
            code,
        });
        self
    }
}

impl CommandRunner for MockRunner {
    fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
        for b in &self.behaviors {
            match b {
                MockBehavior::Output { program: p, output } if p == program => {
                    return Ok(output.clone());
                }
                MockBehavior::ExitCode { program: p, code } if p == program => {
                    return Ok(std::process::Output {
                        status: std::process::ExitStatus::from_raw(*code),
                        stdout: vec![],
                        stderr: vec![],
                    });
                }
                _ => {}
            }
        }
        if self.not_found {
            anyhow::bail!("failed to spawn `{program}`: No such file or directory")
        }
        anyhow::bail!("mock runner: unexpected program `{program}` with args {args:?}")
    }

    fn exists(&self, program: &str) -> bool {
        for b in &self.behaviors {
            match b {
                MockBehavior::Output { program: p, .. } if p == program => return true,
                MockBehavior::ExitCode { program: p, .. } if p == program => return true,
                _ => {}
            }
        }
        if self.not_found {
            return false;
        }
        false
    }
}

// ── Fixture utilities ──

/// Write a file and set its modification time to `days_old` days ago.
pub fn write_aged_file(path: &Path, days_old: u64, content: &[u8]) {
    std::fs::write(path, content).unwrap();
    let mtime = SystemTime::now() - Duration::from_secs(days_old * 86_400);
    set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
}

/// Create a directory and all its ancestors, then set the dir mtime.
pub fn write_aged_dir(path: &Path, days_old: u64) {
    std::fs::create_dir_all(path).unwrap();
    let mtime = SystemTime::now() - Duration::from_secs(days_old * 86_400);
    set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
}

// ── Verbose flag guard ──

/// Sets verbose mode for the duration of a test, restoring on drop.
///
/// Uses the global [`crate::context::TEST_LOCK`] to synchronize access
/// to the verbose flag across parallel tests.
pub struct VerboseGuard {
    previous: bool,
    _lock: Option<std::sync::MutexGuard<'static, ()>>,
}

impl VerboseGuard {
    pub fn new() -> Self {
        let lock = crate::context::TEST_LOCK.lock().ok();
        let previous = crate::context::is_verbose();
        crate::context::set_verbose(true);
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for VerboseGuard {
    fn drop(&mut self) {
        crate::context::set_verbose(self.previous);
    }
}

// ── Exit status / output factories ──

/// Create an ExitStatus representing success (code 0).
pub fn exit_ok() -> std::process::ExitStatus {
    std::process::ExitStatus::from_raw(0)
}

/// Create a successful Output with the given stdout bytes.
pub fn ok_output(stdout: &[u8]) -> std::process::Output {
    std::process::Output {
        status: exit_ok(),
        stdout: stdout.to_vec(),
        stderr: vec![],
    }
}
