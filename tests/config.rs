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

#[test]
fn exclude_removes_cleaner_from_scan() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "exclude = [\"act\"]\n").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("act"),
        "excluded cleaner 'act' should not appear in scan output, got:\n{stdout}"
    );
}

#[test]
fn exclude_maintains_independent_cleaner() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "exclude = [\"uv\"]\n").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("uv"),
        "excluded cleaner 'uv' should not appear in scan output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("brew"),
        "non-excluded cleaner 'brew' should still appear, got:\n{stdout}"
    );
}

#[test]
fn exclude_does_not_block_direct_clean() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "exclude = [\"act\"]\n").unwrap();

    let output = sasurahime(tmp.path())
        .arg("clean")
        .arg("act")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "direct clean of excluded target 'act' should still work, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn custom_config_path_overrides_default() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "not valid toml :::").unwrap();
    let custom_path = tmp.path().join("my-config.toml");
    fs::write(&custom_path, "trash_mode = false\n").unwrap();

    let output = sasurahime(tmp.path())
        .arg("--config")
        .arg(&custom_path)
        .arg("scan")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "custom config path should override default (invalid) config, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn custom_target_appears_in_scan() {
    let tmp = TempDir::new().unwrap();
    // Create a custom cache dir with files
    let custom_path = tmp.path().join("tmp").join("cache");
    fs::create_dir_all(&custom_path).unwrap();
    fs::write(custom_path.join("data.bin"), b"content").unwrap();

    // Write config with [[custom]] entry pointing to it
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    let toml = format!(
        "[[custom]]\nname = \"my-workspace\"\npath = \"{path}\"\n",
        path = custom_path.to_string_lossy()
    );
    fs::write(config_dir.join("config.toml"), &toml).unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my-workspace"),
        "custom target should appear in scan output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("pruneable"),
        "custom target with content should show as pruneable, got:\n{stdout}"
    );
}

#[test]
fn custom_target_empty_dir_shows_not_found() {
    let tmp = TempDir::new().unwrap();
    // Write config with [[custom]] entry pointing to a non-existent path
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    let toml = "[[custom]]\nname = \"my-workspace\"\npath = \"~/tmp/nonexistent\"\n";
    fs::write(config_dir.join("config.toml"), toml).unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my-workspace"),
        "custom target should appear in scan output even when not found, got:\n{stdout}"
    );
    assert!(
        stdout.contains("not found"),
        "custom target without content should show as not found, got:\n{stdout}"
    );
}
