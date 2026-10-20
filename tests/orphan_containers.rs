use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn make_container(home: &Path, sub: &str, name: &str) -> PathBuf {
    let dir = home.join("Library").join(sub).join(name);
    fs::create_dir_all(dir.join("Data")).unwrap();
    fs::write(dir.join("Data/blob.bin"), vec![1u8; 8192]).unwrap();
    dir
}

#[test]
fn clean_orphan_containers_dry_run_lists_orphans_and_keeps_files() {
    let tmp = TempDir::new().unwrap();
    let gone = make_container(tmp.path(), "Containers", "com.example.sasurahimetest.gone");
    let group = make_container(
        tmp.path(),
        "Group Containers",
        "ZZZZZ99999.group.com.example.sasurahimetest.gone",
    );
    make_container(tmp.path(), "Containers", "com.apple.sasurahimetest");

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "orphan-containers", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("com.example.sasurahimetest.gone"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("ZZZZZ99999.group.com.example.sasurahimetest.gone"),
        "stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("com.apple.sasurahimetest"),
        "com.apple.* must be excluded:\n{stdout}"
    );
    assert!(gone.exists() && group.exists(), "dry-run must not delete");
}

#[test]
fn clean_orphan_containers_without_library_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "orphan-containers", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn targets_lists_orphan_containers() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("orphan-containers"), "stdout:\n{stdout}");
}

#[test]
fn clean_orphan_containers_rejects_all_flag() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "orphan-containers", "--all"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}

const SECRET_URL_SCHEME: &str = "sasurahime-secret-url-scheme-7f3a";
const SECRET_VALUE: &str = "SasurahimeSecretValue7f3a";

#[test]
fn info_plist_values_other_than_identity_never_reach_output_or_logs() {
    let tmp = TempDir::new().unwrap();
    let contents = tmp.path().join("Applications/Keeper.app/Contents");
    fs::create_dir_all(&contents).unwrap();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>{SECRET_URL_SCHEME}</string>
            </array>
        </dict>
    </array>
    <key>SasurahimeDummyKey</key>
    <string>{SECRET_VALUE}</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.sasurahimetest.keeper</string>
    <key>CFBundleName</key>
    <string>Keeper</string>
</dict>
</plist>
"#
    );
    fs::write(contents.join("Info.plist"), plist).unwrap();
    let keeper = make_container(
        tmp.path(),
        "Containers",
        "com.example.sasurahimetest.keeper",
    );
    let gone = make_container(tmp.path(), "Containers", "com.example.sasurahimetest.gone");

    let output = sasurahime(tmp.path())
        .env("RUST_LOG", "trace")
        .args(["--permanent", "clean", "orphan-containers", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("com.example.sasurahimetest.gone"),
        "stdout:\n{stdout}"
    );
    // Proves that ~/Applications/Keeper.app/Contents/Info.plist was actually read.
    assert!(
        !stdout.contains("com.example.sasurahimetest.keeper"),
        "installed app's container must not be listed:\n{stdout}"
    );
    for secret in [SECRET_URL_SCHEME, SECRET_VALUE] {
        assert!(
            !stdout.contains(secret),
            "stdout leaked {secret}:\n{stdout}"
        );
        assert!(
            !stderr.contains(secret),
            "stderr or log leaked {secret}:\n{stderr}"
        );
    }
    assert!(keeper.exists() && gone.exists(), "dry-run must not delete");
}
