use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let bin = env!("CARGO_BIN_EXE_sasurahime");
    let mut cmd = Command::new(bin);
    cmd.env("HOME", home);
    cmd
}

fn write_policy(home: &std::path::Path, policy: &str) {
    let config_dir = home.join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), policy).unwrap();
}

fn make_old_act_cache(home: &std::path::Path) -> std::path::PathBuf {
    let act_cache = home.join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), [0u8; 4096]).unwrap();
    // 2020-01-01: comfortably older than any age threshold.
    let status = Command::new("touch")
        .args(["-t", "202001010000"])
        .arg(&act_cache)
        .status()
        .expect("touch available on macOS");
    assert!(status.success());
    act_cache
}

#[test]
fn auto_without_policy_refuses_to_run() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("auto").output().unwrap();
    assert!(
        !output.status.success(),
        "no [policy] section must refuse to delete anything"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("no [policy] section"),
        "the refusal must explain how to fix it, got:\n{combined}"
    );
}

#[test]
fn auto_refuses_when_config_exists_without_policy_section() {
    // A pre-existing config.toml with no [policy] section (here: only
    // trash_mode = false) must not unlock unattended deletes: the no-policy
    // gate must refuse before anything is removed.
    let tmp = TempDir::new().unwrap();
    let act_cache = make_old_act_cache(tmp.path());
    write_policy(tmp.path(), "trash_mode = false\n");

    let output = sasurahime(tmp.path()).arg("auto").output().unwrap();
    assert!(
        !output.status.success(),
        "an existing config without [policy] must refuse `auto`"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("no [policy] section"),
        "the refusal must name the missing [policy] section, got:\n{combined}"
    );
    assert!(
        act_cache.exists(),
        "the refusal must happen before any deletion, got:\n{combined}"
    );
}

#[test]
fn auto_dry_run_prints_the_plan_without_deleting() {
    let tmp = TempDir::new().unwrap();
    let act_cache = make_old_act_cache(tmp.path());
    write_policy(tmp.path(), "[policy]\nolder_than_days = 30\n");

    let output = sasurahime(tmp.path())
        .args(["auto", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        act_cache.exists(),
        "dry-run must not delete, got:\n{combined}"
    );
    assert!(
        combined.contains("Total freed:"),
        "the total must be printed, got:\n{combined}"
    );
}

#[test]
fn auto_applies_the_policy_and_leaves_history() {
    let tmp = TempDir::new().unwrap();
    let act_cache = make_old_act_cache(tmp.path());
    write_policy(tmp.path(), "[policy]\nolder_than_days = 30\n");

    let output = sasurahime(tmp.path())
        .args(["--permanent", "auto"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !act_cache.exists(),
        "the policy-allowed target must be cleaned, got:\n{combined}"
    );

    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    assert!(
        history_path.exists(),
        "unattended results must land in history"
    );
    let raw = fs::read_to_string(&history_path).unwrap();
    assert!(
        raw.contains("act"),
        "history must record the cleaned target"
    );
}

#[test]
fn auto_skips_risky_targets_with_guidance() {
    let tmp = TempDir::new().unwrap();
    write_policy(
        tmp.path(),
        "[policy]\nolder_than_days = 30\ninclude_manual = true\n",
    );

    let output = sasurahime(tmp.path()).arg("auto").output().unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("never runs unattended"),
        "irreversible targets must be skipped with guidance, got:\n{combined}"
    );
    assert!(
        combined.contains("clean <target>"),
        "the explicit-run guidance must be shown, got:\n{combined}"
    );
}

#[test]
fn auto_with_age_filter_skips_fresh_entries() {
    let tmp = TempDir::new().unwrap();
    // Fresh act cache (created now): the global filter must keep it.
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), [0u8; 4096]).unwrap();
    write_policy(tmp.path(), "[policy]\nolder_than_days = 30\n");

    let output = sasurahime(tmp.path())
        .args(["--permanent", "auto"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        act_cache.exists(),
        "fresh entries must not be deleted under the policy, got:\n{combined}"
    );
    assert!(
        combined.contains("younger than 30 day(s)"),
        "the skip reason must be shown, got:\n{combined}"
    );
}

#[test]
fn auto_never_runs_manual_targets_even_with_include_manual() {
    // A Manual target (downloads: whole-dir removal) must never run
    // unattended, even when [policy] sets include_manual = true.
    let tmp = TempDir::new().unwrap();
    write_policy(
        tmp.path(),
        "[policy]\nolder_than_days = 30\ninclude_manual = true\n",
    );
    let downloads = tmp.path().join("Downloads");
    fs::create_dir_all(&downloads).unwrap();
    fs::write(downloads.join("installer.dmg"), [0u8; 4096]).unwrap();
    let status = Command::new("touch")
        .args(["-t", "202001010000"])
        .arg(&downloads)
        .status()
        .unwrap();
    assert!(status.success());
    let output = sasurahime(tmp.path()).arg("auto").output().unwrap();
    assert!(output.status.success(), "auto must succeed");
    assert!(
        downloads.join("installer.dmg").exists(),
        "a Manual target must never be deleted by auto, even with include_manual"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("downloads"),
        "the skipped Manual target must be reported, got:\n{combined}"
    );
}

#[test]
fn auto_runs_moderate_targets_when_include_manual() {
    // [policy] include_manual = true must let Moderate targets (node-gyp)
    // run in auto; only Risky and Manual are blocked unconditionally.
    let tmp = TempDir::new().unwrap();
    write_policy(
        tmp.path(),
        "[policy]\nolder_than_days = 30\ninclude_manual = true\n",
    );
    let cache = tmp.path().join(".cache/node-gyp");
    fs::create_dir_all(&cache).unwrap();
    fs::write(cache.join("api.json"), [0u8; 4096]).unwrap();
    let status = Command::new("touch")
        .args(["-t", "202001010000"])
        .arg(&cache)
        .status()
        .unwrap();
    assert!(status.success());
    let output = sasurahime(tmp.path())
        .args(["--permanent", "auto"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        !cache.exists(),
        "[policy] include_manual = true must let Moderate targets run in auto"
    );
}
