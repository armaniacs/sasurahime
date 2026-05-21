use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn create_log_app(home: &std::path::Path, name: &str, size: u64) {
    let dir = home.join("Library/Logs").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash.log"), vec![0u8; (size as usize) + 1]).unwrap();
}

fn create_large_log(home: &std::path::Path) {
    let dir = home.join("Library/Logs/BloatApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash.log"), vec![0u8; (100 * 1024 * 1024) + 1]).unwrap();
}

#[test]
fn suppress_flag_hides_eta() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--suppress", "clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // --suppress should not show progress bar artifacts like "ETA"
    assert!(
        !stdout.contains("ETA"),
        "suppress should hide ETA:\n{stdout}"
    );
    // Should still show Freed line
    assert!(
        stdout.contains("Freed:"),
        "suppress should show Freed:\n{stdout}"
    );
}

#[test]
fn deep_suppress_flag_hides_freed_line() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--deep-suppress", "clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // deep-suppress should not show the Freed line from run_clean_target
    assert!(
        !stdout.contains("Freed:"),
        "deep-suppress should hide Freed:\n{stdout}"
    );
}

#[test]
fn default_shows_freed_line() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Freed:"),
        "default should show Freed:\n{stdout}"
    );
}

#[test]
fn config_suppress_hides_progress_bar() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "suppress = true\n").unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("ETA"),
        "config suppress should hide ETA:\n{stdout}"
    );
    assert!(
        stdout.contains("Freed:"),
        "config suppress should show Freed:\n{stdout}"
    );
}

#[test]
fn config_deep_suppress_hides_freed_line() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "deep_suppress = true\n").unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Freed:"),
        "config deep-suppress should hide Freed:\n{stdout}"
    );
}

#[test]
fn cli_suppress_stacks_with_config_suppress() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "suppress = true\n").unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--suppress", "clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("ETA"),
        "CLI+config suppress should hide ETA:\n{stdout}"
    );
    assert!(
        stdout.contains("Freed:"),
        "CLI+config suppress should show Freed:\n{stdout}"
    );
}

#[test]
fn dry_run_shows_per_entry_output() {
    let tmp = TempDir::new().unwrap();
    create_log_app(tmp.path(), "MyApp", 100 * 1024 * 1024);

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("would remove"),
        "dry-run must show per-entry listing:\n{stdout}"
    );
    assert!(
        stdout.contains("MyApp"),
        "dry-run must show entry name:\n{stdout}"
    );
}

#[test]
fn dry_run_with_suppress_still_shows_listing() {
    let tmp = TempDir::new().unwrap();
    create_log_app(tmp.path(), "MyApp", 100 * 1024 * 1024);

    let output = sasurahime(tmp.path())
        .args(["--suppress", "clean", "library-logs", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("would remove"),
        "suppress + dry-run must still show listing:\n{stdout}"
    );
}

#[test]
fn clean_all_with_progress_succeeds() {
    let tmp = TempDir::new().unwrap();
    let log_dir = tmp.path().join("Library/Logs/BloatApp");
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(
        log_dir.join("crash.log"),
        vec![0u8; (100 * 1024 * 1024) + 1],
    )
    .unwrap();

    let log_dir2 = tmp.path().join("Library/Logs/OldApp");
    fs::create_dir_all(&log_dir2).unwrap();
    fs::write(
        log_dir2.join("debug.log"),
        vec![0u8; (100 * 1024 * 1024) + 1],
    )
    .unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(!log_dir.exists(), "BloatApp should be deleted");
    assert!(!log_dir2.exists(), "OldApp should be deleted");

    assert!(
        stdout.contains("Freed:"),
        "clean_all should show Freed:\n{stdout}"
    );
}
