use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_device_support_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "device-support"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_device_support_keeps_n_versions() {
    let tmp = TempDir::new().unwrap();
    let ios = tmp.path().join("Library/Developer/Xcode/iOS DeviceSupport");
    for v in &["14.0", "15.0", "16.0", "17.0"] {
        fs::create_dir_all(ios.join(v)).unwrap();
        fs::write(ios.join(v).join("dummy"), b"x").unwrap();
    }
    let output = sasurahime(tmp.path())
        .args(["clean", "device-support"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!ios.join("14.0").exists(), "14.0 should be deleted");
    assert!(!ios.join("15.0").exists(), "15.0 should be deleted");
    assert!(ios.join("16.0").exists(), "16.0 should remain");
    assert!(ios.join("17.0").exists(), "17.0 should remain");
}

#[test]
fn scan_shows_device_support_when_old_versions_exist() {
    let tmp = TempDir::new().unwrap();
    let ios = tmp.path().join("Library/Developer/Xcode/iOS DeviceSupport");
    fs::create_dir_all(ios.join("14.0")).unwrap();
    fs::create_dir_all(ios.join("17.0")).unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("device-support"),
        "scan should include device-support:\n{stdout}"
    );
}
