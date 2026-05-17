use anyhow::Result;
use std::process::{Command, Output};

#[allow(dead_code)]
pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output>;
    /// Returns true if `program` is available in PATH.
    fn exists(&self, program: &str) -> bool;
}

pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| anyhow::anyhow!("failed to run `{}`: {}", program, e))
    }

    fn exists(&self, program: &str) -> bool {
        Command::new("which")
            .arg(program)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}
