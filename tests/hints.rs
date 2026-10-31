// PBI-83: the hint cleaning flow joins the Cleaner-trait registry as the
// `hints` target. These E2E tests pin the CLI surface:
//
// - `sasurahime clean hints` runs quit → delete → relaunch for the
//   KNOWN_ENTRIES candidates and writes ONE history row named "hints"
//   (1 cleaner = 1 name);
// - `--sub <display name>` scopes the run to that entry only;
// - `--dry-run` never deletes anything and lists what would be cleaned;
// - an unknown sub-target name exits non-zero (PBI 63 shape: an unknown
//   spelling is an error, not a silent drop);
// - `targets` lists `hints` (also pinned by tests/coverage.rs);
// - `scan` shows the hints row (not found when no candidate has debris);
// - a known name outside the top 5 (PBI-100: the explicit selection is the
//   threshold judgment) is still cleaned via `--sub`;
// - below-threshold debris behind an explicit selection is still cleaned
//   (PBI-100: the clean never re-runs the collect_hints threshold filter).
//
// The E2E debris uses the no-quit KNOWN_ENTRIES ("Diagnostic Reports",
// "Crash Reporter logs": no process, no osascript, no relaunch), so the runs
// are deterministic in a tempdir HOME. The quit → delete → relaunch order is
// pinned by the RecordingRunner tests in src/hint/mod.rs.

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

fn combined_output(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// 2 MB of debris in a KNOWN_ENTRIES directory, a multiple of the 4096-byte
/// APFS page so `format::dir_size` (st_blocks × 512) measures it exactly —
/// comfortably above the 1 MB log threshold.
const DEBRIS_BYTES: usize = 2_097_152;

/// Creates debris for the no-quit "Diagnostic Reports" hint entry
/// (~/Library/Logs/DiagnosticReports, threshold 1 MB).
fn make_diagnostic_reports(home: &std::path::Path) -> std::path::PathBuf {
    let dir = home.join("Library/Logs/DiagnosticReports");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash-report.ips"), vec![0u8; DEBRIS_BYTES]).unwrap();
    dir
}

/// Creates debris for the no-quit "Crash Reporter logs" hint entry
/// (~/Library/Logs/CrashReporter, threshold 1 MB).
fn make_crash_reporter_logs(home: &std::path::Path) -> std::path::PathBuf {
    let dir = home.join("Library/Logs/CrashReporter");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash-reporter.log"), vec![0u8; DEBRIS_BYTES]).unwrap();
    dir
}

/// Creates `size_bytes` of debris for the no-quit "Diagnostic Reports" hint
/// entry (~/Library/Logs/DiagnosticReports, threshold 1 MB), a multiple of
/// the 4096-byte APFS page so `format::dir_size` (st_blocks × 512) measures
/// it exactly.
fn make_diagnostic_reports_with_bytes(
    home: &std::path::Path,
    size_bytes: usize,
) -> std::path::PathBuf {
    let dir = home.join("Library/Logs/DiagnosticReports");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash-report.ips"), vec![0u8; size_bytes]).unwrap();
    dir
}

/// Creates `size_bytes` of debris in a KNOWN_ENTRIES log directory
/// (~/Library/Logs/<log_dir>, threshold 1 MB), a multiple of the 4096-byte
/// APFS page so `format::dir_size` (st_blocks × 512) measures it exactly.
fn make_log_dir_debris(
    home: &std::path::Path,
    log_dir: &str,
    size_bytes: usize,
) -> std::path::PathBuf {
    let dir = home.join("Library/Logs").join(log_dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("debris.log"), vec![0u8; size_bytes]).unwrap();
    dir
}

#[test]
fn clean_hints_deletes_hint_debris_and_writes_one_history_row() {
    let tmp = TempDir::new().unwrap();
    let dir = make_diagnostic_reports(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "hints"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "clean hints must succeed, got:\n{combined}"
    );
    assert!(
        !dir.exists(),
        "the hint debris must be deleted, got:\n{combined}"
    );

    // One history row named "hints" (1 cleaner = 1 name) for the bytes the
    // run actually freed.
    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    let entries = sasurahime::history::load_history(&history_path);
    assert_eq!(
        entries.len(),
        1,
        "one history row for the hints clean, got: {entries:?}"
    );
    assert_eq!(
        entries[0].cleaner, "hints",
        "the row names the hints cleaner"
    );
    assert_eq!(
        entries[0].freed_bytes, DEBRIS_BYTES as u64,
        "the freed bytes are recorded"
    );
}

#[test]
fn clean_hints_sub_cleans_only_that_entry() {
    let tmp = TempDir::new().unwrap();
    let diagnostic = make_diagnostic_reports(tmp.path());
    let crash_reporter = make_crash_reporter_logs(tmp.path());

    let output = sasurahime(tmp.path())
        .args([
            "--permanent",
            "clean",
            "hints",
            "--sub",
            "Diagnostic Reports",
        ])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "clean hints --sub must succeed, got:\n{combined}"
    );
    assert!(
        !diagnostic.exists(),
        "the selected entry must be cleaned, got:\n{combined}"
    );
    assert!(
        crash_reporter.exists(),
        "the other candidate must survive, got:\n{combined}"
    );
}

#[test]
fn clean_hints_dry_run_deletes_nothing_and_lists_the_plan() {
    let tmp = TempDir::new().unwrap();
    let dir = make_diagnostic_reports(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "hints", "--dry-run"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "the dry run must succeed, got:\n{combined}"
    );
    assert!(
        combined.contains("[dry-run] would"),
        "the dry run must list what would be cleaned, got:\n{combined}"
    );
    assert!(
        dir.exists(),
        "a dry run must never delete anything, got:\n{combined}"
    );
}

#[test]
fn clean_hints_unknown_sub_exits_nonzero() {
    // PBI 63 shape: an unknown sub-target spelling is an error, not a
    // silent no-op clean.
    let tmp = TempDir::new().unwrap();
    make_diagnostic_reports(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "hints", "--sub", "No Such App"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        !output.status.success(),
        "an unknown sub-target must fail, got:\n{combined}"
    );
    assert!(
        combined.contains("unknown hints sub-target"),
        "the error must name the unknown sub-target, got:\n{combined}"
    );
}

#[test]
fn targets_lists_hints() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).args(["targets"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.lines().any(|l| l.starts_with("hints")),
        "`targets` must list the hints target, got:\n{stdout}"
    );
}

/// The first cell of the scan table row whose category is `hints`
/// (`--no-unicode` rows look like `| hints | 2.0 MB | pruneable | ... |`).
fn hints_scan_row(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .find(|l| {
            l.split('|')
                .nth(1)
                .map(|c| c.trim() == "hints")
                .unwrap_or(false)
        })
        .map(|l| l.to_string())
}

#[test]
fn scan_shows_the_hints_row_not_found_without_debris() {
    // BDD: with no candidate above its threshold, `scan` shows the hints
    // row as not found.
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["--no-unicode", "scan"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = hints_scan_row(&stdout).expect("`scan` must show the hints row");
    assert!(
        row.contains("not found"),
        "the hints row must report not found without debris, got: {row}"
    );
}

#[test]
fn scan_shows_the_hints_row_pruneable_with_debris() {
    let tmp = TempDir::new().unwrap();
    make_diagnostic_reports(tmp.path());
    let output = sasurahime(tmp.path())
        .args(["--no-unicode", "scan"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = hints_scan_row(&stdout).expect("`scan` must show the hints row");
    assert!(
        row.contains("pruneable"),
        "the hints row must report the aggregate as pruneable, got: {row}"
    );
    assert!(
        row.contains("Moderate"),
        "the hints row must report the Moderate classification, got: {row}"
    );
}

/// Slack's KNOWN_ENTRIES threshold, read from the table the hint flow
/// actually uses (src/hint/apps.rs).
fn slack_threshold_bytes() -> u64 {
    sasurahime::hint::apps::KNOWN_ENTRIES
        .iter()
        .find(|e| e.display_name == "Slack")
        .expect("the Slack entry is in KNOWN_ENTRIES")
        .threshold_bytes
}

#[test]
fn clean_hints_refuses_to_delete_through_an_intermediate_symlink() {
    // PBI-89: ~/Library/Application Support/Slack is a symlink to
    // ~/Documents/重要 — the 中間 symlink + 実在リーフ (Slack/Cache)
    // configuration. The metric collect_hints uses counts the real bytes
    // through the intermediate symlink (walkdir does not follow a symlinked
    // walk root, but the symlink sits above the root here), so the hint flow
    // reaches the deletion — and the symlink scope check must refuse it
    // fail-closed instead of wiping the real data.
    let tmp = TempDir::new().unwrap();
    let real = tmp.path().join("Documents/重要");
    let cache = real.join("Cache");
    fs::create_dir_all(&cache).unwrap();
    // 80 MB: comfortably above Slack's 64 MB threshold, a multiple of the
    // 4096-byte APFS page so dir_size (st_blocks x 512) measures it.
    let debris = vec![0u8; 80 * 1024 * 1024];
    fs::write(cache.join("important.db"), &debris).unwrap();

    let app_support = tmp.path().join("Library/Application Support");
    fs::create_dir_all(&app_support).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real, app_support.join("Slack")).unwrap();

    // (a) dir_size 計上 pin: the real bytes are counted through the
    // intermediate symlink, so the hint crosses the threshold and the
    // deletion flow is reached (計上が通っても削除は fail closed).
    let hint_path = app_support.join("Slack/Cache");
    let measured = sasurahime::format::dir_size(&hint_path);
    let threshold = slack_threshold_bytes();
    assert!(
        measured > threshold,
        "dir_size must count the real bytes through the intermediate symlink \
         so the hint reaches the deletion flow: measured {measured}, threshold {threshold}"
    );

    let output = sasurahime(tmp.path())
        .args(["--permanent", "clean", "hints"])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    // (b) The real data survives: the deletion is refused fail-closed.
    assert!(
        output.status.success(),
        "the refused hint must not fail the run, got:\n{combined}"
    );
    assert!(
        cache.join("important.db").exists(),
        "the real data behind the intermediate symlink must survive, got:\n{combined}"
    );
}

#[test]
fn clean_hints_sub_deletes_a_known_name_outside_the_top5() {
    // PBI-100: the explicit selection is the threshold judgment. A known
    // name that sorts below the top 5 of the display list used to drop out
    // of the re-derived selection and become a silent no-op (debris alive,
    // exit 0); the named entry must be cleaned instead. 1.5 MB of
    // Diagnostic Reports sorts below the five 2 MB log candidates, so the
    // display list (largest first, top 5) truncates it away — and the
    // non-selected candidates' running probes (pgrep) cannot influence the
    // selected entry's clean, so the run stays deterministic.
    let tmp = TempDir::new().unwrap();
    let diagnostic = make_diagnostic_reports_with_bytes(tmp.path(), 1_572_864);
    let crash_reporter = make_crash_reporter_logs(tmp.path());
    let claude = make_log_dir_debris(tmp.path(), "Claude", DEBRIS_BYTES);
    let zoom = make_log_dir_debris(tmp.path(), "zoom.us", DEBRIS_BYTES);
    let lm_studio = make_log_dir_debris(tmp.path(), "LM Studio", DEBRIS_BYTES);
    let fsck = make_log_dir_debris(tmp.path(), "fsck_hfs.log", DEBRIS_BYTES);

    let output = sasurahime(tmp.path())
        .args([
            "--permanent",
            "clean",
            "hints",
            "--sub",
            "Diagnostic Reports",
        ])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "clean hints --sub must succeed, got:\n{combined}"
    );
    assert!(
        !diagnostic.exists(),
        "a known name outside the top 5 must still be cleaned, got:\n{combined}"
    );
    assert!(
        crash_reporter.exists()
            && claude.exists()
            && zoom.exists()
            && lm_studio.exists()
            && fsck.exists(),
        "the other candidates must survive, got:\n{combined}"
    );
}

#[test]
fn clean_hints_sub_cleans_below_threshold_debris_without_rejudging() {
    // PBI-100: the explicit selection is the threshold judgment — the clean
    // must not re-run collect_hints' threshold filter, so debris below the
    // 1 MB threshold (e.g. logs rotated or shrunk between the offer and the
    // clean) is still cleaned and the history row is written.
    let tmp = TempDir::new().unwrap();
    let diagnostic = make_diagnostic_reports_with_bytes(tmp.path(), 4_096);

    let output = sasurahime(tmp.path())
        .args([
            "--permanent",
            "clean",
            "hints",
            "--sub",
            "Diagnostic Reports",
        ])
        .output()
        .unwrap();
    let combined = combined_output(&output);
    assert!(
        output.status.success(),
        "clean hints --sub must succeed, got:\n{combined}"
    );
    assert!(
        !diagnostic.exists(),
        "below-threshold debris behind an explicit selection must still be \
         cleaned, got:\n{combined}"
    );

    // The history row is written for the bytes the run actually freed.
    let history_path = tmp.path().join(".local/share/sasurahime/history.json");
    let entries = sasurahime::history::load_history(&history_path);
    assert_eq!(
        entries.len(),
        1,
        "one history row for the hints clean, got: {entries:?}"
    );
    assert_eq!(
        entries[0].cleaner, "hints",
        "the row names the hints cleaner"
    );
    assert_eq!(
        entries[0].freed_bytes, 4_096,
        "the freed bytes are recorded"
    );
}
