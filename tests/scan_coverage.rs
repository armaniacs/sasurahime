use assert_cmd::Command;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn install_script(bin_dir: &Path, name: &str, body: &str) {
    fs::create_dir_all(bin_dir).unwrap();
    let path = bin_dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn write_docker_raw(home: &Path) {
    let p = home.join("Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let mut f = fs::File::create(&p).unwrap();
    f.write_all(&vec![1u8; 2 * 1024 * 1024]).unwrap();
    f.set_len(64 * 1024 * 1024).unwrap();
}

#[test]
fn scan_shows_docker_virtual_disk_advice() {
    let tmp = TempDir::new().unwrap();
    write_docker_raw(tmp.path());
    let bin = tmp.path().join("bin");
    install_script(&bin, "docker", "exit 0");

    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Docker.raw"), "stdout: {stdout}");
    assert!(stdout.contains("on disk:"), "stdout: {stdout}");
    assert!(stdout.contains("Disk image size"), "stdout: {stdout}");
}

#[test]
fn scan_omits_docker_advice_when_docker_is_not_installed() {
    let tmp = TempDir::new().unwrap();
    write_docker_raw(tmp.path());

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .arg("scan")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Docker.raw"), "stdout: {stdout}");
}

#[test]
fn scan_does_not_hang_when_docker_daemon_is_unresponsive() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("bin");
    install_script(&bin, "docker", "sleep 120");

    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .timeout(Duration::from_secs(25))
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
}

const FAKE_TOOLS: &[&str] = &[
    "uv", "brew", "mise", "bun", "go", "pip", "npm", "yarn", "pnpm", "deno", "pipx", "docker",
    "orb", "gem", "bundle", "dotnet", "pod", "conda", "poetry", "colima", "xcrun", "mvn", "dart",
    "rustup",
];

const STANDARD_SCAN_NAMES: &[&str] = &[
    "act",
    "uv",
    "brew",
    "mise",
    "browsers",
    "bun",
    "go",
    "pip",
    "node-gyp",
    "npm",
    "yarn",
    "pnpm",
    "cargo",
    "docker",
    "orbstack",
    "cocoapods",
    "colima",
    "spm",
    "conda",
    "poetry",
    "pipx",
    "deno",
    "dotnet",
    "gem",
    "bundle",
    "rustup",
    "simulator",
    "gradle",
    "huggingface",
    "pre-commit",
    "jetbrains",
    "downloads",
    "vscode-extensions",
    "maven",
    "terraform",
    "flutter",
    "volta",
    "sbt",
    "tree-sitter",
];

fn first_cells(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| l.split('|').nth(1).map(|c| c.trim().to_string()))
        .collect()
}

fn scan_with_fake_tools(home: &Path) -> String {
    let bin = home.join("bin");
    for tool in FAKE_TOOLS {
        install_script(&bin, tool, "exit 0");
    }
    let output = sasurahime(home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .args(["--no-unicode", "scan"])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn scan_lists_every_standard_target() {
    let tmp = TempDir::new().unwrap();
    let stdout = scan_with_fake_tools(tmp.path());
    let cells = first_cells(&stdout);
    let missing: Vec<&&str> = STANDARD_SCAN_NAMES
        .iter()
        .filter(|n| !cells.iter().any(|c| c == **n))
        .collect();
    assert!(
        missing.is_empty(),
        "missing from scan: {missing:?}\n{stdout}"
    );
}

#[test]
fn scan_does_not_print_cargo_progress_lines() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("proj/target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("artifact"), vec![1u8; 4096]).unwrap();
    let stdout = scan_with_fake_tools(tmp.path());
    assert!(!stdout.contains("[cargo]"), "stdout: {stdout}");
}
