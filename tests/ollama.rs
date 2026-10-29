use assert_cmd::Command;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

#[test]
fn clean_ollama_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["--permanent", "clean", "ollama"])
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
        .args(["--permanent", "clean", "ollama", "--dry-run"])
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

// PBI-52: ollama is classified `Risk::Safe`, so `--yes` auto-selects it for
// unattended cleaning, but the interactive MultiSelect used to run with no
// TTY guard and failed the whole run on a non-TTY stdin with an obscure
// dialoguer error. `Command::output()` gives the spawned binary a non-TTY
// stdin, so this pins the gated behavior: the clean completes with exit 0,
// the standard skip line is printed, and nothing is deleted.
#[test]
fn clean_ollama_non_tty_skips_interactive_selection() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = "#!/bin/sh\necho 'NAME ID SIZE MODIFIED'\necho 'llama3.2:3b abc123 2.0GB 2 days ago'\nexit 0\n";
    fs::write(bin_dir.join("ollama"), script).unwrap();
    #[cfg(unix)]
    fs::set_permissions(bin_dir.join("ollama"), fs::Permissions::from_mode(0o755)).unwrap();

    // A populated models dir proves the skip deletes nothing.
    let models_dir = tmp.path().join(".ollama/models");
    fs::create_dir_all(&models_dir).unwrap();
    fs::write(models_dir.join("sentinel"), "keep me").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["--permanent", "clean", "ollama"])
        .output()
        .unwrap();
    assert!(output.status.success(), "exit: {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Error cleaning ollama"),
        "no dialoguer failure expected:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stderr.contains("not a terminal - skipping"),
        "standard skip line expected:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        models_dir.join("sentinel").exists(),
        "the skip must delete nothing"
    );
}

// PBI 60: ollama is `Risk::Safe`, so `--yes` selects it for unattended
// cleaning, but its clean() gates on an interactive prompt and skips on a
// non-TTY stdin (PBI-52's fix, kept). The per-cleaner stderr line alone does
// not reach run_auto's skipped-targets notice, so an unattended run looked
// like a no-op success. `Command::output()` gives the spawned binary a
// non-TTY stdin, so this pins the run_auto notice: the gate-skipped ollama
// target must be named on stdout, and nothing is deleted.
#[test]
fn yes_run_auto_notice_names_ollama_as_gate_skipped() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = "#!/bin/sh\necho 'NAME ID SIZE MODIFIED'\necho 'llama3.2:3b abc123 2.0GB 2 days ago'\nexit 0\n";
    fs::write(bin_dir.join("ollama"), script).unwrap();
    #[cfg(unix)]
    fs::set_permissions(bin_dir.join("ollama"), fs::Permissions::from_mode(0o755)).unwrap();

    // A populated models dir proves the skip deletes nothing.
    let models_dir = tmp.path().join(".ollama/models");
    fs::create_dir_all(&models_dir).unwrap();
    fs::write(models_dir.join("sentinel"), "keep me").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["--permanent", "--yes"])
        .write_stdin("y\n")
        .output()
        .unwrap();
    assert!(output.status.success(), "exit: {:?}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Error cleaning ollama"),
        "no dialoguer failure expected:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("ollama") && stdout.contains("interactive terminal"),
        "run_auto's skipped-targets notice must name ollama as gate-skipped:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        models_dir.join("sentinel").exists(),
        "the gate skip must delete nothing"
    );
}
