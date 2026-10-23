use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

/// Maximum number of history entries kept in the file.
/// Oldest entries are trimmed on append.
const MAX_HISTORY_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub cleaner: String,
    pub freed_bytes: u64,
    pub skipped_count: usize,
    /// Whether the freed bytes were moved to the macOS Trash (true) instead of
    /// permanently deleted (false). Older history files lack this field and
    /// default to false.
    #[serde(default)]
    pub moved_to_trash: bool,
}

#[derive(Debug, Default)]
pub struct StatsSummary {
    pub total_freed: u64,
    pub run_count: usize,
    pub entries: Vec<HistoryEntry>,
}

fn inner_width() -> usize {
    44
}

fn format_inner_line(content: &str, inner: usize) -> String {
    let padding = 2;
    let content_available = inner.saturating_sub(padding);
    let truncated: String = content.chars().take(content_available).collect();
    let right_pad = inner.saturating_sub(padding + truncated.len());
    format!(
        "{}{}{}",
        " ".repeat(padding),
        truncated,
        " ".repeat(right_pad)
    )
}

fn build_box(lines: &[String]) -> String {
    let inner = inner_width();
    let (tl, h, tr, vl, bl, br, hl) = if crate::format::use_unicode() {
        ('╔', '═', '╗', '║', '╚', '╝', '═')
    } else {
        ('+', '-', '+', '|', '+', '+', '-')
    };
    let mut out = String::new();
    out.push(tl);
    for _ in 0..inner {
        out.push(h);
    }
    out.push(tr);
    out.push('\n');

    for line in lines {
        out.push(vl);
        out.push_str(&format_inner_line(line, inner));
        out.push(vl);
        out.push('\n');
    }

    out.push(bl);
    for _ in 0..inner {
        out.push(hl);
    }
    out.push(br);
    out.push('\n');

    out
}

fn table_header() -> String {
    format!(
        "  {:>3}  {:<18}  {:<15}  {:>10}  {:>7}",
        "#", "Date", "Cleaner", "Size", "Method"
    )
}

fn table_row(i: usize, ts: &str, cleaner: &str, size: &str, method: &str) -> String {
    format!(
        "  {:>3}  {:<18}  {:<15}  {:>10}  {:>7}",
        i, ts, cleaner, size, method
    )
}

pub fn append_history(entry: &HistoryEntry, history_dir: &Path) -> anyhow::Result<()> {
    let history_path = history_dir.join("history.json");
    fs::create_dir_all(history_dir)?;
    // The exclusive lock must live on a file the rename never replaces: a lock
    // on history.json is inherited by the new inode after every rename, which
    // lets two processes serialize against different inodes and lose entries.
    let lock_path = history_dir.join("history.json.lock");
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)?;
    lock.lock_exclusive()?;

    let mut entries = load_history(&history_path);
    entries.push(entry.clone());
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    entries.truncate(MAX_HISTORY_ENTRIES);

    let json = serde_json::to_string_pretty(&entries)?;
    let tmp_path = history_dir.join("history.json.tmp");
    // Write via an owned file handle so the temp file is fsynced before the
    // rename: an interruption (or crash) can then never leave a torn file.
    // Content always comes from load_history under the lock, so the file is
    // never truncated outside the critical section.
    {
        use std::io::Write;
        let mut tmp = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp_path)?;
        tmp.write_all(json.as_bytes())?;
        tmp.sync_all()?;
    }
    fs::rename(&tmp_path, &history_path)?;
    // fsync the directory too: the rename is not durable on power loss until
    // the directory entry is synced.
    if let Ok(dir) = fs::File::open(history_dir) {
        let _ = dir.sync_all();
    }

    // Lock is released on drop.
    Ok(())
}

pub fn load_history(history_path: &Path) -> Vec<HistoryEntry> {
    if !history_path.exists() {
        return Vec::new();
    }
    let content = match fs::read_to_string(history_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    match serde_json::from_str::<Vec<HistoryEntry>>(&content) {
        Ok(mut entries) => {
            entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
            entries
        }
        Err(_) => {
            log::warn!("history file corrupted, starting fresh.");
            Vec::new()
        }
    }
}

pub fn compute_stats(entries: &[HistoryEntry]) -> StatsSummary {
    let total_freed = entries
        .iter()
        .fold(0u64, |acc, e| acc.saturating_add(e.freed_bytes));
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    StatsSummary {
        total_freed,
        run_count: entries.len(),
        entries: sorted,
    }
}

pub fn format_stats(summary: &StatsSummary) -> String {
    let total_freed_str = crate::format::format_bytes(summary.total_freed);
    let lines = vec![
        "  sasurahime Statistics".to_string(),
        format!("  Total freed:  {}", total_freed_str),
        format!("  Runs:         {}", summary.run_count),
    ];

    let mut out = build_box(&lines);
    format_entries_table(summary, summary.entries.len(), &mut out);
    out
}

pub fn format_stats_last(summary: &StatsSummary, last: usize) -> String {
    let total_freed_str = crate::format::format_bytes(summary.total_freed);
    let lines = vec![
        "  sasurahime Statistics".to_string(),
        format!("  Total freed:  {}", total_freed_str),
        format!("  Runs:         {}", summary.run_count),
    ];

    let mut out = build_box(&lines);
    format_entries_table(summary, last.min(summary.entries.len()), &mut out);
    out
}

fn format_entries_table(summary: &StatsSummary, count: usize, output: &mut String) {
    if summary.entries.is_empty() {
        return;
    }
    output.push_str("\nRecent cleanups:\n");
    output.push_str(&table_header());
    output.push('\n');

    for (i, entry) in summary.entries.iter().take(count).enumerate() {
        let ts = if entry.timestamp.len() >= 16 {
            &entry.timestamp[..16]
        } else {
            &entry.timestamp
        };
        let size = crate::format::format_bytes(entry.freed_bytes);
        // trash/physical distinction stays visible in stats.
        let method = if entry.moved_to_trash {
            "trash"
        } else {
            "delete"
        };
        output.push_str(&table_row(i + 1, ts, &entry.cleaner, &size, method));
        output.push('\n');
    }
}

pub fn default_history_dir(home: &Path) -> PathBuf {
    home.join(".local/share/sasurahime")
}

pub fn write_history_entry(
    label: &str,
    freed_bytes: u64,
    skipped_count: usize,
    moved_to_trash: bool,
) {
    if freed_bytes == 0 {
        return;
    }
    let home = dirs::home_dir().unwrap_or_default();
    let history_dir = default_history_dir(&home);
    let entry = HistoryEntry {
        timestamp: chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string(),
        cleaner: label.to_string(),
        freed_bytes,
        skipped_count,
        moved_to_trash,
    };
    if let Err(e) = append_history(&entry, &history_dir) {
        log::warn!(
            "failed to write history entry to {}: {e}",
            history_dir.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn append_and_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");
        let entry = HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 500_000_000,
            skipped_count: 0,
            moved_to_trash: false,
        };
        append_history(&entry, &history_dir).unwrap();
        let history_path = history_dir.join("history.json");
        assert!(history_path.exists());
        let loaded = load_history(&history_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].cleaner, "uv");
        assert_eq!(loaded[0].freed_bytes, 500_000_000);
        assert!(!loaded[0].moved_to_trash);
    }

    #[test]
    fn moved_to_trash_roundtrips() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");
        let entry = HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 100,
            skipped_count: 0,
            moved_to_trash: true,
        };
        append_history(&entry, &history_dir).unwrap();
        let loaded = load_history(&history_dir.join("history.json"));
        assert!(loaded[0].moved_to_trash, "trash distinction must survive");
    }

    #[test]
    fn older_history_without_moved_to_trash_defaults_to_false() {
        // Pre-22-05 history files have no moved_to_trash field; they must
        // still load.
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");
        std::fs::create_dir_all(&history_dir).unwrap();
        let legacy = r#"[{"timestamp":"2026-05-25T10:30:00+09:00","cleaner":"uv","freed_bytes":500,"skipped_count":0}]"#;
        std::fs::write(history_dir.join("history.json"), legacy).unwrap();
        let loaded = load_history(&history_dir.join("history.json"));
        assert_eq!(loaded.len(), 1);
        assert!(!loaded[0].moved_to_trash, "legacy entries default to false");
    }

    #[test]
    fn stats_table_shows_trash_and_delete_methods() {
        let entries = vec![
            HistoryEntry {
                timestamp: "2026-05-25T10:30:00+09:00".to_string(),
                cleaner: "uv".to_string(),
                freed_bytes: 100,
                skipped_count: 0,
                moved_to_trash: true,
            },
            HistoryEntry {
                timestamp: "2026-05-24T10:30:00+09:00".to_string(),
                cleaner: "brew".to_string(),
                freed_bytes: 200,
                skipped_count: 0,
                moved_to_trash: false,
            },
        ];
        let summary = compute_stats(&entries);
        let stats = format_stats(&summary);
        assert!(stats.contains("Method"), "stats: {stats}");
        assert!(stats.contains("trash"), "stats: {stats}");
        assert!(stats.contains("delete"), "stats: {stats}");
    }

    #[test]
    fn compute_stats_empty() {
        let summary = compute_stats(&[]);
        assert_eq!(summary.total_freed, 0);
        assert_eq!(summary.run_count, 0);
        assert!(summary.entries.is_empty());
    }

    #[test]
    fn compute_stats_aggregation() {
        let entries = vec![
            HistoryEntry {
                timestamp: "2026-05-25T10:30:00+09:00".to_string(),
                cleaner: "uv".to_string(),
                freed_bytes: 500_000_000,
                skipped_count: 0,
                moved_to_trash: false,
            },
            HistoryEntry {
                timestamp: "2026-05-24T22:15:00+09:00".to_string(),
                cleaner: "brew".to_string(),
                freed_bytes: 1_200_000_000,
                skipped_count: 1,
                moved_to_trash: false,
            },
        ];
        let summary = compute_stats(&entries);
        assert_eq!(summary.total_freed, 1_700_000_000);
        assert_eq!(summary.run_count, 2);
        assert_eq!(summary.entries.len(), 2);
        assert_eq!(summary.entries[0].cleaner, "uv");
    }

    #[test]
    fn format_stats_output() {
        let entries = vec![HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 500_000_000,
            skipped_count: 0,
            moved_to_trash: false,
        }];
        let summary = compute_stats(&entries);
        let output = format_stats(&summary);
        assert!(output.contains("sasurahime Statistics"));
        assert!(output.contains("Total freed"));
        assert!(output.contains("Runs:"));
        assert!(output.contains("uv"));
    }

    #[test]
    fn load_corrupted_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("history.json");
        fs::write(&path, "this is not valid json").unwrap();
        let loaded = load_history(&path);
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_missing_file_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.json");
        let loaded = load_history(&path);
        assert!(loaded.is_empty());
    }

    #[test]
    fn append_multiple_entries() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");

        let e1 = HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 500_000_000,
            skipped_count: 0,
            moved_to_trash: false,
        };
        let e2 = HistoryEntry {
            timestamp: "2026-05-24T22:15:00+09:00".to_string(),
            cleaner: "brew".to_string(),
            freed_bytes: 1_200_000_000,
            skipped_count: 1,
            moved_to_trash: false,
        };
        append_history(&e1, &history_dir).unwrap();
        append_history(&e2, &history_dir).unwrap();

        let history_path = history_dir.join("history.json");
        let loaded = load_history(&history_path);
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].cleaner, "uv");
        assert_eq!(loaded[1].cleaner, "brew");
    }

    #[test]
    fn format_stats_last_truncates() {
        let entries = vec![
            HistoryEntry {
                timestamp: "2026-05-25T10:30:00+09:00".to_string(),
                cleaner: "uv".to_string(),
                freed_bytes: 500_000_000,
                skipped_count: 0,
                moved_to_trash: false,
            },
            HistoryEntry {
                timestamp: "2026-05-24T22:15:00+09:00".to_string(),
                cleaner: "brew".to_string(),
                freed_bytes: 1_200_000_000,
                skipped_count: 1,
                moved_to_trash: false,
            },
        ];
        let summary = compute_stats(&entries);
        let output = format_stats_last(&summary, 1);
        assert!(output.contains("1.6 GB"));
        assert!(output.contains("Runs:"));
        let uv_lines: Vec<&str> = output.lines().filter(|l| l.contains("uv")).collect();
        assert_eq!(uv_lines.len(), 1, "only 1 entry should be shown");
    }

    #[test]
    fn compute_stats_entries_sorted_by_timestamp_desc() {
        let entries = vec![
            HistoryEntry {
                timestamp: "2026-05-24T22:15:00+09:00".to_string(),
                cleaner: "old".to_string(),
                freed_bytes: 100,
                skipped_count: 0,
                moved_to_trash: false,
            },
            HistoryEntry {
                timestamp: "2026-05-26T10:00:00+09:00".to_string(),
                cleaner: "newest".to_string(),
                freed_bytes: 200,
                skipped_count: 0,
                moved_to_trash: false,
            },
            HistoryEntry {
                timestamp: "2026-05-25T10:30:00+09:00".to_string(),
                cleaner: "middle".to_string(),
                freed_bytes: 300,
                skipped_count: 0,
                moved_to_trash: false,
            },
        ];
        let summary = compute_stats(&entries);
        assert_eq!(summary.entries[0].cleaner, "newest");
        assert_eq!(summary.entries[1].cleaner, "middle");
        assert_eq!(summary.entries[2].cleaner, "old");
    }

    #[test]
    fn format_stats_uses_unicode_by_default() {
        crate::format::set_use_unicode(true);
        let entries = vec![HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 500_000_000,
            skipped_count: 0,
            moved_to_trash: false,
        }];
        let summary = compute_stats(&entries);
        let output = format_stats(&summary);
        assert!(
            output.contains('╔'),
            "default should use Unicode box-drawing: {output}"
        );
        assert!(
            output.contains('║'),
            "default should use Unicode box-drawing: {output}"
        );
    }

    #[test]
    fn format_stats_no_unicode_uses_ascii() {
        crate::format::set_use_unicode(false);
        let entries = vec![HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 500_000_000,
            skipped_count: 0,
            moved_to_trash: false,
        }];
        let summary = compute_stats(&entries);
        let output = format_stats(&summary);
        assert!(
            output.contains('+'),
            "--no-unicode should use ASCII box-drawing: {output}"
        );
        assert!(
            !output.contains('╔'),
            "--no-unicode should NOT contain Unicode: {output}"
        );
        crate::format::set_use_unicode(true);
    }

    #[test]
    fn append_history_locks_a_dedicated_never_renamed_lock_file() {
        // The lock must live on a file the rename never replaces, so two
        // overlapping processes serialize on the same inode.
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");
        append_history(
            &HistoryEntry {
                timestamp: "2026-05-25T00:00:00+09:00".to_string(),
                cleaner: "uv".to_string(),
                freed_bytes: 100,
                skipped_count: 0,
                moved_to_trash: false,
            },
            &history_dir,
        )
        .unwrap();
        let lock_path = history_dir.join("history.json.lock");
        assert!(
            lock_path.exists(),
            "the exclusive lock must be a dedicated file, not history.json"
        );
    }

    #[test]
    fn concurrent_appends_all_land() {
        // 8 processes x 5 entries: the old truncate-before-lock + renamed-inode
        // lock lost entries under overlap; every entry must land.
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");
        std::fs::create_dir_all(&history_dir).unwrap();
        let dir = history_dir.clone();
        let handles: Vec<_> = (0..8)
            .map(|t| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    for i in 0..5 {
                        append_history(
                            &HistoryEntry {
                                timestamp: format!("2026-05-25T00:0{t}:0{i}+09:00"),
                                cleaner: format!("cleaner-{t}"),
                                freed_bytes: 100,
                                skipped_count: 0,
                                moved_to_trash: false,
                            },
                            &dir,
                        )
                        .unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        let loaded = load_history(&history_dir.join("history.json"));
        assert_eq!(
            loaded.len(),
            40,
            "every concurrent append must land in history"
        );
    }

    #[test]
    fn append_history_truncates_to_max_entries() {
        let tmp = TempDir::new().unwrap();
        let history_dir = tmp.path().join(".local/share/sasurahime");
        // Push more entries than MAX_HISTORY_ENTRIES
        for i in 0..MAX_HISTORY_ENTRIES + 10 {
            let ts = format!("2026-05-25T{:02}:00:00+09:00", i % 24);
            append_history(
                &HistoryEntry {
                    timestamp: ts,
                    cleaner: "uv".to_string(),
                    freed_bytes: 100,
                    skipped_count: 0,
                    moved_to_trash: false,
                },
                &history_dir,
            )
            .unwrap();
        }
        let history_path = history_dir.join("history.json");
        let loaded = load_history(&history_path);
        assert_eq!(loaded.len(), MAX_HISTORY_ENTRIES);
    }

    #[test]
    fn write_history_entry_with_normal_home_does_not_panic() {
        // Regression guard for the best-effort writer: append_history may fail
        // (fs2 lock errors on non-flock filesystems, permission errors, disk
        // full), but write_history_entry must never panic or propagate the
        // error. The failure is logged instead of silently dropped.
        assert!(
            dirs::home_dir().is_some(),
            "HOME must resolve in the test environment"
        );
        write_history_entry("probe", 1024, 0, false);
    }

    #[test]
    fn append_history_fails_on_unwritable_directory() {
        // Documents the error path write_history_entry must surface: a history
        // dir under a regular file cannot be created, so append_history must
        // return Err instead of pretending the entry was stored.
        let tmp = TempDir::new().unwrap();
        let blocker = tmp.path().join("blocker");
        fs::write(&blocker, "not a directory").unwrap();
        let history_dir = blocker.join("sasurahime");
        let entry = HistoryEntry {
            timestamp: "2026-05-25T10:30:00+09:00".to_string(),
            cleaner: "uv".to_string(),
            freed_bytes: 100,
            skipped_count: 0,
            moved_to_trash: false,
        };
        let result = append_history(&entry, &history_dir);
        assert!(
            result.is_err(),
            "append_history must fail when the history dir cannot be created"
        );
    }
}
