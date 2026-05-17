use assert_cmd::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn scan_exits_zero_and_shows_categories() {
    let tmp = TempDir::new().unwrap();

    let output = sasurahime(tmp.path())
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success(), "exit code was not 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("uv"), "expected 'uv' in output, got:\n{stdout}");
    assert!(stdout.contains("brew"), "expected 'brew' in output, got:\n{stdout}");
}

#[test]
fn scan_shows_not_found_for_missing_dirs() {
    let tmp = TempDir::new().unwrap();
    // Do NOT create any cache dirs in tmp

    let output = sasurahime(tmp.path())
        .arg("scan")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found"), "expected 'not found' in output:\n{stdout}");
}
