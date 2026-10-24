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
        .args(["--permanent", "clean", "act"])
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
        .args(["--permanent", "clean", "act", "--dry-run"])
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
        .args(["--permanent", "clean", "act"])
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
        .args(["--permanent", "clean", "act"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    // Real observation of the rejection: the resolver names the rejected
    // override on stderr and falls back to the default. A tripwire-less
    // build would target / instead and never print this.
    assert!(
        stderr.contains("[act] WARNING: ACT_CACHE_DIR=/ points to an unsafe path, using default"),
        "the rejected override must be named on stderr:\n{stderr}"
    );
}

#[test]
fn clean_act_rejects_usr_path() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("ACT_CACHE_DIR", "/usr/local")
        .args(["--permanent", "clean", "act"])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    // Real observations of the rejection: the override is named on stderr
    // and /usr/local survives the run — a tripwire-less build would delete
    // it as the resolved cache dir.
    assert!(
        stderr.contains(
            "[act] WARNING: ACT_CACHE_DIR=/usr/local points to an unsafe path, using default"
        ),
        "the rejected override must be named on stderr:\n{stderr}"
    );
    assert!(
        std::path::Path::new("/usr/local").exists(),
        "/usr/local must survive the rejected override"
    );
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
        .args(["--permanent", "clean", "act"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "must not fail on unsafe symlink: {stderr}"
    );
    // Real observations of the rejection: the symlinked override is named on
    // stderr and the symlink itself survives — a tripwire-less build would
    // resolve it as the cache dir and remove it.
    assert!(
        stderr.contains("WARNING: ACT_CACHE_DIR="),
        "the rejected symlink override must be named on stderr:\n{stderr}"
    );
    assert!(
        stderr.contains("points to an unsafe path, using default"),
        "the rejection must say the path is unsafe:\n{stderr}"
    );
    assert!(
        symlink.exists(),
        "the rejected symlink must survive the run"
    );
}

#[test]
fn clean_act_rejects_system_path_env_var() {
    let tmp = TempDir::new().unwrap();
    // Set ACT_CACHE_DIR to a system path — safety guard should reject
    let output = sasurahime(tmp.path())
        .env("ACT_CACHE_DIR", "/System/Library")
        .args(["--permanent", "clean", "act"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    // Real observations of the rejection: the override is named on stderr
    // and the SIP-protected system path survives the run — a tripwire-less
    // build would target it as the resolved cache dir.
    assert!(
        stderr.contains(
            "[act] WARNING: ACT_CACHE_DIR=/System/Library points to an unsafe path, using default"
        ),
        "the rejected override must be named on stderr:\n{stderr}"
    );
    assert!(
        std::path::Path::new("/System/Library").exists(),
        "/System/Library must survive the rejected override"
    );
}
