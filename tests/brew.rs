use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

/// Creates a fake `brew` script that prints a known "freed" message and exits 0.
fn install_fake_brew(bin_dir: &std::path::Path, dry_run_output: &str) {
    let script = format!(
        "#!/bin/sh\nif echo \"$@\" | grep -q 'dry-run'; then\n  echo \"{dry_run_output}\"\nelse\n  echo \"This operation has freed approximately 1.0GB of disk space.\"\nfi\nexit 0\n"
    );
    let path = bin_dir.join("brew");
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn clean_brew_dry_run_shows_would_free() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_brew(
        &bin_dir,
        "Would remove: approximately 500.0MB of disk space.",
    );

    // Create Homebrew cache dir so cleaner doesn't skip
    fs::create_dir_all(tmp.path().join("Library/Caches/Homebrew/downloads")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "brew", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("dry-run") || stdout.contains("dry_run") || stdout.contains("would"),
        "expected dry-run indication in output:\n{stdout}"
    );
}

#[test]
fn clean_brew_calls_cleanup_with_correct_flags() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();

    // Fake brew that records its arguments to a file
    let args_file = tmp.path().join("brew_args.txt");
    let args_file_str = args_file.display().to_string();
    let script = format!(
        "#!/bin/sh\necho \"$@\" > \"{args_file_str}\"\necho \"This operation has freed approximately 2.0GB of disk space.\"\nexit 0\n"
    );
    fs::write(bin_dir.join("brew"), &script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join("brew"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    fs::create_dir_all(tmp.path().join("Library/Caches/Homebrew/downloads")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["clean", "brew"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let recorded_args = fs::read_to_string(&args_file).unwrap_or_default();
    assert!(
        recorded_args.contains("cleanup"),
        "expected 'cleanup' in args: {recorded_args}"
    );
    assert!(
        recorded_args.contains("-s"),
        "expected '-s' in args: {recorded_args}"
    );
    assert!(
        recorded_args.contains("--prune=all"),
        "expected '--prune=all' in args: {recorded_args}"
    );
}

#[test]
fn clean_brew_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "brew"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found") || stdout.contains("skipping"));
}
