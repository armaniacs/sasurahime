use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn create_large_log(home: &std::path::Path) {
    let dir = home.join("Library/Logs/BloatApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("crash.log"),
        vec![0u8; (100 * 1024 * 1024) + 1],
    )
    .unwrap();
}

fn create_small_log(home: &std::path::Path) {
    let dir = home.join("Library/Logs/SmallApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("info.log"), b"just a small entry").unwrap();
}

#[test]
fn clean_library_logs_dry_run_shows_entries() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());
    create_small_log(tmp.path());

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "library-logs", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BloatApp"), "stdout:\n{stdout}");
    assert!(
        !stdout.contains("SmallApp"),
        "SmallApp should not appear (no reason):\n{stdout}"
    );
}

#[test]
fn clean_library_logs_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_library_logs_all_deletes_entries() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let bloat = tmp.path().join("Library/Logs/BloatApp");
    assert!(!bloat.exists(), "BloatApp should be deleted");
}

#[test]
fn scan_shows_library_logs_in_scan_output() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("library-logs"),
        "scan output should include library-logs:\n{stdout}"
    );
}
