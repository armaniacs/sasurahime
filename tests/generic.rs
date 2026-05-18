use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

/// Installs a fake tool that appends its args to `bin_dir/calls_<name>.txt`.
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
fn clean_bun_calls_pm_cache_rm() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "bun");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "bun"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "bun");
    assert!(calls.contains("pm cache rm"), "expected 'pm cache rm', got: {calls}");
}

#[test]
fn clean_go_calls_clean_cache() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "go");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "go"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "go");
    assert!(calls.contains("clean -cache"), "expected 'clean -cache', got: {calls}");
}

#[test]
fn clean_pip_calls_cache_purge() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "pip");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "pip"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "pip");
    assert!(calls.contains("cache purge"), "expected 'cache purge', got: {calls}");
}

#[test]
fn clean_node_gyp_removes_cache_dir() {
    let tmp = TempDir::new().unwrap();
    let node_gyp = tmp.path().join(".cache/node-gyp");
    fs::create_dir_all(&node_gyp).unwrap();
    fs::write(node_gyp.join("dummy"), b"x").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "node-gyp"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!node_gyp.exists(), "node-gyp cache dir should be deleted");
}

#[test]
fn clean_caches_continues_past_missing_tools() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // Only install bun; go/pip/npm/yarn/pnpm are absent
    install_fake_tool(&bin_dir, "bun");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "caches"])
        .output()
        .unwrap();

    // Must exit 0 even though most tools are missing
    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "bun");
    assert!(calls.contains("pm cache rm"), "bun must still be called");
}

#[test]
fn clean_tool_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "bun"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found") || stdout.contains("skipping"));
}

#[test]
fn clean_bun_dry_run_does_not_invoke_tool() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "bun");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "bun", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !bin_dir.join("calls_bun.txt").exists(),
        "bun must not be invoked in dry-run"
    );
}
