use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn make_old(path: &std::path::Path) {
    let status = Command::new("touch")
        .args(["-t", "202001010000"])
        .arg(path)
        .status()
        .expect("touch available on macOS");
    assert!(status.success());
}

fn make_old_project(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    let project = home.join("Playground").join(name);
    fs::create_dir_all(project.join("node_modules/pkg")).unwrap();
    fs::write(project.join("package.json"), b"{}").unwrap();
    fs::write(project.join("node_modules/pkg/lib.js"), [0u8; 4096]).unwrap();
    fs::write(project.join("src.js"), b"console.log(1)").unwrap();
    // The newest source update must be older than the recency window.
    make_old(&project.join("src.js"));
    make_old(&project.join("package.json"));
    project
}

fn make_recent_project(home: &std::path::Path, name: &str) -> std::path::PathBuf {
    let project = home.join("Playground").join(name);
    fs::create_dir_all(project.join("node_modules")).unwrap();
    fs::write(project.join("package.json"), b"{}").unwrap();
    fs::write(project.join("node_modules/lib.js"), [0u8; 4096]).unwrap();
    fs::write(project.join("src.js"), b"console.log(2)").unwrap();
    project
}

#[test]
fn scan_detects_stale_project_artifacts() {
    let tmp = TempDir::new().unwrap();
    let project = make_old_project(tmp.path(), "old-app");

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("project-artifacts"))
        .expect("project-artifacts row in the scan table");
    assert!(
        row.contains("pruneable"),
        "stale artifacts must be pruneable: {row}"
    );
    assert!(
        project.join("node_modules").exists(),
        "scan must never delete"
    );
}

#[test]
fn clean_project_artifacts_dry_run_reports_without_deleting() {
    let tmp = TempDir::new().unwrap();
    let project = make_old_project(tmp.path(), "old-app");

    let output = sasurahime(tmp.path())
        .args(["clean", "project-artifacts", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("[dry-run] would remove:"),
        "the dry-run plan must be shown, got:\n{combined}"
    );
    assert!(
        combined.contains("old-app/node_modules"),
        "the artifact path must be in the plan, got:\n{combined}"
    );
    assert!(
        project.join("node_modules").exists(),
        "dry-run must not delete"
    );
}

#[test]
fn clean_project_artifacts_removes_stale_artifacts() {
    let tmp = TempDir::new().unwrap();
    let project = make_old_project(tmp.path(), "old-app");

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "project-artifacts"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !project.join("node_modules").exists(),
        "stale artifacts must be removed, got:\n{combined}"
    );
    assert!(
        project.join("package.json").exists(),
        "the manifest must be kept"
    );
    assert!(project.join("src.js").exists(), "the source must be kept");
    assert!(
        combined.contains("Permanently deleted: no Trash copy was kept."),
        "the audit trail must be shown, got:\n{combined}"
    );
}

#[test]
fn recent_projects_are_excluded_with_a_reason() {
    let tmp = TempDir::new().unwrap();
    let recent = make_recent_project(tmp.path(), "active-app");
    let _ = make_old_project(tmp.path(), "old-app");

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "project-artifacts"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        recent.join("node_modules").exists(),
        "recent projects must not be touched, got:\n{combined}"
    );
}

#[test]
fn artifact_without_manifest_is_not_detected() {
    let tmp = TempDir::new().unwrap();
    let orphan = tmp.path().join("Playground").join("not-a-project");
    fs::create_dir_all(orphan.join("node_modules")).unwrap();
    fs::write(orphan.join("node_modules/lib.js"), [0u8; 4096]).unwrap();
    // No package.json next to node_modules.

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout.lines().find(|l| l.contains("project-artifacts"));
    let not_detected = match row {
        None => true,
        Some(l) => l.contains("not found"),
    };
    assert!(
        not_detected,
        "an artifact without a manifest must not be detected, got: {:?}",
        row
    );
    assert!(orphan.join("node_modules").exists());
}

#[test]
fn yes_does_not_run_project_artifacts() {
    let tmp = TempDir::new().unwrap();
    let project = make_old_project(tmp.path(), "old-app");

    let output = sasurahime(tmp.path())
        .args(["--permanent", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("project-artifacts (Manual)"),
        "--yes must list project-artifacts as skipped with its classification, got:\n{combined}"
    );
    assert!(
        project.join("node_modules").exists(),
        "--yes must not run the Manual cleaner"
    );
}

#[test]
fn config_roots_and_recency_are_honored() {
    let tmp = TempDir::new().unwrap();
    let custom_root = tmp.path().join("Code");
    let project = custom_root.join("old-app");
    fs::create_dir_all(project.join("node_modules")).unwrap();
    fs::write(project.join("package.json"), b"{}").unwrap();
    fs::write(project.join("node_modules/lib.js"), [0u8; 4096]).unwrap();
    fs::write(project.join("src.js"), b"x").unwrap();
    make_old(&project.join("src.js"));
    make_old(&project.join("package.json"));
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        "[project-artifacts]\nroots = [\"~/Code\"]\nmax_depth = 3\nrecency_days = 7\n",
    )
    .unwrap();

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "project-artifacts"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !project.join("node_modules").exists(),
        "the configured root must be scanned, got:\n{combined}"
    );
}
