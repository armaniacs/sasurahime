use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_huggingface_dry_run_does_not_delete() {
    let tmp = TempDir::new().unwrap();
    let hub = tmp.path().join(".cache/huggingface/hub");
    fs::create_dir_all(&hub).unwrap();
    fs::write(hub.join("model.bin"), b"dummy").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "huggingface", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(hub.exists(), "dry-run must not delete hub/");
    assert!(
        hub.join("model.bin").exists(),
        "dry-run must not delete files"
    );
}

#[test]
fn clean_huggingface_fallback_deletes_hub_contents() {
    let tmp = TempDir::new().unwrap();
    let hub = tmp.path().join(".cache/huggingface/hub");
    fs::create_dir_all(&hub).unwrap();
    fs::write(hub.join("model.bin"), b"dummy").unwrap();

    // Restrict PATH so huggingface-cli is not found (forces fallback path)
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "huggingface"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // hub/ should exist (recreated) but contents removed
    assert!(hub.exists(), "hub/ should be recreated");
    assert!(
        !hub.join("model.bin").exists(),
        "contents should be removed"
    );
}

#[test]
fn clean_huggingface_rejects_unsafe_hf_home_and_falls_back() {
    let tmp = TempDir::new().unwrap();
    // Create default hub/ so the fallback path has something to clean
    let hub = tmp.path().join(".cache/huggingface/hub");
    fs::create_dir_all(&hub).unwrap();
    fs::write(hub.join("model.bin"), b"dummy").unwrap();

    // HF_HOME=/etc is blocked by is_safe_delete_target → falls back to default
    let output = sasurahime(tmp.path())
        .env("HF_HOME", "/etc")
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "huggingface"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // fallback should have cleaned the default hub/ path
    assert!(hub.exists(), "hub/ should be recreated");
    assert!(
        !hub.join("model.bin").exists(),
        "contents should be removed via fallback"
    );
}

#[test]
fn clean_huggingface_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "huggingface"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_huggingface_prefers_cli_when_available() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // Install fake huggingface-cli
    let script = bin_dir.join("huggingface-cli");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let hub = tmp.path().join(".cache/huggingface/hub");
    fs::create_dir_all(&hub).unwrap();
    fs::write(hub.join("model.bin"), b"dummy").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "huggingface"])
        .output()
        .unwrap();

    assert!(output.status.success());
}
