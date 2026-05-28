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

// ── EnvGuard ──

/// A panic-safe guard that sets an environment variable for the duration of a
/// test and restores the original value (or removes it) on `Drop`, even during
/// a panic.
pub struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    /// Set `key` to `val`, saving the previous value. Restores on drop.
    pub fn set(key: &'static str, val: &str) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, val);
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
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

    pub fn with_value(verbose: bool) -> Self {
        let lock = crate::context::TEST_LOCK.lock().ok();
        let previous = crate::context::is_verbose();
        crate::context::set_verbose(verbose);
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
