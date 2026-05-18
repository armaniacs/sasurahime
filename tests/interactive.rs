use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn install_fake_tool(bin_dir: &std::path::Path, name: &str) {
    fs::write(bin_dir.join(name), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join(name), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn version_flag_output() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("--version").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sasurahime"), "stdout: {stdout}");
    assert!(stdout.contains("0.1.2"), "stdout: {stdout}");
}

#[test]
fn yes_flag_exits_zero_and_skips_tui() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    for tool in &[
        "uv", "brew", "mise", "bun", "go", "pip", "npm", "yarn", "pnpm",
    ] {
        install_fake_tool(&bin_dir, tool);
    }

    // Create a minimal uv cache so at least one cleaner is pruneable
    let uv_cache = tmp.path().join(".cache/uv/archive-v0");
    fs::create_dir_all(&uv_cache).unwrap();
    fs::write(uv_cache.join("dummy"), b"x".repeat(1024)).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .arg("--yes")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Must not contain TUI-specific cursor escape sequences or "Select" prompt
    assert!(
        !stdout.contains("Select caches"),
        "TUI prompt must not appear with --yes, got:\n{stdout}"
    );
}

#[test]
fn yes_flag_nothing_pruneable_exits_zero() {
    let tmp = TempDir::new().unwrap();
    // Empty HOME, restricted PATH — every cleaner returns NotFound or Clean
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Nothing to clean"),
        "expected 'Nothing to clean', got:\n{stdout}"
    );
}

#[test]
fn no_args_without_tty_exits_with_hint() {
    let tmp = TempDir::new().unwrap();
    // No TTY available in CI/headless test env: run_interactive should refuse to prompt
    // and exit with a helpful message.
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");
    // In a non-TTY test env, the process exits 1 with a hint about --yes
    assert!(
        !output.status.success() || combined.contains("--yes") || combined.contains("terminal"),
        "expected non-zero exit or hint, got stdout={stdout} stderr={stderr}"
    );
}

#[test]
fn startup_version_display_yes() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("--yes").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "stdout must start with version, got: {stdout}"
    );
}

#[test]
fn version_display_on_scan() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["scan"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "scan output must start with version, got: {stdout}"
    );
}

#[test]
fn version_display_on_targets() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["targets"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "targets output must start with version, got: {stdout}"
    );
}

#[test]
fn version_display_on_clean_dry_run() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "clean output must start with version, got: {stdout}"
    );
}

// ── GAP-007: --yes bypasses Xcode interactive prompt ──────────────────────
#[test]
fn yes_flag_cleans_xcode_without_interactive_prompt() {
    let tmp = TempDir::new().unwrap();
    let derived = tmp.path().join("Library/Developer/Xcode/DerivedData");
    fs::create_dir_all(derived.join("ProjectA-abcdef")).unwrap();
    fs::write(derived.join("ProjectA-abcdef/dummy"), b"x").unwrap();

    let output = sasurahime(tmp.path()).arg("--yes").output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // ProjectA should have been deleted (--yes bypasses the xcode-running prompt)
    assert!(
        !derived.join("ProjectA-abcdef").exists(),
        "ProjectA must be deleted in --yes mode"
    );
    // DerivedData root must remain
    assert!(derived.exists(), "DerivedData root must remain");
}

#[test]
fn targets_subcommand_output() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain a selection of targets
    assert!(stdout.contains("uv"), "stdout: {stdout}");
    assert!(stdout.contains("brew"), "stdout: {stdout}");
    assert!(stdout.contains("logs"), "stdout: {stdout}");
    assert!(stdout.contains("xcode"), "stdout: {stdout}");
    // Should have descriptions
    assert!(stdout.contains("Stale"), "stdout: {stdout}");
}
