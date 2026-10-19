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
fn clean_apfs_snapshot_restricted_path_does_not_crash() {
    // Restrict PATH; tmutil may or may not be reachable from it
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
    // Snapshot count decides the message: none -> stdout, some -> stderr
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("not found")
            || combined.contains("not a terminal")
            || combined.contains("running")
            || combined.contains("no local snapshots found"),
        "expected a skip/no-snapshot message in output: {combined}"
    );
}
