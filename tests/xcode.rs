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
fn clean_xcode_deletes_project_dirs_not_root() {
    let tmp = TempDir::new().unwrap();
    let derived = tmp.path().join("Library/Developer/Xcode/DerivedData");
    touch(&derived.join("ProjectA-abcdef"));
    touch(&derived.join("ProjectB-ghijkl"));

    let output = sasurahime(tmp.path())
        .args(["clean", "xcode"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !derived.join("ProjectA-abcdef").exists(),
        "ProjectA should be deleted"
    );
    assert!(
        !derived.join("ProjectB-ghijkl").exists(),
        "ProjectB should be deleted"
    );
    assert!(derived.exists(), "DerivedData root itself must remain");
}

#[test]
fn clean_xcode_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let derived = tmp.path().join("Library/Developer/Xcode/DerivedData");
    touch(&derived.join("ProjectA-abcdef"));

    let output = sasurahime(tmp.path())
        .args(["clean", "xcode", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        derived.join("ProjectA-abcdef").exists(),
        "must not delete in dry-run"
    );
}

#[test]
fn clean_xcode_missing_derived_data_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "xcode"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("not found") || stdout.contains("skipping"),
        "expected skip message, got:\n{stdout}"
    );
}
