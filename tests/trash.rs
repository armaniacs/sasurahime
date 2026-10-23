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
fn dry_run_with_default_trash_mode_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let cache = tmp.path().join(".cache/uv/simple-v16");
    fs::create_dir_all(&cache).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    // Default: trash mode on. --dry-run must prevent any action.
    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "uv", "--dry-run"])
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
fn yes_with_empty_dir_exits_cleanly() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn yes_permanent_requires_confirmation() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("archive-v0")).unwrap();
    fs::write(uv_cache.join("archive-v0/some_pack"), [0u8; 100]).unwrap();

    let stdin_file = tmp.path().join("empty_stdin.txt");
    fs::write(&stdin_file, "").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{}", bin_dir.display(), original_path))
        .args(["--permanent", "--yes"])
        .pipe_stdin(&stdin_file)
        .unwrap()
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // With --permanent --yes and empty stdin (no "y"), the confirmation prompt rejects.
    // The output contains either "Are you sure" or "Aborted".
    assert!(
        stdout.contains("Are you sure") || stdout.contains("Aborted"),
        "should show confirmation prompt then abort without 'y':\n{stdout}"
    );
}

#[test]
fn trash_mode_with_permanent_cleaner_does_not_claim_trash() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();
    fs::write(uv_cache.join("simple-v16/pack.seq"), [0u8; 100]).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "uv"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    // PBI-20: the old-index deletion goes through the safe_delete seam, so
    // trash mode is honored. Which branch ran depends on the trash backend:
    // the stale dir still existing after the run is the branch discriminator
    // (a TCC-blocked backend makes the seam classify the failure skippable).
    if uv_cache.join("simple-v16").exists() {
        // Skip branch: the delete failed with a skippable error, nothing was
        // freed and the clean reports exit 1.
        assert_eq!(
            output.status.code(),
            Some(1),
            "a skipped delete must exit 1:\nstdout: {stdout}\nstderr: {stderr}"
        );
        assert!(stdout.contains("Freed: 0 B"), "stdout:\n{stdout}");
    } else {
        // Trash-truth branch: the Trash move succeeded, the source is gone
        // and the freed line reports the trash truth, not a permanent delete.
        assert!(
            output.status.success(),
            "stdout: {stdout}\nstderr: {stderr}"
        );
        assert!(stdout.contains("Logical freed:"), "stdout:\n{stdout}");
    }
    // In BOTH branches the freed line must not claim a Trash move.
    assert!(
        !stdout.contains("moved to Trash"),
        "the freed line must not claim a Trash move:\n{stdout}"
    );
}

#[test]
fn clean_with_permanent_flag_does_not_show_trash_note() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();
    fs::write(uv_cache.join("simple-v16/pack.seq"), [0u8; 100]).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--permanent", "clean", "uv"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(
        stdout.contains("Freed:"),
        "--permanent should show Freed:\n{stdout}"
    );
    assert!(
        !stdout.contains("moved to Trash"),
        "--permanent must not mention Trash:\n{stdout}"
    );
}

#[test]
fn clean_dry_run_does_not_show_trash_note() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();
    fs::write(uv_cache.join("simple-v16/pack.seq"), [0u8; 100]).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}",
    );
    assert!(
        !stdout.contains("moved to Trash"),
        "dry-run should not show trash note:\n{stdout}"
    );
    assert!(
        uv_cache.join("simple-v16").exists(),
        "dry-run must preserve files"
    );
}

#[test]
fn permanent_dry_run_shows_freed_not_trash() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();
    fs::write(uv_cache.join("simple-v16/pack.seq"), [0u8; 100]).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{}", bin_dir.display(), original_path))
        .args(["--permanent", "clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    // --permanent mode: should show "Freed:" not "moved to Trash"
    assert!(stdout.contains("Freed:"), "stdout:\n{stdout}");
    assert!(
        !stdout.contains("Trash"),
        "--permanent should bypass Trash:\n{stdout}"
    );
}

#[test]
fn config_trash_mode_false_disables_trash() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "trash_mode = false\n").unwrap();

    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_uv(&bin_dir);

    let uv_cache = tmp.path().join(".cache/uv");
    fs::create_dir_all(uv_cache.join("simple-v16")).unwrap();
    fs::create_dir_all(uv_cache.join("simple-v21")).unwrap();
    fs::write(uv_cache.join("simple-v16/pack.seq"), [0u8; 100]).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{}", bin_dir.display(), original_path))
        .args(["clean", "uv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // config trash_mode=false disables Trash → show "Freed:" not "moved to Trash"
    assert!(
        stdout.contains("Freed:"),
        "config trash_mode=false should bypass Trash:\n{stdout}"
    );
    assert!(
        !stdout.contains("moved to Trash"),
        "config trash_mode=false must not mention Trash:\n{stdout}"
    );
}
