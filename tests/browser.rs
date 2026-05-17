use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn touch(dir: &std::path::Path) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("dummy"), b"x").unwrap();
}

#[test]
fn clean_browsers_removes_old_puppeteer_chrome() {
    let tmp = TempDir::new().unwrap();
    let chrome = tmp.path().join(".cache/puppeteer/chrome");
    touch(&chrome.join("mac_arm-131.0.6778.204"));
    touch(&chrome.join("mac_arm-140.0.7339.80"));

    let output = sasurahime(tmp.path())
        .args(["clean", "browsers"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !chrome.join("mac_arm-131.0.6778.204").exists(),
        "old version should be deleted"
    );
    assert!(
        chrome.join("mac_arm-140.0.7339.80").exists(),
        "latest version must remain"
    );
}

#[test]
fn clean_browsers_removes_old_playwright_build() {
    let tmp = TempDir::new().unwrap();
    let playwright = tmp.path().join("Library/Caches/ms-playwright");
    touch(&playwright.join("chromium-1208"));
    touch(&playwright.join("chromium-1217"));

    let output = sasurahime(tmp.path())
        .args(["clean", "browsers"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        !playwright.join("chromium-1208").exists(),
        "old build should be deleted"
    );
    assert!(
        playwright.join("chromium-1217").exists(),
        "newest build must remain"
    );
}

#[test]
fn clean_browsers_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let chrome = tmp.path().join(".cache/puppeteer/chrome");
    touch(&chrome.join("mac_arm-131.0.6778.204"));
    touch(&chrome.join("mac_arm-140.0.7339.80"));

    let output = sasurahime(tmp.path())
        .args(["clean", "browsers", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        chrome.join("mac_arm-131.0.6778.204").exists(),
        "must not delete in dry-run"
    );
    assert!(
        chrome.join("mac_arm-140.0.7339.80").exists(),
        "must not delete in dry-run"
    );
}

#[test]
fn clean_browsers_missing_dirs_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "browsers"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("not found") || stdout.contains("skipping") || stdout.contains("nothing"),
        "expected skip message, got:\n{stdout}"
    );
}

#[test]
fn clean_browsers_single_version_is_kept() {
    let tmp = TempDir::new().unwrap();
    let chrome = tmp.path().join(".cache/puppeteer/chrome");
    touch(&chrome.join("mac_arm-140.0.7339.80"));

    let output = sasurahime(tmp.path())
        .args(["clean", "browsers"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        chrome.join("mac_arm-140.0.7339.80").exists(),
        "sole version must never be deleted"
    );
}
