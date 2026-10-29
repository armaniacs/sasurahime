use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path, path: &str) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd.env("PATH", path);
    cmd
}

fn install_plugin(dir: &std::path::Path, name: &str, body: &str) {
    let plugin = dir.join(name);
    fs::write(&plugin, body).unwrap();
    fs::set_permissions(&plugin, fs::Permissions::from_mode(0o755)).unwrap();
}

fn good_plugin_body() -> &'static str {
    "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nexit 0\n"
}

#[test]
fn plugin_appears_in_scan_and_total() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("foo"))
        .expect("foo row in the scan table");
    assert!(
        row.contains("pruneable"),
        "the plugin must be measured like any target: {row}"
    );
    assert!(
        stdout.contains("512 B"),
        "the plugin size must appear in the total"
    );
}

#[test]
fn failing_plugin_is_isolated_as_unavailable() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    install_plugin(&bin_dir, "sasurahime-broken", "#!/bin/sh\nexit 1\n");
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path).arg("scan").output().unwrap();
    assert!(
        output.status.success(),
        "one broken plugin must not take the scan down"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let broken_row = stdout
        .lines()
        .find(|l| l.contains("broken"))
        .expect("broken row");
    assert!(
        broken_row.contains("unavailable"),
        "the failing plugin's row must be unavailable: {broken_row}"
    );
    let foo_row = stdout.lines().find(|l| l.contains("foo")).expect("foo row");
    assert!(
        foo_row.contains("pruneable"),
        "other targets must display normally: {foo_row}"
    );
}

#[test]
fn hung_plugin_is_isolated_by_the_timeout() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-hung", "#!/bin/sh\nsleep 60\n");
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    // Override the plugin timeout so the test stays fast.
    let output = sasurahime(tmp.path(), &path)
        .env("SASURAHIME_PLUGIN_TIMEOUT_SECS", "1")
        .arg("scan")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "a hung plugin must not block the scan"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("hung"))
        .expect("hung row");
    assert!(
        row.contains("unavailable"),
        "the timed-out plugin must be unavailable: {row}"
    );
}

#[test]
fn targets_marks_plugin_derived_entries() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .arg("targets")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find(|l| l.starts_with("foo"))
        .expect("foo entry in targets");
    assert!(
        line.contains("(plugin)"),
        "plugin-derived targets must be marked: {line}"
    );
}

#[test]
fn plugin_clean_uses_the_protocol() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // A plugin whose clean deletes its cache dir for real (no Trash).
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nrm -rf \"$HOME/.cache/plugin-foo\"\nexit 0\n",
    );
    let cache = tmp.path().join(".cache/plugin-foo");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("x"), [0u8; 512]).unwrap();
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(output.status.success(), "the plugin clean must run");
    assert!(!cache.exists(), "the plugin must have cleaned its cache");
}

#[test]
fn chatty_plugin_is_measured_not_unavailable() {
    // A plugin writing more than the pipe buffer to stderr must be measured,
    // not killed at the deadline and reported unavailable.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-chatty",
        "#!/bin/sh\nhead -c 1048576 /dev/zero | tr '\\0' 'x' >&2\necho '{\"status\":\"pruneable\",\"bytes\":512}'\nexit 0\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());
    let output = sasurahime(tmp.path(), &path).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("chatty"))
        .expect("chatty row");
    assert!(
        row.contains("pruneable"),
        "a chatty plugin must be measured: {row}"
    );
}

#[test]
fn grandchild_holding_pipe_does_not_hang_the_scan() {
    // A plugin that exits but leaves a grandchild holding stdout must not
    // hang the scan past the timeout.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-daemon",
        "#!/bin/sh\n(sleep 30 &)\necho '{\"status\":\"clean\",\"bytes\":0}'\nexit 0\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());
    let start = std::time::Instant::now();
    let output = sasurahime(tmp.path(), &path)
        .env("SASURAHIME_PLUGIN_TIMEOUT_SECS", "1")
        .arg("scan")
        .output()
        .unwrap();
    let elapsed = start.elapsed();
    assert!(output.status.success(), "the scan must succeed");
    assert!(
        elapsed.as_secs() < 5,
        "the grandchild must not hang the scan: {elapsed:?}"
    );
    // The plugin's output could not be fully read within the timeout, so it
    // is isolated as unavailable: the important property is that the scan
    // returns fast instead of hanging on the grandchild.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("daemon"))
        .expect("daemon row");
    assert!(
        row.contains("unavailable"),
        "the plugin must be isolated: {row}"
    );
}

#[test]
fn clean_forwards_dry_run_to_the_plugin() {
    // The protocol contract "clean [--dry-run]" must actually forward the flag.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nprintf '%s\\n' \"$@\" > \"$HOME/args.txt\"\nexit 0\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());
    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let args = fs::read_to_string(tmp.path().join("args.txt")).unwrap();
    assert!(
        args.contains("--dry-run"),
        "clean --dry-run must forward --dry-run to the plugin, got: {args}"
    );
}

fn history_path(home: &std::path::Path) -> std::path::PathBuf {
    home.join(".local/share/sasurahime/history.json")
}

#[test]
fn plugin_clean_reports_freed_bytes_from_trailing_json() {
    // A plugin reporting {"freed": N} as the last JSON object on stdout has
    // that count recorded in history instead of a hardcoded 0.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\necho '[plugin] removing...'\necho '{\"freed\": 1234, \"uses_trash\": false}'\nexit 0\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(output.status.success(), "the plugin clean must run");

    let history = fs::read_to_string(history_path(tmp.path()))
        .expect("a clean that freed bytes must be recorded in history");
    assert!(
        history.contains("\"cleaner\": \"foo\"") && history.contains("\"freed_bytes\": 1234"),
        "the plugin's reported freed bytes must reach history: {history}"
    );
}

#[test]
fn plugin_clean_without_json_stays_backward_compatible() {
    // A plugin reporting by exit code only (the current protocol) keeps
    // working: success, 0 freed, no history row.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(&bin_dir, "sasurahime-foo", good_plugin_body());
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "a plugin without JSON must still succeed"
    );
    assert!(
        !history_path(tmp.path()).exists(),
        "a 0-freed plugin clean must not be recorded in history"
    );
}

#[test]
fn plugin_clean_with_malformed_json_falls_back_without_error() {
    // Malformed JSON is ignored: the exit code alone decides success, and the
    // clean falls back to 0 freed instead of erroring.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\necho '{\"freed\": '\nexit 0\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "malformed JSON must not fail the clean"
    );
    assert!(
        !history_path(tmp.path()).exists(),
        "malformed JSON falls back to 0 freed: no history row"
    );
}

#[test]
fn plugin_clean_does_not_pass_a_json_flag() {
    // WHY: --json is deliberately not passed. A plugin that inspects its
    // arguments could break on an unknown flag, so the trailing-JSON protocol
    // stays opt-in from the plugin's side.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\nif [ \"$1\" = \"scan\" ]; then echo '{\"status\":\"pruneable\",\"bytes\":512}'; exit 0; fi\nprintf '%s\\n' \"$@\" > \"$HOME/args.txt\"\necho '{\"freed\": 5, \"uses_trash\": false}'\nexit 0\n",
    );
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let args = fs::read_to_string(tmp.path().join("args.txt")).unwrap();
    assert_eq!(
        args.trim(),
        "clean",
        "the plugin must receive exactly `clean`, got: {args}"
    );
}

#[test]
fn plugin_scan_claim_is_capped_at_the_verified_size() {
    // PBI 66: a plugin's claimed `bytes` must not inflate the scan. A forged
    // u64::MAX claim is capped at the size sasurahime verified itself (the
    // plugin binary's own on-disk footprint) in the row and the total.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\necho '{\"status\":\"pruneable\",\"bytes\":18446744073709551615}'\nexit 0\n",
    );
    let plugin = bin_dir.join("sasurahime-foo");
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("18446744073709551615"),
        "the forged u64::MAX claim must never appear in the scan report:\n{stdout}"
    );

    // scan --json reports the capped raw byte count: the verified size.
    let json = sasurahime(tmp.path(), &path)
        .args(["scan", "--json"])
        .output()
        .unwrap();
    assert!(json.status.success());
    let stdout = String::from_utf8_lossy(&json.stdout);
    let verified = sasurahime::format::dir_size(&plugin);
    assert!(
        stdout.contains(&format!("\"size_bytes\": {}", verified)),
        "the row must report the verified size {verified}, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("18446744073709551615"),
        "the forged claim must never appear in scan --json:\n{stdout}"
    );
}

#[test]
fn plugin_freed_claim_is_capped_at_the_verified_size() {
    // PBI 66: a plugin's claimed `freed` must not forge the audit trail. A
    // 1000000-byte claim is capped at the size sasurahime verified itself
    // before `Total freed`, history and the trash note report it.
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_plugin(
        &bin_dir,
        "sasurahime-foo",
        "#!/bin/sh\necho '{\"freed\": 1000000, \"uses_trash\": false}'\nexit 0\n",
    );
    let plugin = bin_dir.join("sasurahime-foo");
    let path = format!("{}:/usr/bin:/bin", bin_dir.display());

    let output = sasurahime(tmp.path(), &path)
        .args(["clean", "foo"])
        .output()
        .unwrap();
    assert!(output.status.success(), "the plugin clean must run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let verified = sasurahime::format::dir_size(&plugin);

    let history = fs::read_to_string(history_path(tmp.path()))
        .expect("a capped-freed clean is still recorded in history");
    assert!(
        !history.contains("\"freed_bytes\": 1000000"),
        "the forged freed claim must never reach history: {history}"
    );
    assert!(
        history.contains(&format!("\"freed_bytes\": {}", verified)),
        "history must record the verified size {verified}, got: {history}"
    );
    assert!(
        stdout.contains(&format!(
            "Freed: {}",
            sasurahime::format::format_bytes(verified)
        )),
        "the Freed line must report the verified size, got:\n{stdout}"
    );
}
