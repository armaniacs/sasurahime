use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

/// Creates a fake `mise` whose `ls --current` output is fixed.
/// The output is written to a temp file to avoid shell quoting issues with
/// embedded whitespace in the heredoc approach.
fn install_fake_mise(bin_dir: &std::path::Path, ls_current_output: &str) {
    // Write the output to a side file so the script just `cat`s it.
    let output_file = bin_dir.join("mise_ls_output.txt");
    fs::write(&output_file, ls_current_output).unwrap();

    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"ls\" ] && [ \"$2\" = \"--current\" ]; then\n  cat \"{}\"\nfi\nexit 0\n",
        output_file.display()
    );
    let path = bin_dir.join("mise");
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn clean_mise_removes_unused_version() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_mise(&bin_dir, "node\t24.15.0\t~/.config/mise/config.toml\n");

    let installs = tmp.path().join(".local/share/mise/installs/node");
    fs::create_dir_all(installs.join("20.11.0")).unwrap();
    fs::create_dir_all(installs.join("24.15.0")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "mise"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !installs.join("20.11.0").exists(),
        "20.11.0 should have been deleted"
    );
    assert!(
        installs.join("24.15.0").exists(),
        "24.15.0 (active) must remain"
    );
}

#[test]
fn clean_mise_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_mise(&bin_dir, "node\t24.15.0\t~/.config/mise/config.toml\n");

    let installs = tmp.path().join(".local/share/mise/installs/node");
    fs::create_dir_all(installs.join("20.11.0")).unwrap();
    fs::create_dir_all(installs.join("24.15.0")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "mise", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        installs.join("20.11.0").exists(),
        "must not delete in dry-run"
    );
    assert!(
        installs.join("24.15.0").exists(),
        "must not delete in dry-run"
    );
}

#[test]
fn clean_mise_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "mise"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("not found") || stdout.contains("skipping"),
        "expected skip message, got:\n{stdout}"
    );
}

#[test]
fn clean_mise_active_version_is_never_deleted() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_mise(&bin_dir, "node\t24.15.0\t~/.config/mise/config.toml\n");

    let installs = tmp.path().join(".local/share/mise/installs/node");
    fs::create_dir_all(installs.join("24.15.0")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "mise"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(
        installs.join("24.15.0").exists(),
        "active version must not be deleted"
    );
}
