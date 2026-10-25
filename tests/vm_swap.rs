use assert_cmd::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn scan_shows_vm_swap_row_with_size_and_uptime() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("vm-swap"))
        .expect("vm-swap row in the scan table");
    // Manual target: the risk column classifies it.
    assert!(row.contains("Manual"), "vm-swap must be manual, got: {row}");
    // The uptime is written alongside the target (sysctl kern.boottime).
    assert!(
        stdout.contains("uptime"),
        "uptime days must be shown, got:\n{stdout}"
    );
}

#[test]
fn clean_vm_swap_is_a_no_op_with_reboot_guidance() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "vm-swap"])
        .output()
        .unwrap();

    assert!(output.status.success(), "clean vm-swap must exit 0");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("reboot"),
        "clean must explain that swap is re-initialized on reboot, got:\n{combined}"
    );
    // Nothing is deleted: the clean is a no-op by design, so the output
    // reports the no-op and never a removal. The run used a fake HOME, so
    // the real swap directory is untouched and must not be probed here: on
    // a CI runner /private/var/vm exists but holds no swapfiles.
    assert!(
        combined.contains("Nothing was deleted"),
        "vm-swap clean must report the no-op, got:\n{combined}"
    );
    assert!(
        !combined.contains("Removed"),
        "vm-swap clean must not report deletions, got:\n{combined}"
    );
}

#[test]
fn targets_lists_vm_swap() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("vm-swap"),
        "targets must list vm-swap, got:\n{stdout}"
    );
}
