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
    assert!(
        calls.contains("pm cache rm"),
        "expected 'pm cache rm', got: {calls}"
    );
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
    assert!(
        calls.contains("clean -cache"),
        "expected 'clean -cache', got: {calls}"
    );
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
    assert!(
        calls.contains("cache purge"),
        "expected 'cache purge', got: {calls}"
    );
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
fn clean_deno_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "deno"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_pipx_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "pipx"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_docker_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "docker"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_orbstack_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "orbstack"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_cocoapods_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "cocoa-pods"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_conda_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "conda"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_poetry_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "poetry"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}");
}

#[test]
fn clean_spm_cache_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let spm_dir = tmp.path().join("Library/Caches/org.swift.swiftpm");
    std::fs::create_dir_all(&spm_dir).unwrap();
    std::fs::write(spm_dir.join("Package.resolved"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "spm", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(spm_dir.join("Package.resolved").exists(),
        "dry-run must not delete");
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

#[test]
fn trash_dry_run_shows_size() {
    let tmp = TempDir::new().unwrap();
    let trash = tmp.path().join(".Trash");
    std::fs::create_dir_all(&trash).unwrap();
    std::fs::write(trash.join("old-file.txt"), b"x".repeat(1024)).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "trash", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn downloads_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let dl = tmp.path().join("Downloads");
    std::fs::create_dir_all(&dl).unwrap();
    std::fs::write(dl.join("readme.pdf"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "downloads", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(dl.join("readme.pdf").exists(), "dry-run must not delete");
}
