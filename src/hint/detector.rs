use crate::command::CommandRunner;
use crate::format::dir_size;
use std::path::Path;

use super::{apps, base_path, ProcessHint};

pub(crate) fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool {
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
            // Same metric as scan and explore (`format::dir_size`): one size
            // standard everywhere, no `du` round-trip or dual accounting.
            let size_bytes: u64 = entry
                .path_suffixes
                .iter()
                .map(|s| dir_size(&base.join(s)))
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
