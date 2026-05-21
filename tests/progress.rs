use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
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
