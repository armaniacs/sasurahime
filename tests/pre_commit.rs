use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_pre_commit_dry_run_does_not_delete() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/pre-commit");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("hook.pck"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "pre-commit", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(cache.exists(), "dry-run must not delete");
    assert!(
        cache.join("hook.pck").exists(),
        "dry-run must not delete files"
    );
}

#[test]
fn clean_pre_commit_fallback_deletes_cache_dir() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/pre-commit");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("hook.pck"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "pre-commit"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !cache.exists(),
        "cache dir should be removed in fallback mode"
    );
}

#[test]
fn clean_pre_commit_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "pre-commit"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_pre_commit_rejects_unsafe_pre_commit_home() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/pre-commit");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("hook.pck"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .env("PRE_COMMIT_HOME", "/")
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "pre-commit"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // fallback should have cleaned the default cache dir
    assert!(
        !cache.exists(),
        "cache dir should be removed in fallback mode"
    );
}

#[test]
fn clean_pre_commit_prefers_cli_when_available() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // Install fake pre-commit
    let script = bin_dir.join("pre-commit");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let cache = tmp.path().join(".cache/pre-commit");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("hook.pck"), b"dummy").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "pre-commit"])
        .output()
        .unwrap();

    assert!(output.status.success());
}
