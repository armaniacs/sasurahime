use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn put(home: &Path, rel: &str, bytes: usize) {
    let p = home.join("Library/Application Support").join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![1u8; bytes]).unwrap();
}

const OVER_THRESHOLD: usize = 65 * 1024 * 1024;

#[test]
fn dry_run_lists_cache_dirs_and_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    put(tmp.path(), "Slack/Local Storage/leveldb/keep.bin", 4096);
    put(tmp.path(), "Slack/IndexedDB/keep.bin", 4096);

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Slack/Cache"), "stdout:\n{stdout}");
    assert!(!stdout.contains("Local Storage"), "stdout:\n{stdout}");
    assert!(!stdout.contains("IndexedDB"), "stdout:\n{stdout}");
    let base = tmp.path().join("Library/Application Support/Slack");
    assert!(base.join("Cache/big.bin").exists());
    assert!(base.join("Local Storage/leveldb/keep.bin").exists());
    assert!(base.join("IndexedDB/keep.bin").exists());
}

#[test]
fn dry_run_without_application_support_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn non_interactive_clean_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a terminal"), "stderr:\n{stderr}");
    assert!(tmp
        .path()
        .join("Library/Application Support/Slack/Cache/big.bin")
        .exists());
}

#[test]
fn targets_lists_app_support_exactly_once() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let n = stdout
        .lines()
        .filter(|l| l.split_whitespace().next() == Some("app-support"))
        .count();
    assert_eq!(n, 1, "stdout:\n{stdout}");
}

#[test]
fn scan_includes_app_support() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("app-support"), "stdout:\n{stdout}");
}

#[test]
fn dry_run_shows_leftovers_without_deleting() {
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "ZzzNoSuchApp12345/data.bin", 2 * 1024 * 1024);
    let dir = tmp
        .path()
        .join("Library/Application Support/ZzzNoSuchApp12345");
    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(200 * 86_400));
    set_file_mtime(dir.join("data.bin"), old).unwrap();
    set_file_mtime(&dir, old).unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ZzzNoSuchApp12345"), "stdout:\n{stdout}");
    assert!(stdout.contains("display only"), "stdout:\n{stdout}");
    assert!(dir.join("data.bin").exists());
}

#[test]
fn non_interactive_clean_does_not_list_leftovers() {
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    put(tmp.path(), "ZzzNoSuchApp12345/data.bin", 2 * 1024 * 1024);
    let dir = tmp
        .path()
        .join("Library/Application Support/ZzzNoSuchApp12345");
    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(200 * 86_400));
    set_file_mtime(dir.join("data.bin"), old).unwrap();
    set_file_mtime(&dir, old).unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("ZzzNoSuchApp12345"));
    assert!(dir.join("data.bin").exists());
}
