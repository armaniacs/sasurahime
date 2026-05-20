use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

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
fn trash_flag_with_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let cache = tmp.path().join(".cache/uv/simple-v16");
    fs::create_dir_all(&cache).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--trash", "clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {stderr}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
    );
    assert!(cache.exists(), "--dry-run must prevent deletion/trashing");
}

#[test]
fn trash_clean_shows_moved_to_trash_message() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    // Create two simple-vN dirs so the cleaner deletes the older one.
    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();
    // Put some content in the old one so bytes_freed > 0
    fs::write(uv_cache.join("simple-v16/pack.seq"), [0u8; 100]).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--trash", "clean", "uv"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code(),
    );
    assert!(stdout.contains("moved to Trash"), "stdout:\n{stdout}\nstderr:\n{stderr}");
}
