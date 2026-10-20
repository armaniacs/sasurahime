use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn put(home: &std::path::Path, name: &str, bytes: usize) {
    let d = home.join("Library/Caches").join(name);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("data.bin"), vec![1u8; bytes]).unwrap();
}

fn fixture(home: &std::path::Path) {
    put(home, "typescript", 65 * 1024 * 1024);
    put(home, "BigUnknownApp", 4096);
    put(home, "Homebrew", 4096);
    put(home, "com.apple.foo", 4096);
}

#[test]
fn dry_run_lists_deletable_and_hides_excluded() {
    let tmp = TempDir::new().unwrap();
    fixture(tmp.path());
    let out = sasurahime(tmp.path())
        .args(["--permanent", "clean", "library-caches", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("would remove:"), "stdout:\n{stdout}");
    assert!(stdout.contains("typescript"), "stdout:\n{stdout}");
    assert!(stdout.contains("BigUnknownApp"), "stdout:\n{stdout}");
    assert!(stdout.contains("[display only]"), "stdout:\n{stdout}");
    assert!(!stdout.contains("Homebrew"), "stdout:\n{stdout}");
    assert!(!stdout.contains("com.apple.foo"), "stdout:\n{stdout}");
    assert!(
        !stdout
            .lines()
            .any(|l| l.contains("would remove:") && l.contains("BigUnknownApp")),
        "unknown apps must never be removable:\n{stdout}"
    );
    assert!(tmp
        .path()
        .join("Library/Caches/typescript/data.bin")
        .exists());
}

#[test]
fn non_tty_clean_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    fixture(tmp.path());
    let out = sasurahime(tmp.path())
        .args(["--permanent", "clean", "library-caches"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(tmp
        .path()
        .join("Library/Caches/typescript/data.bin")
        .exists());
    assert!(tmp
        .path()
        .join("Library/Caches/BigUnknownApp/data.bin")
        .exists());
}

#[test]
fn missing_caches_dir_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let out = sasurahime(tmp.path())
        .args(["--permanent", "clean", "library-caches", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn scan_includes_library_caches_with_deletable_total_only() {
    let tmp = TempDir::new().unwrap();
    fixture(tmp.path());
    let out = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("library-caches"))
        .unwrap_or_else(|| panic!("no library-caches row:\n{stdout}"));
    assert!(row.contains("pruneable"), "row: {row}");
    assert!(
        row.contains("65."),
        "only typescript (about 65 MB) is counted: {row}"
    );
}

#[test]
fn targets_lists_both_caches_and_library_caches() {
    let tmp = TempDir::new().unwrap();
    let out = sasurahime(tmp.path()).arg("targets").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.lines().any(|l| l.starts_with("caches ")), "{stdout}");
    assert!(stdout.contains("library-caches"), "{stdout}");
}
