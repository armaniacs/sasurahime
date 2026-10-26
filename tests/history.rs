use assert_cmd::Command;
use std::fs;
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
