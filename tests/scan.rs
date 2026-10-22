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

    // `scan` hides targets whose binary is missing, and CI runners do not
    // install uv or brew, so provide deterministic stubs on PATH.
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    for tool in ["uv", "brew"] {
        let stub = bin.join(tool);
        fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = sasurahime(tmp.path())
        .env("PATH", path)
        .arg("scan")
        .output()
        .unwrap();

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

#[test]
fn scan_lists_largest_targets_first() {
    let tmp = TempDir::new().unwrap();
    // act is small, tree-sitter is larger; both are directory-based cleaners so
    // they are available without external binaries.
    fs::create_dir_all(tmp.path().join(".cache/act")).unwrap();
    fs::write(tmp.path().join(".cache/act/dummy"), b"x".repeat(100)).unwrap();
    fs::create_dir_all(tmp.path().join(".cache/tree-sitter")).unwrap();
    fs::write(
        tmp.path().join(".cache/tree-sitter/big"),
        b"x".repeat(50_000),
    )
    .unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let pos = |needle: &str| {
        stdout
            .lines()
            .position(|l| l.trim_start().starts_with(&format!("│ {needle} ")))
            .unwrap_or_else(|| panic!("row for {needle} not found:\n{stdout}"))
    };
    assert!(
        pos("tree-sitter") < pos("act"),
        "largest target must come first:\n{stdout}"
    );
}

#[test]
fn scan_shows_unavailable_when_tool_command_fails() {
    let tmp = TempDir::new().unwrap();

    // rustup stub exists on PATH (so the cleaner is available) but its command
    // fails, which the cleaner cannot measure — the row must say `unavailable`
    // instead of lying `clean` or `not found`. act cache is readable so the
    // rest of the scan must not be interrupted.
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let stub = bin.join("rustup");
    fs::write(&stub, "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(100)).unwrap();

    let output = sasurahime(tmp.path())
        .env("PATH", path)
        .arg("scan")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "scan must not fail: exit code must be 0"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unavailable"),
        "expected 'unavailable' row in output:\n{stdout}"
    );
    assert!(
        stdout.contains("act"),
        "other rows must still be scanned and shown:\n{stdout}"
    );
}

#[test]
fn scan_shows_unknown_for_cli_delegating_cleaners() {
    // docker is installed (stub) but detect cannot measure a byte count
    // without running it: the row must say `unknown`, not lie `pruneable`.
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let stub = bin.join("docker");
    fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = sasurahime(tmp.path())
        .env("PATH", path)
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unknown"),
        "expected 'unknown' row for a CLI-delegating cleaner:\n{stdout}"
    );
}

#[test]
fn scan_json_outputs_valid_schema() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(100)).unwrap();

    let output = sasurahime(tmp.path())
        .arg("scan")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json_path = tmp.path().join("scan.json");
    fs::write(&json_path, stdout.as_bytes()).unwrap();
    let check = std::process::Command::new("jq")
        .arg("-e")
        .arg(
            r#"(.total_reclaimable_bytes >= 0) and
               ([.targets[] | select(.name == "act" and .status == "pruneable" and .estimable == true and .size_bytes > 0)] | length) == 1 and
               (all(.targets[]; has("name") and has("status") and has("estimable")))"#,
        )
        .arg(&json_path)
        .status()
        .expect("jq must be available for the JSON schema test");
    assert!(
        check.success(),
        "scan --json schema check failed:\n{stdout}"
    );
}

#[test]
fn scan_shows_risk_column() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    for tool in ["uv", "brew"] {
        let stub = bin.join(tool);
        fs::write(&stub, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );

    let output = sasurahime(tmp.path())
        .env("PATH", path)
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Risk"),
        "expected a Risk column header, got:\n{stdout}"
    );
    // Every one of the four labels may appear; uv (regenerable cache) must be Safe.
    let uv_row = stdout
        .lines()
        .find(|l| l.contains("uv") && l.contains("Safe"))
        .expect("uv row classified Safe in the scan table");
    assert!(
        uv_row.contains("pruneable") || uv_row.contains("not found") || uv_row.contains("clean"),
        "uv row must be a scan table row, got: {uv_row}"
    );
}

#[test]
fn yes_runs_only_safe_risk_targets_and_skips_the_rest() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // uv is present (Safe target, pruneable below). cargo is intentionally
    // absent: the Moderate cargo cache must be skipped, not cleaned.
    let uv_stub = bin_dir.join("uv");
    fs::write(&uv_stub, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&uv_stub, fs::Permissions::from_mode(0o755)).unwrap();
    }

    // Safe target with reclaimable data: uv cache archive.
    let uv_cache = tmp.path().join(".cache/uv/archive-v0");
    fs::create_dir_all(&uv_cache).unwrap();
    fs::write(uv_cache.join("dummy"), b"x".repeat(1024)).unwrap();
    // Moderate target with reclaimable data: cargo registry cache. `--yes`
    // must not touch it because cargo is not Risk::Safe.
    let cargo_cache = tmp.path().join(".cargo/registry/cache");
    fs::create_dir_all(&cargo_cache).unwrap();
    fs::write(cargo_cache.join("dummy.crate"), b"x".repeat(512)).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["--permanent", "--yes"])
        .write_stdin("y\n")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "exit code was not 0\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );

    // The Safe target ran; the Moderate target was skipped with a reason.
    // The stub `uv` does not actually delete, so prove selection via the
    // confirmation summary: exactly one item (uv) was chosen for --yes.
    assert!(
        stdout.contains("Found 1 item"),
        "exactly the Safe target must run under --yes, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Skipped") && stdout.contains("cargo (Moderate)"),
        "cargo (Moderate) must be skipped with a reason, got:\n{stdout}"
    );
    assert!(
        cargo_cache.join("dummy.crate").exists(),
        "cargo cache (Moderate) must not be deleted by --yes"
    );
}

#[test]
fn yes_never_runs_irreversible_risky_targets() {
    let tmp = TempDir::new().unwrap();
    // ios-backup (Risky) with pruneable data must be untouched by --yes.
    // detect() sums the sizes of the per-device backup subdirectories.
    let backup_dir = tmp
        .path()
        .join("Library/Application Support/MobileSync/Backup");
    let device_backup = backup_dir.join("000000-AAAAAAAA");
    fs::create_dir_all(&device_backup).unwrap();
    fs::write(device_backup.join("manifest"), b"x".repeat(64)).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", original_path)
        .args(["--permanent", "--yes"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ios-backup") || stdout.contains("Skipped"),
        "Risky target should be reported as skipped, got:\n{stdout}"
    );
    assert!(
        backup_dir.exists(),
        "ios-backup (Risky) must never be deleted by --yes"
    );
}

#[test]
fn config_exclude_removes_the_risk_from_scan() {
    let tmp = TempDir::new().unwrap();
    let cargo_cache = tmp.path().join(".cargo/registry/cache");
    fs::create_dir_all(&cargo_cache).unwrap();
    fs::write(cargo_cache.join("dummy.crate"), b"x".repeat(512)).unwrap();

    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    // Excluded targets are removed from all_cleaners: no scan row, no risk.
    fs::write(config_dir.join("config.toml"), "exclude = [\"cargo\"]\n").unwrap();

    let output = sasurahime(tmp.path())
        .arg("scan")
        .arg("--json")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_path = tmp.path().join("scan.json");
    fs::write(&json_path, stdout.as_bytes()).unwrap();
    let check = std::process::Command::new("jq")
        .arg("-e")
        .arg(
            r#"([.targets[] | select(.name == "cargo")] | length) == 0 and
               (all(.targets[]; has("name") and has("status") and has("risk") and has("estimable")))"#,
        )
        .arg(&json_path)
        .status()
        .expect("jq must be available for the JSON schema test");
    assert!(
        check.success(),
        "excluded target must not appear:\n{stdout}"
    );
}

#[test]
fn second_scan_reuses_the_persistent_cache() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(100)).unwrap();

    // First scan: creates the cache file.
    let first = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(first.status.success());
    let cache_path = tmp.path().join(".local/share/sasurahime/scan-cache.json");
    assert!(
        cache_path.exists(),
        "first scan must save the persistent cache"
    );
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let first_total = first_stdout
        .lines()
        .find(|l| l.contains("Total reclaimable"))
        .expect("total line")
        .to_string();

    // Second scan with no file changes: served from the cache.
    let second = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(second.status.success());
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    assert!(
        second_stdout.contains(&first_total),
        "totals must match between scans:\nfirst: {first_total}\nsecond:\n{second_stdout}"
    );
    let second_cache = fs::read_to_string(&cache_path).unwrap();
    assert!(
        second_cache.contains(".cache/act"),
        "cache must record the target dir:\n{second_cache}"
    );
}

#[test]
fn changed_target_is_remeasured_by_the_cache() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(100)).unwrap();
    let _ = sasurahime(tmp.path()).arg("scan").output().unwrap();

    // Change the target: a new direct child updates the directory mtime, so
    // the cache must re-measure it. 100 B + 4 KiB of files occupy 8 KiB of
    // physical blocks on APFS (dir_size reports blocks * 512).
    fs::write(act_cache.join("extra"), b"x".repeat(4096)).unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let act_row = stdout
        .lines()
        .find(|l| l.contains("act"))
        .expect("act row in the scan table");
    assert!(
        act_row.contains("8.0 KB"),
        "the changed target must be re-measured (8.0 KB expected), got: {act_row}"
    );
}

#[test]
fn corrupt_cache_falls_back_to_a_full_scan() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(100)).unwrap();

    let _ = sasurahime(tmp.path()).arg("scan").output().unwrap();
    let cache_path = tmp.path().join(".local/share/sasurahime/scan-cache.json");
    fs::write(&cache_path, "not valid json :::").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(
        output.status.success(),
        "a corrupt cache must not fail the scan"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("act") && stdout.contains("Total reclaimable"),
        "full scan results must still be correct:\n{stdout}"
    );
}

#[test]
fn no_cache_flag_skips_the_persistent_cache() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(100)).unwrap();

    let output = sasurahime(tmp.path())
        .args(["--no-cache", "scan"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let cache_path = tmp.path().join(".local/share/sasurahime/scan-cache.json");
    assert!(
        !cache_path.exists(),
        "--no-cache must not write the cache file"
    );
}
