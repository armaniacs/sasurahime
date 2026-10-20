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
