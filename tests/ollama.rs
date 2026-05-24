use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_ollama_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "ollama"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_ollama_dry_run_no_models_shows_nothing() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = "#!/bin/sh\necho 'NAME ID SIZE MODIFIED'\nexit 0\n";
    fs::write(bin_dir.join("ollama"), script).unwrap();
    #[cfg(unix)]
    fs::set_permissions(bin_dir.join("ollama"), fs::Permissions::from_mode(0o755)).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "ollama", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn scan_shows_ollama_in_output() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = "#!/bin/sh\necho 'NAME ID SIZE MODIFIED'\nexit 0\n";
    fs::write(bin_dir.join("ollama"), script).unwrap();
    #[cfg(unix)]
    fs::set_permissions(bin_dir.join("ollama"), fs::Permissions::from_mode(0o755)).unwrap();

    fs::create_dir_all(tmp.path().join(".ollama/models/blobs")).unwrap();
    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .arg("scan")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("ollama"),
        "scan should include ollama:\n{stdout}"
    );
}
