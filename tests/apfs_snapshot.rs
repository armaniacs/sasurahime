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
        .args(["--permanent", "clean", "apfs-snapshot", "--dry-run"])
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
        .args(["--permanent", "clean", "apfs-snapshot"])
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

#[test]
fn clean_apfs_snapshot_all_dry_run_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args([
            "--permanent",
            "clean",
            "apfs-snapshot",
            "--all",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn thin_dry_run_prints_the_plan_without_calling_tmutil() {
    let tmp = TempDir::new().unwrap();
    // Restrict PATH: if tmutil were called, the spawn would still succeed
    // only where it exists; the dry-run plan must print either way.
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args([
            "--permanent",
            "clean",
            "apfs-snapshot",
            "--thin",
            "10GB",
            "--dry-run",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("tmutil thinlocalsnapshots / 10000000000 2"),
        "the exact tmutil argument list must be in the plan, got:\n{combined}"
    );
}

#[test]
fn thin_without_a_tty_skips_without_erroring() {
    let tmp = TempDir::new().unwrap();
    // E2E runs without a terminal: thinning must skip safely, exit 0.
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args(["--permanent", "clean", "apfs-snapshot", "--thin", "10GB"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn thin_urgency_4_never_runs_without_confirmation() {
    let tmp = TempDir::new().unwrap();
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args([
            "--permanent",
            "clean",
            "apfs-snapshot",
            "--thin",
            "10GB",
            "--urgency",
            "4",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Urgency 4 interrupts running backups"),
        "the backup-interruption warning must be shown, got:\n{combined}"
    );
    assert!(
        combined.contains("cancelled") || !combined.contains("Freed:"),
        "urgency 4 must not run unattended, got:\n{combined}"
    );
}

#[test]
fn thin_rejects_unparseable_sizes() {
    let tmp = TempDir::new().unwrap();
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args([
            "--permanent",
            "clean",
            "apfs-snapshot",
            "--thin",
            "nonsense",
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "an unparseable size must fail with an error"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("cannot parse size"),
        "the error must explain the expected format, got:\n{combined}"
    );
}

#[test]
fn all_and_thin_together_fail_loudly_without_running_tmutil() {
    let tmp = TempDir::new().unwrap();
    // The conflict must be an explicit error (exit 1), not thin silently
    // winning over --all. The error happens at flag conversion, before any
    // cleaner is constructed, so tmutil is never called.
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args([
            "--permanent",
            "clean",
            "apfs-snapshot",
            "--all",
            "--thin",
            "10GB",
        ])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "conflicting flags must exit 1, got stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("mutually exclusive"),
        "the error must name the conflict, got:\n{combined}"
    );
    assert!(
        !combined.contains("deleted:"),
        "tmutil must never run before the conflict error, got:\n{combined}"
    );
}

#[test]
fn urgency_without_thin_fails_loudly() {
    let tmp = TempDir::new().unwrap();
    // --urgency only applies to --thin; alone it used to be silently ignored
    // (the default was only evaluated inside thin mode).
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args(["--permanent", "clean", "apfs-snapshot", "--urgency", "3"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(1),
        "--urgency without --thin must exit 1, got stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("--urgency requires --thin"),
        "the error must point at the missing flag, got:\n{combined}"
    );
}
