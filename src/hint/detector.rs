use crate::command::CommandRunner;
use std::path::{Path, PathBuf};

use super::{apps, BaseDir, ProcessHint};

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

/// Collect hints for known applications whose cache size exceeds the threshold.
pub fn collect_hints(home: &Path, runner: &dyn CommandRunner) -> Vec<ProcessHint> {
    let mut hints: Vec<ProcessHint> = apps::KNOWN_ENTRIES
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
