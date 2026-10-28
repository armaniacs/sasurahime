use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn set_old_mtime(path: &std::path::Path) {
    // 2020-01-01 00:00 — comfortably older than any day threshold.
    let status = Command::new("touch")
        .args(["-t", "202001010000"])
        .arg(path)
        .status()
        .expect("touch available on macOS");
    assert!(status.success());
}

#[test]
fn clean_all_skips_moderate_targets_without_include_manual() {
    let tmp = TempDir::new().unwrap();
    let spm_cache = tmp.path().join("Library/Caches/org.swift.swiftpm");
    fs::create_dir_all(&spm_cache).unwrap();
    fs::write(spm_cache.join("manifest.db"), [0u8; 4096]).unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "all"])
        .output()
        .unwrap();
    assert!(output.status.success(), "clean all must exit 0");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Skipped spm (Moderate)"),
        "Moderate targets must be skipped with their classification, got:\n{combined}"
    );
    assert!(
        combined.contains("needs --include-manual"),
        "the skip reason must be shown, got:\n{combined}"
    );
    assert!(
        spm_cache.exists(),
        "the Moderate target must not be deleted without --include-manual"
    );
}

#[test]
fn clean_all_include_manual_cleans_moderate_targets() {
    let tmp = TempDir::new().unwrap();
    let spm_cache = tmp.path().join("Library/Caches/org.swift.swiftpm");
    fs::create_dir_all(&spm_cache).unwrap();
    fs::write(spm_cache.join("manifest.db"), [0u8; 4096]).unwrap();
    set_old_mtime(&spm_cache);

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "all", "--include-manual"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !spm_cache.exists(),
        "--include-manual must clean the Moderate target, got:\n{combined}"
    );
    assert!(
        combined.contains("Permanently deleted: no Trash copy was kept."),
        "the audit trail must be shown, got:\n{combined}"
    );
}

#[test]
fn global_older_than_filter_skips_fresh_entries_with_a_reason() {
    let tmp = TempDir::new().unwrap();
    let spm_cache = tmp.path().join("Library/Caches/org.swift.swiftpm");
    fs::create_dir_all(&spm_cache).unwrap();
    fs::write(spm_cache.join("manifest.db"), [0u8; 4096]).unwrap();

    let output = sasurahime(tmp.path())
        .args([
            "--permanent",
            "clean",
            "all",
            "--include-manual",
            "--older-than",
            "30",
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
        combined.contains("younger than 30 day(s)"),
        "out-of-condition entries must be shown with a skip reason, got:\n{combined}"
    );
    assert!(
        spm_cache.exists(),
        "the fresh entry must not be deleted by the global filter"
    );
}

#[test]
fn global_older_than_filter_passes_old_entries() {
    let tmp = TempDir::new().unwrap();
    let spm_cache = tmp.path().join("Library/Caches/org.swift.swiftpm");
    fs::create_dir_all(&spm_cache).unwrap();
    fs::write(spm_cache.join("manifest.db"), [0u8; 4096]).unwrap();
    set_old_mtime(&spm_cache);

    let output = sasurahime(tmp.path())
        .args([
            "--permanent",
            "clean",
            "all",
            "--include-manual",
            "--older-than",
            "30",
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
        !spm_cache.exists(),
        "old entries must be cleaned under the global filter, got:\n{combined}"
    );
}

#[test]
fn clean_all_without_include_manual_reports_skipped_target_count() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "all"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("Skipped") && combined.contains("--include-manual"),
        "the summary must report skipped targets, got:\n{combined}"
    );
    assert!(
        combined.contains("Total freed:"),
        "the total must be printed, got:\n{combined}"
    );
}

#[test]
fn per_cleaner_config_filters_apply_in_clean_all() {
    let tmp = TempDir::new().unwrap();
    let spm_cache = tmp.path().join("Library/Caches/org.swift.swiftpm");
    fs::create_dir_all(&spm_cache).unwrap();
    fs::write(spm_cache.join("manifest.db"), [0u8; 4096]).unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        "[cleaner.spm]\nolder_than_days = 30\n",
    )
    .unwrap();

    // Fresh entry + config filter: the setting must actually apply.
    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "all", "--include-manual"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("younger than 30 day(s)"),
        "the [cleaner.*] setting must apply in clean --all, got:\n{combined}"
    );
    assert!(
        spm_cache.exists(),
        "the fresh entry must not be deleted under the config filter"
    );

    // And the ineffective-settings warning must not fire for spm anymore.
    let scan = sasurahime(tmp.path()).arg("scan").output().unwrap();
    let scan_out = format!(
        "{}{}",
        String::from_utf8_lossy(&scan.stdout),
        String::from_utf8_lossy(&scan.stderr)
    );
    assert!(
        !scan_out.contains("have no effect"),
        "effective settings must not be warned about, got:\n{scan_out}"
    );
}
