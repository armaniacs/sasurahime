use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn put(home: &Path, rel: &str, bytes: usize) {
    let p = home.join("Library/Application Support").join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![1u8; bytes]).unwrap();
}

const OVER_THRESHOLD: usize = 65 * 1024 * 1024;

#[test]
fn dry_run_lists_cache_dirs_and_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    put(tmp.path(), "Slack/Local Storage/leveldb/keep.bin", 4096);
    put(tmp.path(), "Slack/IndexedDB/keep.bin", 4096);

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Slack/Cache"), "stdout:\n{stdout}");
    assert!(!stdout.contains("Local Storage"), "stdout:\n{stdout}");
    assert!(!stdout.contains("IndexedDB"), "stdout:\n{stdout}");
    let base = tmp.path().join("Library/Application Support/Slack");
    assert!(base.join("Cache/big.bin").exists());
    assert!(base.join("Local Storage/leveldb/keep.bin").exists());
    assert!(base.join("IndexedDB/keep.bin").exists());
}

#[test]
fn dry_run_without_application_support_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn non_interactive_clean_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a terminal"), "stderr:\n{stderr}");
    assert!(tmp
        .path()
        .join("Library/Application Support/Slack/Cache/big.bin")
        .exists());
}

#[test]
fn targets_lists_app_support_exactly_once() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let n = stdout
        .lines()
        .filter(|l| l.split_whitespace().next() == Some("app-support"))
        .count();
    assert_eq!(n, 1, "stdout:\n{stdout}");
}

#[test]
fn scan_includes_app_support() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("app-support"), "stdout:\n{stdout}");
}

/// The scan table row (first cell) whose category is `name`, for the
/// `--no-unicode` table (`| hints | 65.0 MB | pruneable | ... |`).
fn scan_row<'a>(stdout: &'a str, name: &str) -> Option<&'a str> {
    stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .find(|l| {
            l.split('|')
                .nth(1)
                .map(|c| c.trim() == name)
                .unwrap_or(false)
        })
}

#[test]
fn scan_total_excludes_the_hints_row_and_notes_the_overlap() {
    // BDD: Slack/Cache sits in both the hints row (64 MB hint threshold) and
    // the app-support row, so the total must count it once. The hints row
    // stays displayed with its size and one note line after the total
    // explains the overlap. The exact byte-level exclusion is pinned by
    // scan_json_total_excludes_the_hints_row: the formatted table total
    // cannot separate a 65 MB delta from the machine-global vm-swap row.
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);

    let output = sasurahime(tmp.path())
        .args(["--no-unicode", "scan"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Both rows still show the size: the exclusion is total-only.
    let hints_row = scan_row(&stdout, "hints").expect("the hints row is displayed");
    assert!(
        hints_row.contains("65.0 MB"),
        "the hints row keeps its size display:\n{stdout}"
    );
    let app_row = scan_row(&stdout, "app-support").expect("the app-support row is displayed");
    assert!(
        app_row.contains("65.0 MB"),
        "the app-support row keeps its size display:\n{stdout}"
    );
    // Total line (the tests/scan.rs:513-517 find shape) followed by one note
    // line that explains the overlap and the log-threshold counting.
    let total_idx = stdout
        .lines()
        .position(|l| l.contains("Total reclaimable"))
        .expect("total line");
    let after_total: Vec<&str> = stdout.lines().skip(total_idx + 1).collect();
    assert!(
        after_total
            .iter()
            .any(|l| l.contains("overlap the app-support / library-caches rows")),
        "a note line must follow the total explaining the hints overlap:\n{stdout}"
    );
    assert!(
        after_total
            .iter()
            .any(|l| l.contains("counted only in the hints row")),
        "the note line must explain the log-threshold counting:\n{stdout}"
    );
}

#[test]
fn scan_json_total_excludes_the_hints_row() {
    // BDD: the --json total excludes the hints row too — provable in the
    // exact byte values (the same dir is counted in both the hints and the
    // app-support rows) — while the hints entry keeps size_bytes and
    // estimable so consumers can sum it themselves.
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);

    let output = sasurahime(tmp.path())
        .args(["scan", "--json"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    let json_path = tmp.path().join("scan.json");
    fs::write(&json_path, stdout.as_bytes()).unwrap();
    let check = std::process::Command::new("jq")
        .arg("-e")
        .arg(
            r#"(.total_reclaimable_bytes == ([.targets[] | select(.status == "pruneable" and .name != "hints") | .size_bytes] | add // 0)) and
               ([.targets[] | select(.name == "hints" and .status == "pruneable" and .estimable == true and .size_bytes > 0)] | length) == 1"#,
        )
        .arg(&json_path)
        .status()
        .expect("jq must be available for the JSON schema test");
    assert!(
        check.success(),
        "scan --json total must exclude the hints row while the hints entry keeps its size:\n{stdout}"
    );
}

#[test]
fn dry_run_shows_full_paths_without_the_hints_tag() {
    // BDD: dry-run rows keep the full-path contract — the [managed by hints]
    // tag exists on the selection screen only.
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Slack/Cache"), "stdout:\n{stdout}");
    assert!(
        !stdout.contains("[managed by hints]"),
        "dry-run rows carry full paths, not the selection tag:\n{stdout}"
    );
}

#[test]
fn dry_run_shows_leftovers_without_deleting() {
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "ZzzNoSuchApp12345/data.bin", 2 * 1024 * 1024);
    let dir = tmp
        .path()
        .join("Library/Application Support/ZzzNoSuchApp12345");
    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(200 * 86_400));
    set_file_mtime(dir.join("data.bin"), old).unwrap();
    set_file_mtime(&dir, old).unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ZzzNoSuchApp12345"), "stdout:\n{stdout}");
    assert!(stdout.contains("display only"), "stdout:\n{stdout}");
    assert!(dir.join("data.bin").exists());
}

#[test]
fn non_interactive_clean_does_not_list_leftovers() {
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    put(tmp.path(), "ZzzNoSuchApp12345/data.bin", 2 * 1024 * 1024);
    let dir = tmp
        .path()
        .join("Library/Application Support/ZzzNoSuchApp12345");
    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(200 * 86_400));
    set_file_mtime(dir.join("data.bin"), old).unwrap();
    set_file_mtime(&dir, old).unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "app-support"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("ZzzNoSuchApp12345"));
    assert!(dir.join("data.bin").exists());
}
