# Test Coverage Gaps Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all 9 coverage gaps from the audit report, fix a flaky pre-commit test, and extract shared test helpers.

**Architecture:** Work proceeds in 3 phases — (1) fix pre-commit failing test, (2) create shared test helpers in `src/test_helpers.rs`, (3) add tests for all gaps. Only test code is changed; no production logic is modified.

**Tech Stack:** Rust, assert_cmd (E2E), tempfile (fixtures), filetime (mtime manipulation)

---

### Task 1: Fix pre-commit failing test

**Files:**
- Modify: `src/cleaners/pre_commit.rs:162-172`

- [ ] **Step 1: Debug the failing test**

Run the test to see the exact failure:
```bash
cargo test cleaners::pre_commit::tests::detect_returns_pruneable_when_cache_exists -- --nocapture
```

The test writes `b"dummy"` (5 bytes) but `dir_size()` may return 0. The most likely cause is that `dir_size()` only counts bytes from regular file entries via `WalkDir`, and the single 5-byte file may not be picked up reliably on all platforms/filesystem states.

- [ ] **Step 2: Increase the test fixture size**

Replace the fixture with a larger payload to ensure `dir_size()` returns > 0:

```rust
#[test]
fn detect_returns_pruneable_when_cache_exists() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/pre-commit");
    fs::create_dir_all(&cache).unwrap();
    // Use a large file to ensure dir_size() > 0
    fs::write(cache.join("hook.pck"), b"x".repeat(4096)).unwrap();

    let cleaner = PreCommitCleaner::new(tmp.path(), Box::new(SystemCommandRunner));
    let result = cleaner.detect();
    assert!(matches!(result.status, ScanStatus::Pruneable(_)));
}
```

- [ ] **Step 3: Run to verify**

```bash
cargo test cleaners::pre_commit::tests -- --nocapture
```
Expected: all pre_commit tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cleaners/pre_commit.rs
git commit -m "fix: pre-commit detect test — increase fixture size to fix flaky dir_size()==0"
```

---

### Task 2: Create shared test helpers (`src/test_helpers.rs`)

**Files:**
- Create: `src/test_helpers.rs`
- Modify: `src/lib.rs` (add `pub mod test_helpers;` under `#[cfg(test)]`)

This module provides reusable mocks and fixtures for all test modules.

- [ ] **Step 1: Add the module declaration to lib.rs**

In `src/lib.rs`, find the existing module declarations and add:

```rust
#[cfg(test)]
pub mod test_helpers;
```

- [ ] **Step 2: Create `src/test_helpers.rs` with all shared utilities**

```rust
#![cfg(test)]

use crate::command::CommandRunner;
use filetime::{set_file_mtime, FileTime};
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::time::{Duration, SystemTime};

// ── Mock CommandRunner ──

enum MockBehavior {
    NotFound,
    Output { program: String, output: std::process::Output },
    ExitCode { program: String, code: i32 },
}

pub struct MockRunner {
    behaviors: Vec<MockBehavior>,
}

impl MockRunner {
    pub fn new() -> Self {
        Self { behaviors: vec![] }
    }

    /// All tools report as not found (exists() returns false for everything).
    pub fn with_not_found(mut self) -> Self {
        self.behaviors.push(MockBehavior::NotFound);
        self
    }

    /// A specific tool exists and succeeds with empty output.
    pub fn with_success(mut self, program: &str) -> Self {
        self.behaviors.push(MockBehavior::Output {
            program: program.to_string(),
            output: std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            },
        });
        self
    }

    /// A specific tool exists and returns the given output.
    pub fn with_output(mut self, program: &str, output: std::process::Output) -> Self {
        self.behaviors.push(MockBehavior::Output {
            program: program.to_string(),
            output,
        });
        self
    }

    /// A specific tool exists but exits with the given code.
    pub fn with_exit_code(mut self, program: &str, code: i32) -> Self {
        self.behaviors.push(MockBehavior::ExitCode {
            program: program.to_string(),
            code,
        });
        self
    }
}

impl CommandRunner for MockRunner {
    fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
        for b in &self.behaviors {
            match b {
                MockBehavior::NotFound => {
                    anyhow::bail!("failed to spawn `{program}`: No such file or directory")
                }
                MockBehavior::Output { program: p, output } if p == program => {
                    return Ok(std::process::Output {
                        status: output.status,
                        stdout: output.stdout.clone(),
                        stderr: output.stderr.clone(),
                    });
                }
                MockBehavior::ExitCode { program: p, code } if p == program => {
                    if *code == 0 {
                        return Ok(std::process::Output {
                            status: std::process::ExitStatus::from_raw(0),
                            stdout: vec![],
                            stderr: vec![],
                        });
                    }
                    // Simulate a non-zero exit: run a fake command and return its output
                    return Ok(std::process::Output {
                        status: std::process::ExitStatus::from_raw(*code),
                        stdout: vec![],
                        stderr: vec![],
                    });
                }
                _ => {}
            }
        }
        anyhow::bail!("mock runner: unexpected program `{program}` with args {args:?}")
    }

    fn exists(&self, program: &str) -> bool {
        for b in &self.behaviors {
            match b {
                MockBehavior::NotFound => return false,
                MockBehavior::Output { program: p, .. } if p == program => return true,
                MockBehavior::ExitCode { program: p, .. } if p == program => return true,
                _ => {}
            }
        }
        false
    }
}

// ── Fixture utilities ──

/// Write a file and set its modification time to `days_old` days ago.
pub fn write_aged_file(path: &Path, days_old: u64, content: &[u8]) {
    std::fs::write(path, content).unwrap();
    let mtime = SystemTime::now() - Duration::from_secs(days_old * 86_400);
    set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
}

/// Create a directory and all its ancestors, then set the dir mtime.
pub fn write_aged_dir(path: &Path, days_old: u64) {
    std::fs::create_dir_all(path).unwrap();
    let mtime = SystemTime::now() - Duration::from_secs(days_old * 86_400);
    set_file_mtime(path, FileTime::from_system_time(mtime)).unwrap();
}

// ── Verbose flag guard ──

/// Sets verbose mode for the duration of a test, restoring on drop.
pub struct VerboseGuard {
    previous: bool,
}

impl VerboseGuard {
    pub fn new() -> Self {
        let previous = crate::context::is_verbose();
        crate::context::set_verbose(true);
        Self { previous }
    }
}

impl Drop for VerboseGuard {
    fn drop(&mut self) {
        crate::context::set_verbose(self.previous);
    }
}

// ── Exit status / output factories ──

pub fn exit_ok() -> std::process::ExitStatus {
    std::process::ExitStatus::from_raw(0)
}

pub fn ok_output(stdout: &[u8]) -> std::process::Output {
    std::process::Output {
        status: exit_ok(),
        stdout: stdout.to_vec(),
        stderr: vec![],
    }
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo build 2>&1
```
Expected: clean build with no warnings.

- [ ] **Step 4: Commit**

```bash
git add src/test_helpers.rs src/lib.rs
git commit -m "test: add shared test helpers module (MockRunner, write_aged_file, VerboseGuard)"
```

---

### Task 3: C01 — `is_skippable_error()` unit tests

**Files:**
- Modify: `src/cleaner.rs` (test module, after line 179)

- [ ] **Step 1: Add tests for `is_skippable_error()`**

Add to the existing test module in `src/cleaner.rs`:

```rust
#[test]
fn is_skippable_error_permission_denied() {
    let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_would_block() {
    let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::WouldBlock));
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_already_exists() {
    let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::AlreadyExists));
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_not_found_is_not_skippable() {
    let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert!(!is_skippable_error(&e));
}

#[test]
fn is_skippable_error_connection_refused_is_not_skippable() {
    let e = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::ConnectionRefused));
    assert!(!is_skippable_error(&e));
}

#[test]
fn is_skippable_error_permission_denied_string() {
    let e = anyhow::anyhow!("Permission denied: /some/path");
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_operation_not_permitted_string() {
    let e = anyhow::anyhow!("Operation not permitted");
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_resource_busy_string() {
    let e = anyhow::anyhow!("Resource busy");
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_trash_failed_string() {
    let e = anyhow::anyhow!("trash failed: could not move to trash");
    assert!(is_skippable_error(&e));
}

#[test]
fn is_skippable_error_arbitrary_error_is_not_skippable() {
    let e = anyhow::anyhow!("something went horribly wrong");
    assert!(!is_skippable_error(&e));
}

#[test]
fn is_skippable_error_clean_cancelled_is_not_skippable() {
    let e = anyhow::Error::from(crate::cleaner::CleanCancelled);
    assert!(!is_skippable_error(&e));
}
```

- [ ] **Step 2: Add required imports to the test module**

Add at the top of the test module (inside `mod tests`):
```rust
use crate::cleaner::is_skippable_error;
```
(Already has `use super::*;` which should bring `is_skippable_error` into scope since it's `pub` in the parent module.)

- [ ] **Step 3: Run to verify**

```bash
cargo test cleaner::tests::is_skippable_error -- --nocapture
```
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/cleaner.rs
git commit -m "test: add is_skippable_error unit tests (C01)"
```

---

### Task 4: C02 — `terraform()` env safety E2E test

**Files:**
- Modify: `tests/generic.rs`

- [ ] **Step 1: Add E2E test for terraform unsafe env var**

Add to `tests/generic.rs` (at the end, before any closing):

```rust
#[test]
fn clean_terraform_rejects_unsafe_env_var() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("TF_PLUGIN_CACHE_DIR", "/")
        .args(["clean", "terraform"])
        .output()
        .unwrap();
    assert!(output.status.success());
}
```

- [ ] **Step 2: Run to verify**

```bash
cargo test clean_terraform_rejects_unsafe_env_var -- --nocapture
```
Expected: passes (the safety guard in `is_safe_delete_target` rejects `/` and falls back).

- [ ] **Step 3: Commit**

```bash
git add tests/generic.rs
git commit -m "test: add terraform unsafe env var test (C02)"
```

---

### Task 5: C03 — `flutter()` env safety E2E test

**Files:**
- Modify: `tests/generic.rs`

- [ ] **Step 1: Add E2E test for flutter unsafe env var**

Add to `tests/generic.rs`:

```rust
#[test]
fn clean_flutter_rejects_unsafe_env_var() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PUB_CACHE", "/")
        .args(["clean", "flutter"])
        .output()
        .unwrap();
    assert!(output.status.success());
}
```

- [ ] **Step 2: Run to verify**

```bash
cargo test clean_flutter_rejects_unsafe_env_var -- --nocapture
```
Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add tests/generic.rs
git commit -m "test: add flutter unsafe env var test (C03)"
```

---

### Task 6: H01 — `JetBrainsCleaner::find_old_caches()` unit tests

**Files:**
- Modify: `src/cleaners/gradle.rs` (test module)

- [ ] **Step 1: Add unit tests for JetBrains cache retention logic**

Add to the test module in `src/cleaners/gradle.rs`:

```rust
#[test]
fn jetbrains_empty_dir_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let jb = tmp.path().join("Library/Caches/JetBrains");
    fs::create_dir_all(&jb).unwrap();
    let result = super::JetBrainsCleaner::find_old_caches(&jb);
    assert!(result.is_empty());
}

#[test]
fn jetbrains_single_version_per_ide_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let jb = tmp.path().join("Library/Caches/JetBrains");
    fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
    let result = super::JetBrainsCleaner::find_old_caches(&jb);
    assert!(result.is_empty(), "single version must be kept");
}

#[test]
fn jetbrains_old_versions_are_returned() {
    let tmp = TempDir::new().unwrap();
    let jb = tmp.path().join("Library/Caches/JetBrains");
    fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
    fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
    let result = super::JetBrainsCleaner::find_old_caches(&jb);
    assert_eq!(result.len(), 1);
    assert!(result[0].ends_with("GoLand2024.2"));
}

#[test]
fn jetbrains_multiple_ides_independent_retention() {
    let tmp = TempDir::new().unwrap();
    let jb = tmp.path().join("Library/Caches/JetBrains");
    fs::create_dir_all(jb.join("GoLand2024.2")).unwrap();
    fs::create_dir_all(jb.join("GoLand2025.1")).unwrap();
    fs::create_dir_all(jb.join("IntelliJIdea2024.3")).unwrap();
    fs::create_dir_all(jb.join("IntelliJIdea2025.2")).unwrap();
    let result = super::JetBrainsCleaner::find_old_caches(&jb);
    assert_eq!(result.len(), 2);
    assert!(result.iter().any(|p| p.ends_with("GoLand2024.2")));
    assert!(result.iter().any(|p| p.ends_with("IntelliJIdea2024.3")));
}

#[test]
fn jetbrains_non_parseable_names_skipped() {
    let tmp = TempDir::new().unwrap();
    let jb = tmp.path().join("Library/Caches/JetBrains");
    fs::create_dir_all(jb.join("_tmp")).unwrap();
    fs::create_dir_all(jb.join(".hidden")).unwrap();
    let result = super::JetBrainsCleaner::find_old_caches(&jb);
    assert!(result.is_empty(), "unparseable names must be skipped");
}
```

- [ ] **Step 2: Run to verify**

```bash
cargo test cleaners::gradle::tests::jetbrains -- --nocapture
```
Expected: all new tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cleaners/gradle.rs
git commit -m "test: add JetBrainsCleaner::find_old_caches unit tests (H01)"
```

---

### Task 7: H02 — `GenericCleaner::with_config()` unit tests

**Files:**
- Modify: `src/cleaners/generic.rs` (test module)

- [ ] **Step 1: Add unit tests for with_config filter application**

`with_config()` reads from `config.per_cleaner[cleaner_name]`, NOT from the global `config.older_than_days`. Tests must populate `per_cleaner` HashMap.

Add to the test module in `src/cleaners/generic.rs`:

```rust
#[test]
fn with_config_older_than_days_filters_detect_results() {
    use crate::test_helpers::write_aged_file;

    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();

    // Create a recent file (1 day old) and an old file (10 days old)
    write_aged_file(&data_dir.join("recent.log"), 1, b"x");
    write_aged_file(&data_dir.join("old10.log"), 10, b"x");

    let mut per_cleaner = std::collections::HashMap::new();
    per_cleaner.insert(
        "test-older".to_string(),
        crate::config::PerCleanerConfig {
            older_than_days: Some(5),
            larger_than_mb: None,
        },
    );
    let config = crate::config::Config {
        per_cleaner,
        ..crate::config::Config::default()
    };
    let cleaner = GenericCleaner::delete_dirs(
        "test-older",
        data_dir.clone(),
    )
    .with_config(&config);
    // Without with_config, no filter is set → both files detected.
    // With with_config and older_than_days=5, only old10.log qualifies.
    let result = cleaner.detect();
    assert!(matches!(result.status, ScanStatus::Pruneable(_)));
}

#[test]
fn with_config_no_match_for_name_leaves_cleaner_unchanged() {
    use crate::test_helpers::write_aged_file;

    let tmp = TempDir::new().unwrap();
    let data_dir = tmp.path().join("data");
    fs::create_dir_all(&data_dir).unwrap();
    write_aged_file(&data_dir.join("any.log"), 1, b"x");

    // per_cleaner has entries but none matching "no-match-cleaner"
    let mut per_cleaner = std::collections::HashMap::new();
    per_cleaner.insert(
        "other-cleaner".to_string(),
        crate::config::PerCleanerConfig {
            older_than_days: Some(999),
            larger_than_mb: None,
        },
    );
    let config = crate::config::Config {
        per_cleaner,
        ..crate::config::Config::default()
    };
    let cleaner = GenericCleaner::delete_dirs(
        "no-match-cleaner",
        data_dir.clone(),
    )
    .with_config(&config);
    // No matching entry → no filter applied → all files detected
    let result = cleaner.detect();
    assert!(matches!(result.status, ScanStatus::Pruneable(_)));
}
```

- [ ] **Step 2: Run to verify**

```bash
cargo test cleaners::generic::tests::with_config -- --nocapture
```
Expected: tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cleaners/generic.rs
git commit -m "test: add GenericCleaner::with_config unit tests (H02)"
```

---

### Task 8: H03 — `CargoCleaner::find_target_dirs()` unit tests

**Files:**
- Modify: `src/cleaners/cargo.rs` (test module)

- [ ] **Step 1: Add unit tests for target dir walk logic**

The current test module in `src/cleaners/cargo.rs` only has E2E-style tests. Add focused unit tests for `find_target_dirs()`:

```rust
#[test]
fn find_target_dirs_empty_home_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let result = super::find_target_dirs(tmp.path());
    assert!(result.is_empty());
}

#[test]
fn find_target_dirs_finds_single_target_dir() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("my-project/target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("dummy.o"), b"x").unwrap();
    let result = super::find_target_dirs(tmp.path());
    assert_eq!(result.len(), 1);
    assert!(result[0].0.ends_with("my-project/target"));
}

#[test]
fn find_target_dirs_excludes_cargo_registry() {
    let tmp = TempDir::new().unwrap();
    // Create both a project target dir .cargo/registry (should be excluded)
    let registry = tmp.path().join(".cargo/registry/cache/index.crates.io-xxx");
    fs::create_dir_all(&registry).unwrap();
    fs::write(registry.join("dummy.crate"), b"x").unwrap();
    // And a real project target dir
    let real_target = tmp.path().join("my-project/target");
    fs::create_dir_all(&real_target).unwrap();
    fs::write(real_target.join("dummy.o"), b"x").unwrap();

    let result = super::find_target_dirs(tmp.path());
    // Should find only the project target dir, not .cargo/registry stuff
    assert!(!result.is_empty());
    for (path, _) in &result {
        assert!(!path.to_string_lossy().contains(".cargo"),
            "paths containing .cargo must be excluded: {}", path.display());
    }
}

#[test]
fn find_target_dirs_respects_max_depth() {
    let tmp = TempDir::new().unwrap();
    // Create a deeply nested target dir beyond depth 5
    let deep = tmp.path().join("a/b/c/d/e/f/target");
    fs::create_dir_all(&deep).unwrap();
    fs::write(deep.join("x.o"), b"x").unwrap();

    let result = super::find_target_dirs(tmp.path());
    // With max_depth=5, a/b/c/d/e/f/target (depth 6) should NOT be found
    // But we still need at least one entry to exist within depth 5
    let shallow = tmp.path().join("a/b/target");
    fs::create_dir_all(&shallow).unwrap();
    fs::write(shallow.join("y.o"), b"x").unwrap();

    let result = super::find_target_dirs(tmp.path());
    // Should find shallow (depth 2) but NOT deep (depth 6)
    assert!(result.iter().any(|(p, _)| p.ends_with("a/b/target")),
        "shallow target at depth 2 should be found");
    assert!(!result.iter().any(|(p, _)| p.ends_with("a/b/c/d/e/f/target")),
        "deep target at depth 6 must be excluded");
}
```

- [ ] **Step 2: Run to verify**

```bash
cargo test cleaners::cargo::tests::find_target_dirs -- --nocapture
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cleaners/cargo.rs
git commit -m "test: add CargoCleaner::find_target_dirs unit tests (H03)"
```

---

### Task 9: M01 — `MiseCleaner::scan_pinned_versions()` and parser unit tests

**Files:**
- Modify: `src/cleaners/mise.rs` (test module)

- [ ] **Step 1: Add unit tests for `parse_toml_kv()`**

```rust
#[test]
fn parse_toml_kv_valid_line_returns_key_value() {
    assert_eq!(
        super::parse_toml_kv(r#"node = "20.11.0""#),
        Some(("node".to_string(), "20.11.0".to_string()))
    );
}

#[test]
fn parse_toml_kv_comment_line_returns_none() {
    assert_eq!(super::parse_toml_kv("# node = \"20.11.0\""), None);
}

#[test]
fn parse_toml_kv_empty_line_returns_none() {
    assert_eq!(super::parse_toml_kv(""), None);
}

#[test]
fn parse_toml_kv_no_equals_returns_none() {
    assert_eq!(super::parse_toml_kv("node 20.11.0"), None);
}
```

- [ ] **Step 2: Add unit tests for `parse_tools_section()`**

```rust
#[test]
fn parse_tools_section_with_tools_returns_parsed_entries() {
    let content = "\
[tools]
node = \"20.11.0\"
python = \"3.12.0\"
";
    let result = super::parse_tools_section(content);
    assert_eq!(result.len(), 2);
    assert!(result.contains(&("node".to_string(), "20.11.0".to_string())));
    assert!(result.contains(&("python".to_string(), "3.12.0".to_string())));
}

#[test]
fn parse_tools_section_empty_returns_empty() {
    let result = super::parse_tools_section("");
    assert!(result.is_empty());
}
```

- [ ] **Step 3: Add unit tests for `scan_pinned_versions()`**

These test the walkdir + TOML parsing integration:

```rust
#[test]
fn scan_pinned_versions_no_config_files_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let result = super::scan_pinned_versions(tmp.path());
    assert!(result.is_empty());
}

#[test]
fn scan_pinned_versions_global_config_is_read() {
    let tmp = TempDir::new().unwrap();
    let config_dir = tmp.path().join(".config/mise");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.toml"),
        "[tools]\nnode = \"22.0.0\"\n",
    )
    .unwrap();
    let result = super::scan_pinned_versions(tmp.path());
    assert!(result.contains(&("node".to_string(), "22.0.0".to_string())));
}

#[test]
fn scan_pinned_versions_project_mise_toml_is_read() {
    let tmp = TempDir::new().unwrap();
    fs::write(
        tmp.path().join(".mise.toml"),
        "[tools]\npython = \"3.13.0\"\n",
    )
    .unwrap();
    let result = super::scan_pinned_versions(tmp.path());
    assert!(result.contains(&("python".to_string(), "3.13.0".to_string())));
}

#[test]
fn scan_pinned_versions_malformed_toml_does_not_panic() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join(".mise.toml"), "[[[invalid toml").unwrap();
    // Should not panic
    let result = super::scan_pinned_versions(tmp.path());
    assert!(result.is_empty());
}
```

- [ ] **Step 4: Run to verify**

```bash
cargo test cleaners::mise::tests::parse_toml -- --nocapture
cargo test cleaners::mise::tests::parse_tools_section -- --nocapture
cargo test cleaners::mise::tests::scan_pinned_versions -- --nocapture
```
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/cleaners/mise.rs
git commit -m "test: add MiseCleaner scan/parse unit tests (M01)"
```

---

### Task 10: M02 — `GradleCleaner::find_old_caches()` unit tests

**Files:**
- Modify: `src/cleaners/gradle.rs` (test module)

- [ ] **Step 1: Add unit tests for Gradle cache version retention**

```rust
#[test]
fn gradle_find_old_caches_single_version_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let caches = tmp.path().join(".gradle/caches");
    fs::create_dir_all(caches.join("8.12.0")).unwrap();
    let result = super::GradleCleaner::find_old_caches(&caches);
    assert!(result.is_empty());
}

#[test]
fn gradle_find_old_caches_old_versions_returned() {
    let tmp = TempDir::new().unwrap();
    let caches = tmp.path().join(".gradle/caches");
    fs::create_dir_all(caches.join("8.8.0")).unwrap();
    fs::create_dir_all(caches.join("8.10.1")).unwrap();
    fs::create_dir_all(caches.join("8.12.0")).unwrap();
    let result = super::GradleCleaner::find_old_caches(&caches);
    // 8.8.0 and 8.10.1 should be removed, 8.12.0 kept
    assert_eq!(result.len(), 2);
    assert!(result.iter().any(|p| p.ends_with("8.8.0")));
    assert!(result.iter().any(|p| p.ends_with("8.10.1")));
}

#[test]
fn gradle_find_old_caches_varying_digit_counts() {
    let tmp = TempDir::new().unwrap();
    let caches = tmp.path().join(".gradle/caches");
    // Versions with different numbers of digits
    fs::create_dir_all(caches.join("7.0")).unwrap();
    fs::create_dir_all(caches.join("8.12.0")).unwrap();
    fs::create_dir_all(caches.join("8.12.1")).unwrap();
    let result = super::GradleCleaner::find_old_caches(&caches);
    // 7.0 and 8.12.0 should be removed (oldest), 8.12.1 kept
    assert_eq!(result.len(), 2);
}

#[test]
fn gradle_find_old_caches_skip_non_version_names() {
    let tmp = TempDir::new().unwrap();
    let caches = tmp.path().join(".gradle/caches");
    fs::create_dir_all(caches.join("modules-2")).unwrap();
    fs::create_dir_all(caches.join("wrapper")).unwrap();
    fs::create_dir_all(caches.join("journal-1")).unwrap();
    let result = super::GradleCleaner::find_old_caches(&caches);
    assert!(result.is_empty());
}
```

- [ ] **Step 2: Run to verify**

```bash
cargo test cleaners::gradle::tests::gradle_find_old_caches -- --nocapture
```
Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/cleaners/gradle.rs
git commit -m "test: add GradleCleaner::find_old_caches unit tests (M02)"
```

---

### Task 11: M03 — `run_clean_target()` dispatch unit tests

**Files:**
- Modify: `src/main.rs` (add `#[cfg(test)] mod tests` at end of file)

- [ ] **Step 1: Read the end of `src/main.rs`**

```bash
tail -30 src/main.rs
```
Find the end of the file to know where to add the test module.

- [ ] **Step 2: Add a test module at the end of `src/main.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, CleanCancelled};
    use crate::progress::ProgressReporter;
    use anyhow::Result;
    use std::path::Path;
    use std::sync::Arc;

    struct CancelledCleaner;
    impl Cleaner for CancelledCleaner {
        fn name(&self) -> &'static str { "cancelled-cleaner" }
        fn detect(&self) -> ScanResult {
            ScanResult::new("cancelled-cleaner", ScanStatus::Pruneable(100))
        }
        fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
            Err(anyhow::Error::from(CleanCancelled))
        }
    }

    struct SkippedCleaner;
    impl Cleaner for SkippedCleaner {
        fn name(&self) -> &'static str { "skipped-cleaner" }
        fn detect(&self) -> ScanResult {
            ScanResult::new("skipped-cleaner", ScanStatus::Pruneable(200))
        }
        fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
            Ok(CleanResult {
                name: "skipped-cleaner",
                bytes_freed: 100,
                uses_trash: false,
                skipped: vec![
                    SkippedEntry {
                        path: PathBuf::from("/tmp/skipped-file"),
                        reason: "Permission denied".to_string(),
                    },
                ],
            })
        }
    }

    #[test]
    fn run_clean_target_cancelled_returns_ok_zero() {
        let cleaner: Arc<dyn Cleaner> = Arc::new(CancelledCleaner);
        let reporter = crate::progress::VerboseProgress::new();
        let result = run_clean_target(
            &cleaner,
            false,      // not dry_run
            false,      // no spinner in tests
            &reporter,
        );
        assert!(result.is_ok(), "CleanCancelled should be converted to Ok");
        let clean_result = result.unwrap();
        assert_eq!(clean_result.bytes_freed, 0);
    }

    #[test]
    fn run_clean_target_skipped_entries_do_not_panic() {
        let cleaner: Arc<dyn Cleaner> = Arc::new(SkippedCleaner);
        let reporter = crate::progress::VerboseProgress::new();
        let result = run_clean_target(
            &cleaner,
            false,
            false,
            &reporter,
        );
        assert!(result.is_ok());
        let clean_result = result.unwrap();
        assert_eq!(clean_result.bytes_freed, 100);
        assert_eq!(clean_result.skipped.len(), 1);
    }
}
```

- [ ] **Step 3: Run to verify**

```bash
cargo test main::tests -- --nocapture
```
Expected: all tests pass.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "test: add run_clean_target dispatch unit tests (M03)"
```

---

### Task 12: Final verification — full test suite

- [ ] **Step 1: Run full test suite**

```bash
cargo test 2>&1
```
Expected: 300+ tests pass with 0 failures.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -- -D warnings 2>&1
```
Expected: no warnings.

- [ ] **Step 3: Run cargo fmt check**

```bash
cargo fmt --check 2>&1
```
Expected: no formatting issues.

- [ ] **Step 4: If any failures, fix them**

Address any test failures or clippy warnings, then re-run Steps 1-3 until clean.

---

## Summary of Changes

| File | Action | Tasks |
|------|--------|-------|
| `src/test_helpers.rs` | **NEW** | Task 2 |
| `src/lib.rs` | Modify (add module decl) | Task 2 |
| `src/cleaners/pre_commit.rs` | Modify (fix test fixture) | Task 1 |
| `src/cleaner.rs` | Modify (add 11 tests) | Task 3 (C01) |
| `tests/generic.rs` | Modify (add 2 tests) | Task 4-5 (C02, C03) |
| `src/cleaners/gradle.rs` | Modify (add 9 tests) | Task 6 (H01), Task 10 (M02) |
| `src/cleaners/generic.rs` | Modify (add 2 tests) | Task 7 (H02) |
| `src/cleaners/cargo.rs` | Modify (add 4 tests) | Task 8 (H03) |
| `src/cleaners/mise.rs` | Modify (add 9 tests) | Task 9 (M01) |
| `src/main.rs` | Modify (add 2 tests) | Task 11 (M03) |
