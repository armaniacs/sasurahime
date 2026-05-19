use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn gradle_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "gradle"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn gradle_keeps_highest_version() {
    let tmp = TempDir::new().unwrap();
    let caches = tmp.path().join(".gradle/caches");
    fs::create_dir_all(caches.join("8.10.1")).unwrap();
    fs::create_dir_all(caches.join("8.12.0")).unwrap();
    fs::create_dir_all(caches.join("8.8.0")).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "gradle", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("8.10.1") || stdout.contains("8.8.0"),
        "must show old versions: {stdout}");
}

#[test]
fn jetbrains_keeps_highest_per_ide() {
    let tmp = TempDir::new().unwrap();
    let jb = tmp.path().join("Library/Caches/JetBrains");
    fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
    fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
    fs::create_dir_all(jb.join("IntelliJIdea2025.1")).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "jetbrains", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("GoLand2024.2"), "old GoLand must be removed: {stdout}");
}
