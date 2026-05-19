use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_act_removes_cache_dir() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("action.tar.gz"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "act"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!act_cache.exists(), "act cache dir should be deleted");
}

#[test]
fn clean_act_dry_run_does_not_delete() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("action.tar.gz"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "act", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        act_cache.join("action.tar.gz").exists(),
        "dry-run must not delete files"
    );
}

#[test]
fn clean_act_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    // No .cache/act directory at all

    let output = sasurahime(tmp.path())
        .args(["clean", "act"])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn clean_act_rejects_unsafe_env_var_path() {
    let tmp = TempDir::new().unwrap();
    // Set ACT_CACHE_DIR to root — the safety guard should reject it and fall back
    // to the default ~/.cache/act, which doesn't exist in this test.
    let output = sasurahime(tmp.path())
        .env("ACT_CACHE_DIR", "/")
        .args(["clean", "act"])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn clean_act_rejects_usr_path() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("ACT_CACHE_DIR", "/usr/local")
        .args(["clean", "act"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_act_rejects_symlink_to_system_path() {
    let tmp = TempDir::new().unwrap();
    let symlink = tmp.path().join("evil-link");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc", &symlink).unwrap();
    }
    let output = sasurahime(tmp.path())
        .env("ACT_CACHE_DIR", &symlink)
        .args(["clean", "act"])
        .output()
        .unwrap();

    assert!(output.status.success(), "must not fail on unsafe symlink");
}

#[test]
fn clean_act_rejects_system_path_env_var() {
    let tmp = TempDir::new().unwrap();
    // Set ACT_CACHE_DIR to a system path — safety guard should reject
    let output = sasurahime(tmp.path())
        .env("ACT_CACHE_DIR", "/System/Library")
        .args(["clean", "act"])
        .output()
        .unwrap();

    assert!(output.status.success());
}
