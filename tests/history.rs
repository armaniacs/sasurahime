use assert_cmd::Command;
use std::fs;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

const TWO_ENTRIES: &str = r#"[
    {"timestamp":"2026-05-25T10:30:00+09:00","cleaner":"uv","freed_bytes":500000000,"skipped_count":0},
    {"timestamp":"2026-05-24T22:15:00+09:00","cleaner":"brew","freed_bytes":1200000000,"skipped_count":1}
]"#;

const THREE_ENTRIES: &str = r#"[
    {"timestamp":"2026-05-25T10:30:00+09:00","cleaner":"uv","freed_bytes":500000000,"skipped_count":0},
    {"timestamp":"2026-05-24T22:15:00+09:00","cleaner":"brew","freed_bytes":1200000000,"skipped_count":1},
    {"timestamp":"2026-05-23T18:00:00+09:00","cleaner":"xcode","freed_bytes":3500000000,"skipped_count":0}
]"#;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn write_history(home: &std::path::Path, entries: &str) {
    let dir = home.join(".local/share/sasurahime");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("history.json"), entries).unwrap();
}

fn assert_history_alias_matches_stats(home: &std::path::Path, extra_args: &[&str]) {
    let mut stats_args = vec!["stats"];
    stats_args.extend_from_slice(extra_args);
    let mut history_args = vec!["history"];
    history_args.extend_from_slice(extra_args);

    let stats_output = sasurahime(home).args(&stats_args).output().unwrap();
    let history_output = sasurahime(home).args(&history_args).output().unwrap();
    assert!(history_output.status.success());
    let stats_stdout = String::from_utf8_lossy(&stats_output.stdout);
    let history_stdout = String::from_utf8_lossy(&history_output.stdout);
    assert_eq!(stats_stdout, history_stdout);
}

#[test]
fn stats_shows_aggregated_results() {
    let tmp = TempDir::new().unwrap();
    write_history(tmp.path(), TWO_ENTRIES);
    let output = sasurahime(tmp.path()).args(["stats"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sasurahime Statistics"));
    assert!(stdout.contains("Total freed"));
    assert!(stdout.contains("Runs:"));
    assert!(stdout.contains("uv"));
    assert!(stdout.contains("brew"));
}

#[test]
fn stats_empty_shows_no_history_message() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).args(["stats"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No history yet"));
}

#[test]
fn stats_corrupted_history_does_not_crash() {
    let tmp = TempDir::new().unwrap();
    write_history(tmp.path(), "this is not valid json");
    let output = sasurahime(tmp.path()).args(["stats"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No history yet"));
}

#[test]
fn stats_last_n_filters() {
    let tmp = TempDir::new().unwrap();
    write_history(tmp.path(), THREE_ENTRIES);
    let output = sasurahime(tmp.path())
        .args(["stats", "--last", "2"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Total freed"));
    assert!(stdout.contains("Runs:"));
    let uv_count = stdout.matches("uv").count();
    let brew_count = stdout.matches("brew").count();
    let xcode_count = stdout.matches("xcode").count();
    assert_eq!(
        uv_count + brew_count + xcode_count,
        2,
        "Only 2 of 3 entries should appear"
    );
}

#[test]
fn history_alias_matches_stats_with_entries() {
    let tmp = TempDir::new().unwrap();
    write_history(tmp.path(), TWO_ENTRIES);
    assert_history_alias_matches_stats(tmp.path(), &[]);
}

#[test]
fn history_alias_matches_stats_empty() {
    let tmp = TempDir::new().unwrap();
    assert_history_alias_matches_stats(tmp.path(), &[]);
}

#[test]
fn history_alias_matches_stats_with_last_filter() {
    let tmp = TempDir::new().unwrap();
    write_history(tmp.path(), THREE_ENTRIES);
    assert_history_alias_matches_stats(tmp.path(), &["--last", "2"]);
}

#[test]
fn dry_run_does_not_record_history() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = bin_dir.join("uv");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v17")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--permanent", "clean", "uv", "--dry-run"])
        .assert()
        .success();

    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    assert!(!history_path.exists(), "dry-run must not write history");
}

#[test]
fn clean_creates_history_json() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = bin_dir.join("uv");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::write(uv_cache.join("simple-v16/pkg.tar.gz"), [0u8; 4096]).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v17")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--permanent", "clean", "uv"])
        .assert()
        .success();

    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    assert!(
        history_path.exists(),
        "history.json should exist after clean"
    );
    let content = fs::read_to_string(&history_path).unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["cleaner"], "uv");
    let freed = entries[0]["freed_bytes"].as_u64().unwrap();
    assert!(freed > 0, "freed_bytes should be > 0");
}

#[test]
fn permanent_clean_keeps_an_audit_trail_without_verbose() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = bin_dir.join("uv");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    // simple-v16 with data + simple-v17 empty: the uv cleaner removes the
    // stale simple-v16 directory for real (no Trash involved).
    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::write(uv_cache.join("simple-v16/pkg.tar.gz"), [0u8; 4096]).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v17")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--permanent", "clean", "uv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Permanently deleted: no Trash copy was kept."),
        "--permanent must keep a minimal audit trail without --verbose, got:\n{combined}"
    );

    // The record also lands in history.json as a delete (not trash).
    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    assert!(history_path.exists(), "audit trail must reach history.json");
    let raw = fs::read_to_string(&history_path).unwrap();
    assert!(
        raw.contains("\"moved_to_trash\": false"),
        "history must record the physical delete, got:\n{raw}"
    );
}

#[test]
fn clean_trash_shows_size_and_finder_guidance() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "trash"])
        .output()
        .unwrap();

    assert!(output.status.success(), "clean trash must exit 0");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Trash holds:"),
        "the Trash size must be shown, got:\n{combined}"
    );
    assert!(
        combined.contains("Finder") && combined.contains("empty"),
        "Finder emptying steps must be shown, got:\n{combined}"
    );
    assert!(
        combined.contains("open ~/.Trash") || combined.contains("~/.Trash"),
        "an open ~/.Trash pointer must be shown, got:\n{combined}"
    );
}

#[test]
fn clean_trash_dry_run_is_report_only_on_both_streams() {
    // Stream parity pin (PBI 2026-11-20-02, added before the flow
    // extraction): `clean trash --dry-run` is Report-only in both modes —
    // the arm deliberately ignores the dry-run flag (`let _ = dry_run;`,
    // sasurahime never empties the Trash itself), the size report streams
    // to stdout, the Finder guidance to stderr, and the run exits 0. The
    // Trash arm's extraction must not move either stream or the
    // flag-ignoring contract.
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "trash", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success(), "clean trash --dry-run must exit 0");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("Trash holds:"),
        "the Trash size report must stream to stdout, got stdout:\n{stdout}"
    );
    assert!(
        stderr.contains("Warning: sasurahime cannot safely empty ~/.Trash."),
        "the Finder guidance must stream to stderr, got stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("Trash holds:"),
        "the size report must not leak to stderr, got stderr:\n{stderr}"
    );
}

#[test]
fn stats_shows_trash_method_after_a_trash_clean() {
    let tmp = TempDir::new().unwrap();
    // A trash-mode clean of a PathCleaner target really moves files to the
    // Trash, which is an Apple-Event operation (TCC); run the trash-mode
    // detection through history.json instead by writing an entry with the
    // new field and checking stats renders it.
    write_history(
        tmp.path(),
        r#"[{"timestamp":"2026-05-25T10:30:00+09:00","cleaner":"uv","freed_bytes":4096,"skipped_count":0,"moved_to_trash":true}]"#,
    );
    let output = sasurahime(tmp.path()).args(["stats"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("trash"),
        "stats must surface the trash/physical distinction, got:\n{stdout}"
    );
    assert!(stdout.contains("Method"), "stats: {stdout}");
}

#[test]
fn written_history_entries_keep_the_rfc3339_local_offset_shape() {
    // Parity pin for the chrono → jiff timestamp swap: a freshly written
    // history row must keep the exact byte shape the chrono implementation
    // produced with "%Y-%m-%dT%H:%M:%S%:z" — a 19-char local stem plus a
    // colon-carrying ±HH:MM offset. The assert is wall-clock independent:
    // it checks character classes at fixed positions, never the actual
    // date or offset value.
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();
    let aged = log_dir.join("parity-pin.log");
    fs::write(&aged, b"log content").unwrap();
    let mtime = SystemTime::now() - Duration::from_secs(10 * 86_400);
    fs::File::options()
        .write(true)
        .open(&aged)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(mtime))
        .unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "logs"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    let content = fs::read_to_string(&history_path).unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
    assert_eq!(entries.len(), 1, "one logs run must record one entry");
    assert_eq!(entries[0]["cleaner"], "logs");
    assert!(
        entries[0]["freed_bytes"].as_u64().unwrap() > 0,
        "the aged log deletion must free > 0 bytes"
    );
    let ts = entries[0]["timestamp"].as_str().unwrap();
    assert_rfc3339_local_offset_shape(ts);
}

fn assert_rfc3339_local_offset_shape(ts: &str) {
    let b = ts.as_bytes();
    let digits_at = |idx: &[usize]| idx.iter().all(|&i| b[i].is_ascii_digit());
    // 19-char stem + sign + HH:MM.
    assert_eq!(ts.len(), 25, "YYYY-MM-DDTHH:MM:SS±HH:MM is 25 bytes: {ts}");
    assert!(
        digits_at(&[0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18, 20, 21, 23, 24]),
        "date, time and offset digits in the chrono shape: {ts}"
    );
    assert_eq!(b[4], b'-', "year-month separator: {ts}");
    assert_eq!(b[7], b'-', "month-day separator: {ts}");
    assert_eq!(b[10], b'T', "date-time separator: {ts}");
    assert_eq!(b[13], b':', "hour-minute separator: {ts}");
    assert_eq!(b[16], b':', "minute-second separator: {ts}");
    assert!(b[19] == b'+' || b[19] == b'-', "offset sign: {ts}");
    assert_eq!(b[22], b':', "offset must carry a colon: {ts}");
}
