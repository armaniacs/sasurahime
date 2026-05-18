use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn install_fake_tool(bin_dir: &std::path::Path, name: &str) {
    let calls_file = bin_dir.join(format!("calls_{name}.txt"));
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
        calls_file.display()
    );
    let path = bin_dir.join(name);
    fs::write(&path, &script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn recorded_calls(bin_dir: &std::path::Path, name: &str) -> String {
    fs::read_to_string(bin_dir.join(format!("calls_{name}.txt"))).unwrap_or_default()
}

#[test]
fn clean_npm_calls_cache_clean_force() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "npm");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "npm"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "npm");
    assert!(calls.contains("cache clean"), "expected 'cache clean', got: {calls}");
    assert!(calls.contains("--force"), "expected '--force', got: {calls}");
}

#[test]
fn clean_yarn_calls_cache_clean() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "yarn");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "yarn"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "yarn");
    assert!(calls.contains("cache clean"), "expected 'cache clean', got: {calls}");
}

#[test]
fn clean_pnpm_calls_store_prune() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "pnpm");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "pnpm"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "pnpm");
    assert!(calls.contains("store prune"), "expected 'store prune', got: {calls}");
}

#[test]
fn clean_npm_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "npm"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found") || stdout.contains("skipping"));
}
