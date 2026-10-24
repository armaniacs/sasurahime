use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn write_config(home: &Path, body: &str) {
    let dir = home.join(".config/sasurahime");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("config.toml"), body).unwrap();
}

const HEADING: &str = "some [cleaner.*] settings have no effect";

fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

#[test]
fn scan_warns_once_about_an_ineffective_setting() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "[cleaner.docker]\nolder_than_days = 30\n");

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(count(&stderr, HEADING), 1, "stderr:\n{stderr}");
    assert!(
        stderr.contains("[docker] older_than_days"),
        "stderr:\n{stderr}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Category"),
        "scan table must still print:\n{stdout}"
    );
}

#[test]
fn no_config_means_no_warning() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("[config]"), "stderr:\n{stderr}");
}

#[test]
fn every_entry_point_warns_exactly_once() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "[cleaner.docker]\nolder_than_days = 30\n");

    for args in [
        vec!["scan"],
        vec!["targets"],
        vec!["clean", "uv", "--dry-run"],
        vec!["explore", "--dry-run"],
    ] {
        let output = sasurahime(tmp.path()).args(&args).output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(
            count(&stderr, HEADING),
            1,
            "args={args:?}\nstderr:\n{stderr}"
        );
    }
}

#[test]
fn accepted_filters_are_silent() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "[cleaner.logs]\nolder_than_days = 14\n\n[cleaner.act]\nlarger_than_mb = 10\n",
    );

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("[config]"), "stderr:\n{stderr}");
}

#[test]
fn unknown_cleaner_name_gets_its_own_line() {
    let tmp = TempDir::new().unwrap();
    write_config(tmp.path(), "[cleaner.no-such]\nlarger_than_mb = 10\n");

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[no-such] unknown cleaner name"),
        "stderr:\n{stderr}"
    );
}

// --- PBI-39: [[custom]] names colliding with built-in targets ---

#[test]
fn custom_name_colliding_with_builtin_warns_at_startup() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "[[custom]]\nname = \"uv\"\npath = \"~/.cache/uv-custom\"\n",
    );

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[uv] collides with built-in target"),
        "the collision must be warned about, stderr:\n{stderr}"
    );
}

#[test]
fn non_colliding_custom_name_is_silent() {
    let tmp = TempDir::new().unwrap();
    write_config(
        tmp.path(),
        "[[custom]]\nname = \"my-workspace\"\npath = \"~/.cache/my-workspace\"\n",
    );

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("collides"),
        "a non-colliding [[custom]] name must not warn, stderr:\n{stderr}"
    );
}
