use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub cleaner: String,
    pub freed_bytes: u64,
    pub skipped_count: usize,
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
    let mut out = String::new();
    out.push('╔');
    for _ in 0..inner {
        out.push('═');
    }
    out.push('╗');
    out.push('\n');

    for line in lines {
        out.push('║');
        out.push_str(&format_inner_line(line, inner));
        out.push('║');
        out.push('\n');
    }

    out.push('╚');
    for _ in 0..inner {
        out.push('═');
    }
    out.push('╝');
    out.push('\n');

    out
}

fn table_header() -> String {
    format!(
        "  {:>3}  {:<18}  {:<15}  {:>10}",
        "#", "Date", "Cleaner", "Size"
    )
}

fn table_row(i: usize, ts: &str, cleaner: &str, size: &str) -> String {
    format!("  {:>3}  {:<18}  {:<15}  {:>10}", i, ts, cleaner, size)
}

pub fn append_history(entry: &HistoryEntry, history_dir: &Path) -> anyhow::Result<()> {
    let history_path = history_dir.join("history.json");
    let mut entries = if history_path.exists() {
        load_history(&history_path)
    } else {
        fs::create_dir_all(history_dir)?;
        Vec::new()
    };
    entries.push(entry.clone());

    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let json = serde_json::to_string_pretty(&entries)?;
    let tmp_path = history_dir.join("history.json.tmp");
    fs::write(&tmp_path, &json)?;
    fs::rename(&tmp_path, &history_path)?;

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
            eprintln!("Warning: history file corrupted, starting fresh.");
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
        output.push_str(&table_row(i + 1, ts, &entry.cleaner, &size));
        output.push('\n');
    }
}

pub fn write_history_entry(label: &str, freed_bytes: u64, skipped_count: usize) {
    if freed_bytes == 0 {
        return;
    }
    let home_str = std::env::var("HOME").unwrap_or_default();
    let home = std::path::Path::new(&home_str);
    let history_dir = home.join(".local/share/sasurahime");
    let entry = HistoryEntry {
        timestamp: chrono::Local::now()
            .format("%Y-%m-%dT%H:%M:%S%:z")
            .to_string(),
        cleaner: label.to_string(),
        freed_bytes,
        skipped_count,
    };
    let _ = append_history(&entry, &history_dir);
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
        };
        append_history(&entry, &history_dir).unwrap();
        let history_path = history_dir.join("history.json");
        assert!(history_path.exists());
        let loaded = load_history(&history_path);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].cleaner, "uv");
        assert_eq!(loaded[0].freed_bytes, 500_000_000);
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
            },
            HistoryEntry {
                timestamp: "2026-05-24T22:15:00+09:00".to_string(),
                cleaner: "brew".to_string(),
                freed_bytes: 1_200_000_000,
                skipped_count: 1,
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
        };
        let e2 = HistoryEntry {
            timestamp: "2026-05-24T22:15:00+09:00".to_string(),
            cleaner: "brew".to_string(),
            freed_bytes: 1_200_000_000,
            skipped_count: 1,
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
            },
            HistoryEntry {
                timestamp: "2026-05-24T22:15:00+09:00".to_string(),
                cleaner: "brew".to_string(),
                freed_bytes: 1_200_000_000,
                skipped_count: 1,
            },
        ];
        let summary = compute_stats(&entries);
        let output = format_stats_last(&summary, 1);
        assert!(
            output.contains("1,700,000,000")
                || output.contains("1.7 GB")
                || output.contains("1.6 GB")
        );
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
            },
            HistoryEntry {
                timestamp: "2026-05-26T10:00:00+09:00".to_string(),
                cleaner: "newest".to_string(),
                freed_bytes: 200,
                skipped_count: 0,
            },
            HistoryEntry {
                timestamp: "2026-05-25T10:30:00+09:00".to_string(),
                cleaner: "middle".to_string(),
                freed_bytes: 300,
                skipped_count: 0,
            },
        ];
        let summary = compute_stats(&entries);
        assert_eq!(summary.entries[0].cleaner, "newest");
        assert_eq!(summary.entries[1].cleaner, "middle");
        assert_eq!(summary.entries[2].cleaner, "old");
    }
}
