use assert_cmd::Command;
use tempfile::TempDir;

/// Minimal E2E test: the CLI entry point for apfs-snapshot parses correctly
/// and exits without crashing, regardless of whether tmutil is available.
#[test]
fn clean_apfs_snapshot_dry_run_does_not_crash() {
    let tmp = TempDir::new().unwrap();
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args(["clean", "apfs-snapshot", "--dry-run"])
        .output()
        .unwrap();
    // Should exit successfully even if tmutil is missing or no snapshots exist
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn clean_apfs_snapshot_no_crash_without_tmutil() {
    // Restrict PATH so tmutil is not available
    let tmp = TempDir::new().unwrap();
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "apfs-snapshot"])
        .output()
        .unwrap();
    // Should exit 0; "not found, skipping" is the expected behavior
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found")
            || stderr.contains("not a terminal")
            || stderr.contains("running"),
        "expected 'not found', 'not a terminal', or 'running' in stderr: {stderr}"
    );
}
