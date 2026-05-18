use assert_cmd::Command;
use filetime::FileTime;
use std::fs;
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn write_aged(path: &std::path::Path, days_old: u64) {
    fs::write(path, b"log content").unwrap();
    let mtime = SystemTime::now() - Duration::from_secs(days_old * 86_400);
    filetime::set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
}

#[test]
fn clean_logs_removes_old_kilo_logs() {
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();

    write_aged(&log_dir.join("old.log"), 10);
    write_aged(&log_dir.join("recent.log"), 3);

    let output = sasurahime(tmp.path())
        .args(["clean", "logs"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !log_dir.join("old.log").exists(),
        "10-day-old log should be deleted"
    );
    assert!(
        log_dir.join("recent.log").exists(),
        "3-day-old log must remain"
    );
}

#[test]
fn clean_logs_keeps_dev_log() {
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();

    write_aged(&log_dir.join("dev.log"), 30);
    write_aged(&log_dir.join("old.log"), 30);

    let output = sasurahime(tmp.path())
        .args(["clean", "logs"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        log_dir.join("dev.log").exists(),
        "dev.log must never be deleted"
    );
    assert!(
        !log_dir.join("old.log").exists(),
        "old.log should be deleted"
    );
}

#[test]
fn clean_logs_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();
    write_aged(&log_dir.join("old.log"), 10);

    let output = sasurahime(tmp.path())
        .args(["clean", "logs", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        log_dir.join("old.log").exists(),
        "must not delete in dry-run"
    );
}

#[test]
fn clean_logs_keep_days_flag_overrides_default() {
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();

    write_aged(&log_dir.join("medium.log"), 10);
    write_aged(&log_dir.join("recent.log"), 3);

    let output = sasurahime(tmp.path())
        .args(["clean", "logs", "--keep-days", "5"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !log_dir.join("medium.log").exists(),
        "10d log must be deleted with --keep-days 5"
    );
    assert!(
        log_dir.join("recent.log").exists(),
        "3d log must remain with --keep-days 5"
    );
}

#[test]
fn clean_logs_missing_dirs_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "logs"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_logs_config_keep_days_used_when_no_flag() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "[logs]\nkeep_days = 3\n").unwrap();

    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();

    write_aged(&log_dir.join("five_days.log"), 5); // older than config keep_days=3
    write_aged(&log_dir.join("one_day.log"), 1);

    let output = sasurahime(tmp.path())
        .args(["clean", "logs"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !log_dir.join("five_days.log").exists(),
        "5d log must be deleted with config keep_days=3"
    );
    assert!(log_dir.join("one_day.log").exists(), "1d log must remain");
}
