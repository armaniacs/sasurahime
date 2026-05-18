use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn missing_config_does_not_crash() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn invalid_config_exits_nonzero_with_message() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "not valid toml :::").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error loading config") || stderr.contains("config parse error"),
        "expected error message in stderr, got:\n{stderr}"
    );
}
