use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn deny(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
    }
}

fn allow(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn doctor_reports_denied_protected_paths_with_guidance() {
    let tmp = TempDir::new().unwrap();
    let mail = tmp.path().join("Library/Mail");
    fs::create_dir_all(&mail).unwrap();
    fs::write(mail.join("INBOX"), b"x").unwrap();
    deny(&mail);

    let output = sasurahime(tmp.path()).arg("doctor").output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mail"),
        "unreadable protected paths must be listed, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Full Disk Access") || stdout.contains("full disk access"),
        "granting instructions must be shown, got:\n{stdout}"
    );
    allow(&mail);
}

#[test]
fn doctor_is_quiet_when_everything_is_readable() {
    let tmp = TempDir::new().unwrap();
    let mail = tmp.path().join("Library/Mail");
    fs::create_dir_all(&mail).unwrap();
    fs::write(mail.join("INBOX"), b"x").unwrap();

    let output = sasurahime(tmp.path()).arg("doctor").output().unwrap();

    assert!(
        output.status.success(),
        "exit code must be 0 on a clean run"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No problems"),
        "a clean run must report no problems, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("could not be read"),
        "readable paths must not be reported as problems, got:\n{stdout}"
    );
}

#[test]
fn doctor_ignores_missing_protected_paths() {
    let tmp = TempDir::new().unwrap();
    // No Mail directory at all: the user does not use Mail.
    let output = sasurahime(tmp.path()).arg("doctor").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("Mail"),
        "missing paths must not be warned about, got:\n{stdout}"
    );
}

#[test]
fn scan_notes_unreadable_protected_paths() {
    let tmp = TempDir::new().unwrap();
    let mail = tmp.path().join("Library/Mail");
    fs::create_dir_all(&mail).unwrap();
    fs::write(mail.join("INBOX"), b"x").unwrap();
    deny(&mail);

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Mail")
            && (stdout.contains("Full Disk Access") || stdout.contains("permission")),
        "scan must note unreadable protected paths, got:\n{stdout}"
    );
    allow(&mail);
}

#[test]
fn scan_json_reports_unreadable_protected_paths() {
    let tmp = TempDir::new().unwrap();
    let mail = tmp.path().join("Library/Mail");
    fs::create_dir_all(&mail).unwrap();
    fs::write(mail.join("INBOX"), b"x").unwrap();
    deny(&mail);

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
            r#"([.unreadable_protected_paths[] | select(endswith("Library/Mail"))] | length) == 1"#,
        )
        .arg(&json_path)
        .status()
        .expect("jq must be available for the JSON schema test");
    assert!(check.success(), "JSON must list the denied path:\n{stdout}");
    allow(&mail);
}

#[test]
fn scan_json_has_no_unreadable_paths_when_all_readable() {
    let tmp = TempDir::new().unwrap();
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
        .arg(r#"(.unreadable_protected_paths | length) == 0"#)
        .arg(&json_path)
        .status()
        .expect("jq must be available");
    assert!(check.success(), "empty list expected:\n{stdout}");
}
