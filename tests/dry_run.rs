// PBI-29: the global `--dry-run` flag must reach subcommands that re-declare
// their own `dry_run` arg (the `CleanTarget` variants and `Auto`).
//
// Verified empirically against clap 4.6 (clap_builder
// `ArgMatcher::fill_in_global_values`): after parsing, every matched global
// value is inserted into the arg matches of EVERY level of the subcommand
// tree, including levels that re-declare an arg with the same id. So the
// global value flows down into the subcommand-local `dry_run` field even
// though main.rs re-declares it, and when both positions are given the
// innermost value wins (same ValueSource), keeping local flags working.
//
// These E2E tests pin that behavior at both flag positions for a special
// CleanTarget variant (`caches`), a macro-generated standard target
// (`node-gyp`, dispatched through `target.dry_run()`), another special
// variant (`xcode`), and the `Auto` subcommand. Controls prove the targets
// actually delete without the flag, so the dry-run assertions are not
// vacuous.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd
}

fn combined_output(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Fake `clean caches` observable: the node-gyp PathCleaner deletes these
/// two HOME-scoped directories; the other caches cleaners only delegate to
/// package-manager CLIs (harmless in a tempdir HOME).
fn make_node_gyp_caches(home: &std::path::Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let dot_cache = home.join(".cache/node-gyp");
    let library_cache = home.join("Library/Caches/node-gyp");
    fs::create_dir_all(&dot_cache).unwrap();
    fs::create_dir_all(&library_cache).unwrap();
    fs::write(dot_cache.join("header"), "x").unwrap();
    fs::write(library_cache.join("header"), "x").unwrap();
    (dot_cache, library_cache)
}

fn make_derived_data(home: &std::path::Path) -> std::path::PathBuf {
    let project = home.join("Library/Developer/Xcode/DerivedData/FakeApp-abcdef12");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("build.txt"), "x").unwrap();
    project
}

fn write_policy(home: &std::path::Path, policy: &str) {
    let config_dir = home.join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), policy).unwrap();
}

fn make_old_act_cache(home: &std::path::Path) -> std::path::PathBuf {
    let act_cache = home.join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), [0u8; 4096]).unwrap();
    // 2020-01-01: comfortably older than any age threshold.
    let status = Command::new("touch")
        .args(["-t", "202001010000"])
        .arg(&act_cache)
        .status()
        .expect("touch available on macOS");
    assert!(status.success());
    act_cache
}

#[test]
fn global_dry_run_before_subcommand_reaches_clean_caches() {
    let tmp = TempDir::new().unwrap();
    let (dot_cache, library_cache) = make_node_gyp_caches(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--dry-run", "clean", "caches"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "the global --dry-run before the subcommand must be accepted, got:\n{combined}"
    );
    assert!(
        combined.contains("[dry-run] would remove:"),
        "the global flag must reach the caches batch, got:\n{combined}"
    );
    assert!(
        dot_cache.exists() && library_cache.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn local_dry_run_after_subcommand_reaches_clean_caches() {
    let tmp = TempDir::new().unwrap();
    let (dot_cache, library_cache) = make_node_gyp_caches(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "--dry-run", "caches"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    assert!(
        combined.contains("[dry-run] would remove:"),
        "the subcommand-local flag must still work, got:\n{combined}"
    );
    assert!(
        dot_cache.exists() && library_cache.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn global_dry_run_reaches_standard_clean_target() {
    let tmp = TempDir::new().unwrap();
    let (dot_cache, library_cache) = make_node_gyp_caches(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--dry-run", "clean", "node-gyp"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    assert!(
        combined.contains("[dry-run] would remove:"),
        "the global flag must reach the standard target via target.dry_run(), got:\n{combined}"
    );
    assert!(
        dot_cache.exists() && library_cache.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn clean_node_gyp_without_dry_run_deletes() {
    // Control for global_dry_run_reaches_standard_clean_target: proves the
    // target actually deletes, so the dry-run assertions are not vacuous.
    let tmp = TempDir::new().unwrap();
    let (dot_cache, library_cache) = make_node_gyp_caches(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "node-gyp"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    assert!(
        !dot_cache.exists() && !library_cache.exists(),
        "the control must delete the target, got:\n{combined}"
    );
}

#[test]
fn global_dry_run_reaches_xcode() {
    let tmp = TempDir::new().unwrap();
    let project = make_derived_data(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--dry-run", "clean", "xcode"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    assert!(
        combined.contains("[dry-run] would remove: DerivedData/"),
        "the global flag must reach the xcode cleaner, got:\n{combined}"
    );
    assert!(
        project.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn local_dry_run_reaches_xcode() {
    let tmp = TempDir::new().unwrap();
    let project = make_derived_data(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "--dry-run", "xcode"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    assert!(
        combined.contains("[dry-run] would remove: DerivedData/"),
        "the subcommand-local flag must still work, got:\n{combined}"
    );
    assert!(
        project.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn clean_xcode_without_dry_run_deletes_derived_data() {
    // Control for global_dry_run_reaches_xcode / local_dry_run_reaches_xcode.
    let tmp = TempDir::new().unwrap();
    let project = make_derived_data(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "xcode"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    assert!(
        !project.exists(),
        "the control must delete the target, got:\n{combined}"
    );
}

#[test]
fn global_dry_run_before_auto_prints_the_plan_without_deleting() {
    // auto_policy.rs covers the local `auto --dry-run` position; this pins
    // the global position. `auto` requires a [policy] section.
    let tmp = TempDir::new().unwrap();
    let act_cache = make_old_act_cache(tmp.path());
    write_policy(tmp.path(), "[policy]\nolder_than_days = 30\n");

    let output = sasurahime(tmp.path())
        .args(["--dry-run", "auto"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "the global --dry-run before `auto` must be accepted, got:\n{combined}"
    );
    assert!(
        combined.contains("[dry-run]"),
        "the global flag must reach the auto batch, got:\n{combined}"
    );
    assert!(
        act_cache.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}
