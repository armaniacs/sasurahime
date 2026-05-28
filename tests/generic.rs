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
}

use rstest::rstest;

#[rstest]
#[case("bun",          "/usr/bin:/bin", true)]
#[case("cocoa-pods",   "/usr/bin:/bin", true)]
#[case("colima",       "/usr/bin:/bin", true)]
#[case("conda",        "/usr/bin:/bin", true)]
#[case("deno",         "/usr/bin:/bin", true)]
#[case("docker",       "/usr/bin:/bin", true)]
#[case("flutter",      "/usr/bin:/bin", false)]
#[case("maven",        "/usr/bin:/bin", true)]
#[case("orbstack",     "/usr/bin:/bin", true)]
#[case("pipx",         "/usr/bin:/bin", true)]
#[case("poetry",       "/usr/bin:/bin", true)]
#[case("sbt",          "/usr/bin:/bin", false)]
#[case("simulator",    "/tmp",           true)]
#[case("terraform",    "/usr/bin:/bin", false)]
#[case("tree-sitter",  "/usr/bin:/bin", false)]
#[case("volta",        "/usr/bin:/bin", false)]
#[case("vscode-extensions", "/usr/bin:/bin", false)]
fn clean_tool_not_found_skips(#[case] tool: &str, #[case] path: &str, #[case] check_stdout: bool) {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", path)
        .args(["clean", tool])
        .output()
        .unwrap();
    assert!(output.status.success());
    if check_stdout {
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("skipping") || stdout.contains("not found"),
            "stdout: {stdout}"
        );
    }
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
    assert!(
        spm_dir.join("Package.resolved").exists(),
        "dry-run must not delete"
    );
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

#[test]
fn clean_colima_calls_prune_all() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "colima");
    fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
    fs::write(tmp.path().join(".colima/_lima/colima/dummy.img"), b"x").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "colima"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "colima");
    assert!(
        calls.contains("prune --all --force"),
        "expected 'prune --all --force', got: {calls}"
    );
}

#[test]
fn clean_colima_dry_run_does_not_invoke() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "colima");
    fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "colima", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !bin_dir.join("calls_colima.txt").exists(),
        "colima must not be invoked in dry-run"
    );
}



#[test]
fn scan_shows_colima_for_existing_dir() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
    fs::write(tmp.path().join(".colima/colima.yaml"), b"config").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("colima"),
        "scan output should include colima:\n{stdout}"
    );
}

#[test]
fn clean_terraform_rejects_unsafe_env_var() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("TF_PLUGIN_CACHE_DIR", "/")
        .args(["clean", "terraform"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_flutter_rejects_unsafe_env_var() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PUB_CACHE", "/")
        .args(["clean", "flutter"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_gem_calls_cleanup() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "gem");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "gem"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "gem");
    assert!(
        calls.contains("cleanup"),
        "expected 'cleanup', got: {calls}"
    );
}

#[test]
fn clean_bundle_calls_clean() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "bundle");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "bundle"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "bundle");
    assert!(
        calls.contains("clean"),
        "expected 'clean', got: {calls}"
    );
}

#[test]
fn clean_dotnet_calls_nuget_locals_clear() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "dotnet");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "dotnet"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "dotnet");
    assert!(
        calls.contains("nuget locals all --clear"),
        "expected 'nuget locals all --clear', got: {calls}"
    );
}
