# Coverage Gaps Design

**Date:** 2026-05-28
**Branch:** `fix-2026-05-28`
**Status:** Design approved, ready for implementation planning

## Scope

Address all 9 coverage gaps from the audit report plus test infrastructure improvements:

1. **9 coverage gaps** (3 Critical + 3 High + 3 Medium)
2. **Pre-commit failing test** investigation and fix
3. **Test helper refactoring** — DRY common mock runners/fixtures across test modules

No production code behavior changes. Only test additions, test fixes, and test infrastructure extraction.

---

## Phase 1: Pre-commit Failing Test Fix

**File:** `src/cleaners/pre_commit.rs` line 162-172

**Symptom:** `detect_returns_pruneable_when_cache_exists` panics because `result.status` is not `Pruneable(_)`.

**Likely cause:** `dir_size()` returns 0 for the created fixture. The test creates a file via `fs::write` with content `b"dummy"` which should be non-zero — but `dir_size()` on a directory wrapper or symlink target may behave differently than expected.

**Action:**
1. Debug: add `eprintln!("dir_size: {}", bytes)` or check `dir_size()` output for the cache fixture.
2. If `dir_size()` returns 0: adjust the test fixture (more bytes, deeper structure, or different path).
3. If `dir_size()` works correctly: investigate `detect()` logic — could `cache_dir()` return a different path than expected?
4. Fix the test to reliably produce a `Pruneable` result.

**Exit criteria:** `cargo test cleaners::pre_commit::tests` passes with zero failures.

---

## Phase 2: Test Helper Extraction (`src/test_helpers.rs`)

### Problem

Multiple test modules define identical mock runners and fixture utilities:

| Pattern | Locations |
|---------|-----------|
| `NoopRunner` / `NoToolRunner` (tool not found) | `uv.rs`, `pre_commit.rs`, `brew.rs`, `mise.rs`, `browser.rs`, `log.rs`, `xcode.rs`, generic.rs module tests |
| `CliToolRunner` (tool succeeds) | `pre_commit.rs` |
| Fake exit status | `apfs_snapshot.rs`, `ios_backup.rs`, `mise.rs` |
| `write_aged()` / `write_aged_dir()` | `log.rs`, `generic.rs` |
| `VerboseFlagGuard` (`TEST_LOCK` + `set_verbose`) | `device_support.rs`, `apfs_snapshot.rs`, `xcode.rs`, `generic.rs` |

### Design

**File:** `src/test_helpers.rs` (guarded by `#[cfg(test)]`)

**Exposed API:**

```rust
// ── Mock CommandRunner ──
pub struct MockRunner { ... }
impl MockRunner {
    pub fn new() -> Self;
    /// Tool reports as not found (exists() returns false)
    pub fn with_not_found(self) -> Self;
    /// Tool exists and returns the given output
    pub fn with_output(self, program: &str, output: Output) -> Self;
    /// Tool exists and succeeds with empty output
    pub fn with_success(self, program: &str) -> Self;
    /// Tool exists and fails with the given exit code
    pub fn with_exit_code(self, program: &str, code: i32) -> Self;
}
impl CommandRunner for MockRunner { ... }

// ── Fixture utilities ──
/// Write a file with a specific mtime in days ago
pub fn write_aged_file(path: &Path, days_old: u64, content: &[u8]);

/// Create a directory and set its mtime (note: dir mtime is platform-dependent)
pub fn write_aged_dir(path: &Path, days_old: u64);

// ── Verbose flag guard ──
/// Set verbose mode for the duration of a test
pub struct VerboseGuard;
impl VerboseGuard {
    pub fn new() -> Self; // calls set_verbose(true), restores on drop
}

// ── Common exit status factory ──
pub fn exit_ok() -> ExitStatus;
pub fn ok_output(stdout: &[u8]) -> Output;
```

### Migration Strategy

- Only rewrite test modules that are being modified in Phase 3.
- Existing passing tests (e.g., `is_safe_delete_target` tests in `generic.rs`, `version_key` tests in `browser.rs`) are **not touched**.
- The `tests/*.rs` E2E tests (using `assert_cmd::Command`) are **not affected** — they don't use these helpers.

---

## Phase 3: Gap Tests

### C01: `is_skippable_error()` Unit Tests

**File:** `src/cleaner.rs` (existing test module)

**Test cases (8+):**

| # | Input | Expected |
|---|-------|----------|
| 1 | `io::Error::from(ErrorKind::PermissionDenied)` | `true` (skippable) |
| 2 | `io::Error::from(ErrorKind::WouldBlock)` | `true` |
| 3 | `io::Error::from(ErrorKind::AlreadyExists)` | `true` |
| 4 | `io::Error::from(ErrorKind::Interrupted)` | `true` |
| 5 | `io::Error::from(ErrorKind::NotFound)` | `false` |
| 6 | `io::Error::from(ErrorKind::ConnectionRefused)` | `false` |
| 7 | `anyhow!("Permission denied")` | `true` (string pattern match) |
| 8 | `anyhow!("some other error")` | `false` |
| 9 | `CleanCancelled` wrapped in anyhow | `false` (must propagate) |

### C02: `terraform()` Env Safety E2E Test

**File:** `tests/generic.rs` (or dedicated `tests/generic_env.rs`)

Add test similar to `clean_act_rejects_unsafe_env_var_path`:
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

### C03: `flutter()` Env Safety E2E Test

**File:** `tests/generic.rs`

Same pattern as C02:
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

### H01: `JetBrainsCleaner::find_old_caches()` Unit Tests

**File:** `src/cleaners/gradle.rs` (test module)

**Test cases:**
1. Empty dir → empty result
2. Single version per IDE → empty result (nothing to delete)
3. Multiple versions of same IDE → old versions deleted, newest kept
4. Multiple IDEs interleaved → each IDE independently retains its max
5. Non-parseable names → skipped
6. Symlink entries → skipped (WalkDir default)

### H02: `GenericCleaner::with_config()` Unit Tests

**File:** `src/cleaners/generic.rs` (test module)

Test the filter application logic directly:
1. Create a `GenericCleaner` with `OlderThanDays::new(7)`, apply config with `older_than_days = 3`, verify the filter is overridden by config.
2. Create a `GenericCleaner` without filters, apply config with `larger_than_mb = 10`, verify `detect()` filters by size.
3. Create a command-based `GenericCleaner`, apply config with filters, verify warning is printed (not crash).

### H03: `CargoCleaner::find_target_dirs()` Unit Tests

**File:** `src/cleaners/cargo.rs` (test module)

**Test cases:**
1. No target dirs → empty result
2. Single target dir at depth 1 → found
3. Multiple target dirs at various depths → all found
4. `.cargo/registry` → NOT found (excluded)
5. Target dir beyond max depth → NOT found
6. Symlink to target dir → NOT found

### M01: `MiseCleaner::scan_pinned_versions()` Unit Tests

**File:** `src/cleaners/mise.rs` (test module)

**Test cases:**
1. No config files → empty set
2. Global config with pinned version → one entry
3. Project `.mise.toml` with pinned version → one entry
4. Both global + project with different versions → two entries
5. `.mise.toml` beyond max depth 5 → not read
6. Malformed TOML → graceful skip (no panic)
7. `parse_toml_kv()`: valid line → parsed key=value
8. `parse_toml_kv()`: comment line → skip
9. `parse_toml_kv()`: empty line → skip
10. `parse_tools_section()`: standard tools block → parsed

### M02: `GradleCleaner::find_old_caches()` Unit Tests

**File:** `src/cleaners/gradle.rs` (test module)

**Test cases:**
1. Single version → empty result
2. Multiple versions → old ones returned
3. Multiple versions with differing digit counts (e.g., `8.10.1` vs `8.8.0`) → correct comparison
4. Zero-digit names → skipped
5. Unparseable names → skipped

### M03: `run_clean_target()` Dispatch Unit Tests

**File:** `src/main.rs` (test module)

This function is in `main.rs` which currently has no `#[cfg(test)]` module. Add one.

**Test cases:**
1. Cleaner returns `CleanCancelled` → function returns `Ok` with zero bytes_freed (error converted to graceful skip)
2. Cleaner returns skipped entries → verify they appear in output (harder, but verify function does not panic)

Note: These tests require a mock `Cleaner` implementation. Use a simple struct defined inline in the test module.

---

## Files Changed

| File | Change Type | Phase |
|------|-------------|-------|
| `src/test_helpers.rs` | **NEW** | Phase 2 |
| `src/cleaner.rs` | Add tests (C01) | Phase 3 |
| `src/cleaners/generic.rs` | Add tests (H02), minor | Phase 2+3 |
| `src/cleaners/gradle.rs` | Add tests (H01, M02) | Phase 3 |
| `src/cleaners/cargo.rs` | Add tests (H03) | Phase 3 |
| `src/cleaners/mise.rs` | Add tests (M01) | Phase 3 |
| `src/main.rs` | Add tests (M03) | Phase 3 |
| `src/cleaners/pre_commit.rs` | Fix test | Phase 1 |
| `tests/generic.rs` | Add tests (C02, C03) | Phase 3 |

---

## Exit Criteria

- `cargo test --bin sasurahime` passes with 0 failures
- `cargo clippy -- -D warnings` passes
- Each gap from the audit report has corresponding test coverage
- No regressions in existing passing tests
- `src/test_helpers.rs` is the single source for shared mock runners and fixture utilities
