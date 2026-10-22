use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path, path: &str) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd.env("PATH", path);
    cmd
}

fn install_plugin(dir: &std::path::Path, name: &str, body: &str) {
    let plugin = dir.join(name);
    fs::write(&plugin, body).unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();
}

fn good_plugin_body() -> &'static str {
    "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nexit 0\n"
}

#[test]
fn plugin_appears_in_scan_and_total() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("foo"))
        .expect("foo row in the scan table");
    assert!(
        row.contains("pruneable"),
        "the plugin must be measured like any target: {row}"
    );
    assert!(
        stdout.contains("512 B"),
        "the plugin size must appear in the total"
    );
}

#[test]
fn failing_plugin_is_isolated_as_unavailable() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    install_plugin(&bin_dir, "sasurahime-broken", "#!/bin/sh\nexit 1\n");
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path).arg("scan").output().unwrap();
    assert!(
        output.status.success(),
        "one broken plugin must not take the scan down"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let broken_row = stdout
        .lines()
        .find(|l| l.contains("broken"))
        .expect("broken row");
    assert!(
        broken_row.contains("unavailable"),
        "the failing plugin's row must be unavailable: {broken_row}"
    );
    let foo_row = stdout.lines().find(|l| l.contains("foo")).expect("foo row");
    assert!(
        foo_row.contains("pruneable"),
        "other targets must display normally: {foo_row}"
    );
}

#[test]
fn hung_plugin_is_isolated_by_the_timeout() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-hung", "#!/bin/sh\nsleep 60\n");
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Override the plugin timeout so the test stays fast.
    let output = sasurahime(tmp.path(), &path)
        .env("SASURAHIME_PLUGIN_TIMEOUT_SECS", "1")
        .arg("scan")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "a hung plugin must not block the scan"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("hung"))
        .expect("hung row");
    assert!(
        row.contains("unavailable"),
        "the timed-out plugin must be unavailable: {row}"
    );
}

#[test]
fn targets_marks_plugin_derived_entries() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .arg("targets")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("foo"))
        .expect("foo entry in targets");
    assert!(
        line.contains("(plugin)"),
        "plugin-derived targets must be marked: {line}"
    );
}

#[test]
fn plugin_clean_uses_the_protocol() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // A plugin whose clean deletes its cache dir for real (no Trash).
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nrm -rf \"$HOME/.cache/plugin-foo\"\nexit 0\n",
    );
    let cache = tmp.path().join(".cache/plugin-foo");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("x"), [0u8; 512]).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(output.status.success(), "the plugin clean must run");
    assert!(!cache.exists(), "the plugin must have cleaned its cache");
}
