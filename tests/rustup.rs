use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn fake_rustup(bin_dir: &Path, output: &str) {
    let script = format!("#!/bin/sh\necho '{}'", output);
    let path = bin_dir.join("rustup");
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn rustup_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "rustup"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}"
    );
}

#[test]
fn rustup_dry_run_shows_unused() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    fake_rustup(
        &bin_dir,
        "stable-aarch64-apple-darwin (default)\nnightly-2026-05-01-aarch64-apple-darwin\n",
    );

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "rustup", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nightly"), "must show nightly: {stdout}");
    assert!(
        !stdout.contains("stable"),
        "must NOT show active toolchain: {stdout}"
    );
}
