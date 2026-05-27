use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn missing_config_does_not_crash() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
}

#[test]
fn invalid_config_exits_nonzero_with_message() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "not valid toml :::").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error loading config") || stderr.contains("config parse error"),
        "expected error message in stderr, got:\n{stderr}"
    );
}

#[test]
fn exclude_removes_cleaner_from_scan() {
    let tmp = TempDir::new().unwrap();
    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "exclude = [\"act\"]\n").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("act"),
        "excluded cleaner 'act' should not appear in scan output, got:\n{stdout}"
    );
}

#[test]
fn exclude_maintains_independent_cleaner() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "exclude = [\"uv\"]\n").unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("uv"),
        "excluded cleaner 'uv' should not appear in scan output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("brew"),
        "non-excluded cleaner 'brew' should still appear, got:\n{stdout}"
    );
}

#[test]
fn exclude_does_not_block_direct_clean() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "exclude = [\"act\"]\n").unwrap();

    let output = sasurahime(tmp.path())
        .arg("clean")
        .arg("act")
        .arg("--dry-run")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "direct clean of excluded target 'act' should still work, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn custom_config_path_overrides_default() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.toml"), "not valid toml :::").unwrap();
    let custom_path = tmp.path().join("my-config.toml");
    fs::write(&custom_path, "trash_mode = false\n").unwrap();

    let output = sasurahime(tmp.path())
        .arg("--config")
        .arg(&custom_path)
        .arg("scan")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "custom config path should override default (invalid) config, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn custom_target_appears_in_scan() {
    let tmp = TempDir::new().unwrap();
    // Create a custom cache dir with files
    let custom_path = tmp.path().join("tmp").join("cache");
    fs::create_dir_all(&custom_path).unwrap();
    fs::write(custom_path.join("data.bin"), b"content").unwrap();

    // Write config with [[custom]] entry pointing to it
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    let toml = format!(
        "[[custom]]\nname = \"my-workspace\"\npath = \"{path}\"\n",
        path = custom_path.to_string_lossy()
    );
    fs::write(config_dir.join("config.toml"), &toml).unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my-workspace"),
        "custom target should appear in scan output, got:\n{stdout}"
    );
    assert!(
        stdout.contains("pruneable"),
        "custom target with content should show as pruneable, got:\n{stdout}"
    );
}

#[test]
fn custom_target_nonexistent_path_shows_not_found() {
    let tmp = TempDir::new().unwrap();
    // Write config with [[custom]] entry pointing to a non-existent path
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    let toml = "[[custom]]\nname = \"my-workspace\"\npath = \"~/tmp/nonexistent\"\n";
    fs::write(config_dir.join("config.toml"), toml).unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("my-workspace"),
        "custom target should appear in scan output even when not found, got:\n{stdout}"
    );
    assert!(
        stdout.contains("not found"),
        "custom target without content should show as not found, got:\n{stdout}"
    );
}

#[test]
fn per_cleaner_older_than_days_hides_new_files_in_scan() {
    use filetime::FileTime;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    // Create two cache dirs for "act" cleaner (which uses DeleteDirs)
    let cache_dir = tmp.path().join(".cache/act");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("data"), b"content").unwrap();
    // Set mtime to 60 days ago (old enough to pass older_than_days = 30)
    let old_mtime = SystemTime::now() - Duration::from_secs(60 * 86_400);
    filetime::set_file_mtime(&cache_dir, FileTime::from_system_time(old_mtime)).unwrap();

    // Create a new cache dir that should be excluded
    let new_cache = tmp.path().join(".cache/new-tool");
    fs::create_dir_all(&new_cache).unwrap();
    fs::write(new_cache.join("data"), b"new content").unwrap();
    // Don't set mtime — it stays current

    // Config: older_than_days = 30 for "act" (which uses DeleteDirs with act cache)
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        "[cleaner.act]\nolder_than_days = 30\n",
    )
    .unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());

    // Compute expected physical size of the aged act cache dir
    use std::os::unix::fs::MetadataExt;
    let expected_size: u64 = std::fs::metadata(cache_dir.join("data"))
        .map(|m| m.blocks() * 512)
        .unwrap_or(0);
    assert!(expected_size > 0, "aged cache dir must have non-zero size");

    let stdout_normal = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout_normal.to_lowercase();
    // The old act cache should be pruneable, new-tool should not appear
    // (it's not a registered cleaner)
    assert!(
        stdout.contains("act"),
        "act should appear in scan since old dir is filter-passing, got:\n{stdout}"
    );
    assert!(
        stdout.contains("pruneable"),
        "act should be pruneable since old dir passes age filter, got:\n{stdout}"
    );
    // Check the formatted size appears in the output
    let formatted = if expected_size >= 1_073_741_824 {
        format!("{:.1} gb", expected_size as f64 / 1_073_741_824.0)
    } else if expected_size >= 1_048_576 {
        format!("{:.1} mb", expected_size as f64 / 1_048_576.0)
    } else if expected_size >= 1024 {
        format!("{:.1} kb", expected_size as f64 / 1024.0)
    } else {
        format!("{} b", expected_size)
    };
    assert!(
        stdout.contains(&formatted),
        "expected formatted size '{}' should appear in scan output, got:\n{stdout}",
        formatted
    );
}

#[test]
fn per_cleaner_larger_than_mb_hides_small_files_in_scan() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    // Create cache dir with small files (below threshold)
    let cache_dir = tmp.path().join(".cache/act");
    fs::create_dir_all(&cache_dir).unwrap();
    fs::write(cache_dir.join("small"), b"x").unwrap();

    // Config: larger_than_mb = 100 for "act"
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        "[cleaner.act]\nlarger_than_mb = 100\n",
    )
    .unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    // The act cache is small (< 100 MB), so it should appear as "not found"
    // (filtered out) rather than "pruneable"
    assert!(
        stdout.contains("act") && stdout.contains("not found"),
        "act should appear but as 'not found' since cache is too small, got:\n{stdout}"
    );
}

#[test]
fn per_cleaner_older_than_days_for_logs_affects_scan() {
    use filetime::FileTime;
    use std::fs;
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    // Create log files in the kilo log dir
    let log_dir = tmp.path().join(".local/share/kilo/log");
    fs::create_dir_all(&log_dir).unwrap();
    // Write a recent log file (should be kept)
    fs::write(log_dir.join("recent.log"), b"recent").unwrap();
    // Write an old log file (should be cleaned)
    fs::write(log_dir.join("old.log"), b"old content").unwrap();
    let old_mtime = SystemTime::now() - Duration::from_secs(60 * 86_400);
    filetime::set_file_mtime(
        log_dir.join("old.log"),
        FileTime::from_system_time(old_mtime),
    )
    .unwrap();

    // Config: older_than_days = 7 for "logs"
    // With keep_days=7, a 60-day-old log is pruneable
    let config_dir = tmp.path().join(".config/sasurahime");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        "[cleaner.logs]\nolder_than_days = 7\n",
    )
    .unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
    // Logs should appear as pruneable since old.log exists
    assert!(
        stdout.contains("logs"),
        "logs should appear in scan with old log file, got:\n{stdout}"
    );
}
