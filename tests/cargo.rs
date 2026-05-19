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
fn cargo_detect_reports_registry_cache_size() {
    let tmp = TempDir::new().unwrap();
    let reg = tmp.path().join(".cargo/registry/cache/index.crates.io-xxx");
    fs::create_dir_all(&reg).unwrap();
    fs::write(reg.join("dummy.crate"), b"x".repeat(4096)).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "cargo", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("registry"), "stdout: {stdout}");
}

#[test]
fn cargo_dry_run_does_not_delete() {
    let tmp = TempDir::new().unwrap();
    let reg = tmp.path().join(".cargo/registry/cache/pkg");
    fs::create_dir_all(&reg).unwrap();
    fs::write(reg.join("dummy.crate"), b"x").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "cargo", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        reg.join("dummy.crate").exists(),
        "dry-run must not delete"
    );
}
