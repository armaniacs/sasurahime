use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
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
        .args(["--permanent", "clean", "brew", "--dry-run"])
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
        .args(["--permanent", "clean", "brew"])
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
        .args(["--permanent", "clean", "brew"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found") || stdout.contains("skipping"));
}

/// Creates a fake `brew` script that runs `sh` commands then prints a line.
fn install_fake_brew_with_action(bin_dir: &std::path::Path, action: &str, output: &str, code: i32) {
    let script = format!("#!/bin/sh\n{action}\necho \"{output}\"\nexit {code}\n");
    let path = bin_dir.join("brew");
    fs::write(&path, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn clean_brew_nonzero_exit_reports_failure_and_no_freed() {
    // Failure-path coverage (PBI 53): a non-zero cleanup exit must not have
    // its partial stdout parsed as a freed-bytes report.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_brew_with_action(&bin_dir, "", "partial output before the failure", 1);

    fs::create_dir_all(tmp.path().join("Library/Caches/Homebrew/downloads")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--permanent", "clean", "brew"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stderr.contains("cleanup exited with 1"),
        "expected the exit-code refusal on stderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Freed: 0 B"),
        "a failed run must not report the partial run's bytes as freed:\n{stdout}"
    );
}

#[test]
fn clean_brew_parse_miss_warns_when_the_cache_shrinks() {
    // Parse-miss evidence (PBI 53): brew exited 0 and the cache shrank, but
    // the freed-bytes line did not parse (a wording change) - the warning
    // must fire instead of a silent 0. This also exercises the second
    // dir_size walk, which runs only on this path (review 2026-10-29).
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let cache_file = tmp.path().join("Library/Caches/Homebrew/downloads/f.bin");
    fs::create_dir_all(cache_file.parent().unwrap()).unwrap();
    fs::write(&cache_file, vec![0u8; 4096]).unwrap();
    let action = format!("rm -f \"{}\"", cache_file.display());
    install_fake_brew_with_action(&bin_dir, &action, "brew changed its wording", 0);

    let original_path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{}", bin_dir.display(), original_path);

    let output = sasurahime(tmp.path())
        .env("PATH", &new_path)
        .args(["--permanent", "clean", "brew"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("could not be parsed") && stderr.contains("shrank by"),
        "expected the parse-miss warning on stderr:\n{stderr}"
    );
}
