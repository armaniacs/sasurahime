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
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd
}

/// Same as `sasurahime`, with an explicit PATH so fake plugin executables
/// are discoverable (tests/plugin.rs pattern).
fn sasurahime_on_path(home: &std::path::Path, path: &str) -> Command {
    let mut cmd = sasurahime(home);
    cmd.env("PATH", path);
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

// --- PBI-38: external plugin targets (CleanTarget::External) ---

/// Installs a fake `sasurahime-foo` plugin whose clean deletes
/// `$HOME/.cache/plugin-foo` for real unless invoked with `--dry-run`.
/// Every clean invocation records its args, so the tests can prove exactly
/// what sasurahime forwarded to the protocol.
fn install_dry_run_aware_plugin(bin_dir: &std::path::Path) {
    let plugin = bin_dir.join("sasurahime-foo");
    fs::write(
        &plugin,
        "#!/bin/sh\n\
         if [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\n\
         printf '%s\\n' \"$@\" > \"$HOME/plugin-args.txt\"\n\
         if [ \"$2\" = \"--dry-run\" ]; then\n\
           echo \"[dry-run] would remove: $HOME/.cache/plugin-foo\"\n\
           exit 0\n\
         fi\n\
         rm -rf \"$HOME/.cache/plugin-foo\"\n\
         exit 0\n",
    )
    .unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();
}

fn make_plugin_cache(home: &std::path::Path) -> std::path::PathBuf {
    let cache = home.join(".cache/plugin-foo");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("x"), "x").unwrap();
    cache
}

#[test]
fn global_dry_run_before_clean_reaches_external_plugin() {
    // PBI-38: the External arm re-parses only the raw external args for
    // --dry-run and ignores the global flag, so `--dry-run clean foo`
    // started the plugin's real clean.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_dry_run_aware_plugin(&bin_dir);
    let cache = make_plugin_cache(tmp.path());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime_on_path(tmp.path(), &path)
        .args(["--dry-run", "clean", "foo"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "the global --dry-run before `clean` must be accepted, got:\n{combined}"
    );
    let args = fs::read_to_string(tmp.path().join("plugin-args.txt"))
        .expect("the plugin must have been started");
    assert!(
        args.contains("--dry-run"),
        "the global --dry-run must reach the plugin as `clean --dry-run`, got: {args}"
    );
    assert!(
        cache.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn local_dry_run_after_clean_reaches_external_plugin() {
    // PBI-38: the local position (`clean foo --dry-run`) already worked via
    // the raw-args re-parse; the global-flag fix must not break it.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_dry_run_aware_plugin(&bin_dir);
    let cache = make_plugin_cache(tmp.path());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime_on_path(tmp.path(), &path)
        .args(["clean", "foo", "--dry-run"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    let args = fs::read_to_string(tmp.path().join("plugin-args.txt"))
        .expect("the plugin must have been started");
    assert!(
        args.contains("--dry-run"),
        "the subcommand-local --dry-run must still reach the plugin, got: {args}"
    );
    assert!(
        cache.exists(),
        "nothing may be deleted in dry-run mode, got:\n{combined}"
    );
}

#[test]
fn help_after_external_target_never_runs_the_plugin_clean() {
    // PBI-38: `--help`/`-h` after the target name is captured verbatim into
    // the external_subcommand's raw args, so without a guard the plugin's
    // real clean ran. Help must never be destructive.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_dry_run_aware_plugin(&bin_dir);
    let cache = make_plugin_cache(tmp.path());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime_on_path(tmp.path(), &path)
        .args(["clean", "foo", "--help"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "--help must be treated as help (exit 0), got:\n{combined}"
    );
    assert!(
        !tmp.path().join("plugin-args.txt").exists(),
        "the plugin's clean must never run for --help, got:\n{combined}"
    );
    assert!(
        combined.contains("external plugin"),
        "the advisory must name the plugin target, got:\n{combined}"
    );
    assert!(
        cache.exists(),
        "--help must not delete the plugin's target, got:\n{combined}"
    );
}

#[test]
fn clean_external_without_dry_run_deletes() {
    // Control for the external dry-run/help tests: proves the plugin
    // actually deletes, so the dry-run and help assertions are not vacuous.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_dry_run_aware_plugin(&bin_dir);
    let cache = make_plugin_cache(tmp.path());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime_on_path(tmp.path(), &path)
        .args(["--permanent", "clean", "foo"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(output.status.success(), "got:\n{combined}");
    let args = fs::read_to_string(tmp.path().join("plugin-args.txt"))
        .expect("the plugin must have been started");
    assert!(
        !args.contains("--dry-run"),
        "the control must run the plugin's real clean, got: {args}"
    );
    assert!(
        !cache.exists(),
        "the control must delete the target, got:\n{combined}"
    );
}
