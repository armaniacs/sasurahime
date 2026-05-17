use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

/// Creates a fake `uv` script that does nothing and exits 0.
fn install_fake_uv(bin_dir: &std::path::Path) {
    let script = bin_dir.join("uv");
    fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn clean_uv_dry_run_does_not_delete_old_indexes() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();

    // Prepend our fake bin dir to PATH
    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // Files must still exist after dry-run
    assert!(
        uv_cache.join("simple-v16").exists(),
        "simple-v16 was deleted during dry-run"
    );
    assert!(
        uv_cache.join("simple-v21").exists(),
        "simple-v21 was deleted during dry-run"
    );
}

#[test]
fn clean_uv_removes_old_simple_indexes() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v17")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "uv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !uv_cache.join("simple-v16").exists(),
        "simple-v16 should have been deleted"
    );
    assert!(
        !uv_cache.join("simple-v17").exists(),
        "simple-v17 should have been deleted"
    );
    assert!(
        uv_cache.join("simple-v21").exists(),
        "simple-v21 (newest) should remain"
    );
}

#[test]
fn clean_uv_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    // PATH with no uv binary
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "uv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found") || stdout.contains("skipping"));
}
