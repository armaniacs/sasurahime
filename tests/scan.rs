use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn scan_exits_zero_and_shows_categories() {
    let tmp = TempDir::new().unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success(), "exit code was not 0");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("uv"),
        "expected 'uv' in output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("brew"),
        "expected 'brew' in output, got:\n{stdout}"
    );
    // Sprint 5 targets
    assert!(
        stdout.contains("act"),
        "expected 'act' in output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("huggingface"),
        "expected 'huggingface' in output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("pre-commit"),
        "expected 'pre-commit' in output, got:\n{stdout}"
    );
}

#[test]
fn scan_shows_pruneable_for_existing_cache() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy.tar.gz"), b"x").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("pruneable"),
        "expected 'pruneable' in output:\n{stdout}"
    );
    assert!(
        stdout.contains("act"),
        "expected 'act' in output:\n{stdout}"
    );
}

#[test]
fn scan_verbose_shows_target_column() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy.tar.gz"), b"x").unwrap();

    let output = sasurahime(tmp.path())
        .arg("scan")
        .arg("--verbose")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // The table header should include "Target"
    assert!(
        stdout.contains("Target"),
        "expected 'Target' column header in verbose output:\n{stdout}"
    );
    // The act cleaner's primary target (.cache/act) should appear
    assert!(
        stdout.contains(".cache/act"),
        "expected '.cache/act' in verbose output:\n{stdout}"
    );
}

#[test]
fn scan_with_dry_run_still_shows_scan_results() {
    let tmp = TempDir::new().unwrap();
    // Create a cache dir so some cleaners show as pruneable
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy.tar.gz"), b"x").unwrap();

    // --dry-run is a no-op for scan (scan is read-only) but must not error
    let output = sasurahime(tmp.path())
        .arg("scan")
        .arg("--dry-run")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Category"),
        "expected table header in output:\n{stdout}"
    );
    assert!(
        stdout.contains("act"),
        "expected 'act' in output:\n{stdout}"
    );
}

#[test]
fn scan_with_both_verbose_and_dry_run_shows_target_column() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy.tar.gz"), b"x").unwrap();

    let output = sasurahime(tmp.path())
        .arg("scan")
        .arg("--verbose")
        .arg("--dry-run")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Target"),
        "expected 'Target' column header:\n{stdout}"
    );
    assert!(
        stdout.contains(".cache/act"),
        "expected '.cache/act' in verbose output:\n{stdout}"
    );
}

#[test]
fn scan_shows_not_found_for_missing_dirs() {
    let tmp = TempDir::new().unwrap();
    // Do NOT create any cache dirs in tmp

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("not found"),
        "expected 'not found' in output:\n{stdout}"
    );
}
