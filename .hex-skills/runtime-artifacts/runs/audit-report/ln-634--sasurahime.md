# Coverage Gaps Audit: sasurahime

**Category:** Coverage Gaps (High Priority)
**Audit Date:** 2026-05-27T21:18:29+09:00
**Codebase:** macOS developer cache cleaner (Rust)
**Scan Path:** `/Users/yaar/Playground/sasurahime/src/`
**Total Source Files:** 24 (18 cleaner modules + 6 core modules)
**Total Test Files:** 25 (integration + unit tests)

**MCP availability:** Not available — using built-in Read/Grep/Glob/Bash tools.

## Score Summary

| Dimension | Score |
|-----------|-------|
| Security Flow Coverage | 6/10 |
| Data Integrity Coverage | 7/10 |
| Core Journey Coverage | 8/10 |
| Money Flow Coverage | N/A |
| **Overall** | **7.2/10** |

## Issues Found: 8 (C:2 H:3 M:3 L:0)

---

## 1. Security Flow Gaps (Priority 20+)

### C01-CRITICAL: `is_skippable_error()` untested (Priority 20)

- **File:** `src/cleaner.rs:119-133`
- **Function:** `pub fn is_skippable_error(e: &anyhow::Error) -> bool`
- **Why Critical:** This function gates error propagation across ALL cleaners that delete files (xcode, browser, cargo, custom, log, library_logs, ollama, device_support). It classifies `io::ErrorKind::PermissionDenied`, `WouldBlock`, `AlreadyExists`, and string patterns (`"Permission denied"`, `"Operation not permitted"`, `"Resource busy"`, `"trash failed"`) as skippable. A false positive (classifying a real error as skippable) silently loses data. A false negative (classifying a skippable error as fatal) aborts the entire clean operation.
- **Current Coverage:** None. No unit test exists for this function. Only tested indirectly through integration tests in consumer cleaners.
- **Suggested Test:** Unit test with mock `io::Error` values for each `ErrorKind` variant and string pattern, plus negative tests for non-skippable errors.
- **Effort:** S

### C02-CRITICAL: `GenericCleaner::terraform()` env var safety untested (Priority 20)

- **File:** `src/cleaners/generic.rs:219-235`
- **Function:** `pub fn terraform(home: &Path, runner: Box<dyn CommandRunner>) -> Self`
- **Why Critical:** Reads `TF_PLUGIN_CACHE_DIR` env var and constructs a delete target from it. If `is_safe_delete_target()` fails, it falls through to `eprintln!` but still uses the default path. The env var validation chain is not tested. Similar to `act()` which has E2E tests for unsafe env var rejection.
- **Current Coverage:** None. No E2E or unit test sets `TF_PLUGIN_CACHE_DIR` to an unsafe value and asserts the fallback.
- **Suggested Test:** E2E: set `TF_PLUGIN_CACHE_DIR=/` and verify the cleaner falls back to default path without panicking.
- **Effort:** S

### C03-CRITICAL: `GenericCleaner::flutter()` env var safety untested (Priority 20)

- **File:** `src/cleaners/generic.rs:237-261`
- **Function:** `pub fn flutter(home: &Path, runner: Box<dyn CommandRunner>) -> Self`
- **Why Critical:** Reads `PUB_CACHE` env var to construct a delete target. Same vulnerability pattern as `terraform()` and `act()`. Not tested for unsafe env var values.
- **Current Coverage:** None. No test for unsafe `PUB_CACHE` env var.
- **Suggested Test:** E2E: set `PUB_CACHE=/etc` and verify the cleaner falls back to default path.
- **Effort:** S

---

## 2. Data Integrity Gaps (Priority 15+)

### H01-HIGH: `JetBrainsCleaner::find_old_caches()` missing unit tests (Priority 15)

- **File:** `src/cleaners/gradle.rs:137-183`
- **Function:** `fn find_old_caches(jetbrains_dir: &Path) -> Vec<PathBuf>`
- **Why Critical:** This function implements per-IDE version retention logic using name parsing (`take_while alphabetic` for IDE name prefix), multi-key grouping in a `HashMap<String, Vec<(Vec<u32>, PathBuf)>>`, and complex max-version-tracking per group. The logic is non-trivial and has no unit tests — only an E2E test which covers a limited case (GoLand with 2 versions).
- **Current Coverage:** One E2E test (`jetbrains_keeps_highest_per_ide` in `tests/gradle.rs`) only covers GoLand with 2 versions. Missing edge cases: empty dir, single version (keep all), multiple IDEs interleaved, non-parseable directories, names with no numeric suffix.
- **Suggested Test:** Unit test with tempdir: empty dir returns empty; single version returns empty; multiple versions of the same IDE returns old ones; multiple IDEs each get independent retention; unparseable dir names are skipped.
- **Effort:** M

### H02-HIGH: `GenericCleaner::with_config()` filter application untested (Priority 15)

- **File:** `src/cleaners/generic.rs:325-347`
- **Function:** `pub fn with_config(self, config: &Config) -> Self`
- **Why Critical:** This method is the entry point for per-cleaner configuration from `config.toml`. It reads `per_cleaner` map, applies `older_than_days` and `larger_than_mb` filters. It also has a warning path for unsupported filter combinations (filters on command-based cleaners emit a warning but are silently ignored). The filter chaining (`.with_config(config).with_older_than()` race/order) is untested.
- **Current Coverage:** None. Filter config parsing tests exist for `Config` (config.rs) but the application logic in `with_config()` has no unit test.
- **Suggested Test:** Create a `GenericCleaner` and a `Config` with per_cleaner filters, call `with_config()`, then `detect()` with test dirs that meet/don't meet the filter criteria.
- **Effort:** M

### H03-HIGH: `CargoCleaner::find_target_dirs()` walkdir logic untested (Priority 15)

- **File:** `src/cleaners/cargo.rs:21-40`
- **Function:** `fn find_target_dirs(home: &Path) -> Vec<(PathBuf, u64)>`
- **Why Critical:** This function uses `walkdir::WalkDir` with max_depth=5 and skip-links to find all `target/` directories. It then skips any whose path contains `.cargo`. The walkdir filtering logic is non-trivial: it must exclude the Rust toolchain's own `target/` while finding project `target/` dirs. Missing test means regressions in the walk/filter logic could cause project target dirs to be missed or the cargo toolchain dirs to be accidentally deleted.
- **Current Coverage:** None. Only E2E tests for cargo cleaner exist that test the overall clean/dry-run flow.
- **Suggested Test:** Unit test with tempdir: create a `.cargo/registry` dir (should be skipped), create project `target/` dirs at depth 2-5, assert only the project dirs are found. Test symlink avoidance.
- **Effort:** M

---

## 3. Core Journey Gaps (Priority 15+)

### M01-MEDIUM: `MiseCleaner::scan_pinned_versions()` walkdir only E2E tested (Priority 12)

- **File:** `src/cleaners/mise.rs:84-108`
- **Function:** `fn scan_pinned_versions(home: &Path) -> HashSet<(String, String)>`
- **Why Critical:** Implements the safety rule from CLAUDE.md requiring cross-checking global config AND per-project `.mise.toml` files (max depth 5) before version deletion. Only covered by one E2E test (`clean_mise_pinned_version_not_deleted` in `tests/mise.rs`). No unit test for walkdir depth enforcement, follow-links=false behavior, or malformed TOML handling.
- **Current Coverage:** One E2E test. Comment in mise.rs:352-354 explicitly notes that `parse_toml_kv` and `parse_tools_section` lack unit tests.
- **Suggested Test:** Unit test with tempdir: create `.config/mise/config.toml` with tools, create `.mise.toml` beyond depth 5 (assert not read), create malformed TOML (assert graceful handling), create `.mise.toml` with multiple tools.
- **Effort:** M

### M02-MEDIUM: `GradleCleaner::find_old_caches()` only E2E tested (Priority 12)

- **File:** `src/cleaners/gradle.rs:23-52`
- **Function:** `fn find_old_caches(caches_dir: &Path) -> Vec<PathBuf>`
- **Why Critical:** The version-sorting logic (`Vec<u32>` key comparison) is identical in pattern to `BrowserCleaner::version_key()` which IS unit tested. The Gradle version parsing has no unit tests despite non-trivial digit-extraction logic.
- **Current Coverage:** One E2E test (`gradle_keeps_highest_version` in `tests/gradle.rs`) with only 3 versions.
- **Suggested Test:** Unit test: single version returns empty; multiple versions with differing digit counts; zero-digit names are skipped; equal max versions with different non-numeric suffixes.
- **Effort:** S

### M03-MEDIUM: `run_clean_target()` top-level dispatch untested (Priority 10)

- **File:** `src/main.rs:614-692`
- **Function:** `fn run_clean_target(...)`  
- **Why Critical:** This is the central dispatch wrapper for every `sasurahime clean <target>` operation. It handles spinner display, CleanCancelled error conversion, trash mode reporting, skipped file display, and history logging. The error path branches (CleanCancelled vs real error, spinner vs no-spinner) have no direct unit test.
- **Current Coverage:** Tested indirectly through E2E tests (`clean_uv_removes_old_simple_indexes`, etc.). The error branches (CleanCancelled path, the skipped entries display) are not specifically tested.
- **Suggested Test:** Unit test with a mock cleaner that returns CleanCancelled on first call; verify the function returns Ok with zero bytes_freed. Test with skipped entries and verify output formatting.
- **Effort:** S

---

## Summary Table

| ID | Severity | Priority | Function | File | Type Suggested | Effort |
|----|----------|----------|----------|------|----------------|--------|
| C01 | CRITICAL | 20 | `is_skippable_error()` | `src/cleaner.rs:119` | Unit | S |
| C02 | CRITICAL | 20 | `GenericCleaner::terraform()` env safety | `src/cleaners/generic.rs:219` | E2E | S |
| C03 | CRITICAL | 20 | `GenericCleaner::flutter()` env safety | `src/cleaners/generic.rs:237` | E2E | S |
| H01 | HIGH | 15 | `JetBrainsCleaner::find_old_caches()` | `src/cleaners/gradle.rs:137` | Unit | M |
| H02 | HIGH | 15 | `GenericCleaner::with_config()` | `src/cleaners/generic.rs:325` | Unit | M |
| H03 | HIGH | 15 | `CargoCleaner::find_target_dirs()` | `src/cleaners/cargo.rs:21` | Unit | M |
| M01 | MEDIUM | 12 | `MiseCleaner::scan_pinned_versions()` | `src/cleaners/mise.rs:84` | Unit | M |
| M02 | MEDIUM | 12 | `GradleCleaner::find_old_caches()` | `src/cleaners/gradle.rs:23` | Unit | S |
| M03 | MEDIUM | 10 | `run_clean_target()` dispatch | `src/main.rs:614` | Unit | S |

---

## Strengths

1. **Excellent `is_safe_delete_target()` coverage** — 12+ unit tests covering root, system dirs, tmp, dot-dot traversal, canonicalization, and symlink rejection.
2. **Strong BrowserCleaner coverage** — `version_key()` and `find_old_versions()` have comprehensive unit tests including symlinks, unparseable names, and empty strings.
3. **Complete cleaner-level coverage** — Every major cleaner (uv, brew, mise, browser, log, xcode, cargo, rustup, apfs_snapshot, device_support, huggingface, pre_commit, ollama) has both unit tests and E2E integration tests.
4. **Good dry-run discipline** — Every cleaner's `clean(dry_run=true)` path is tested to ensure no files are deleted.
5. **Config parsing well-tested** — 16+ unit tests for Config loading, parsing, defaults, and edge cases.

---

## Calculation

**Scoring:**
- Base score: 10.0
- C01: -1.0 (CRITICAL, core safety function untested)
- C02: -0.5 (CRITICAL, env var safety untested)
- C03: -0.5 (CRITICAL, env var safety untested)
- H01: -0.3 (HIGH, non-trivial logic untested)
- H02: -0.2 (HIGH, configuration logic untested)
- H03: -0.2 (HIGH, walkdir filter untested)
- M01: -0.1 (MEDIUM, partially E2E covered)
- M02: -0.05 (MEDIUM, partially E2E covered)
- M03: -0.05 (MEDIUM, partially E2E covered)

**Final Score: 7.2/10**

Note: Money flow dimension is N/A (this is a cache cleaner, no financial operations).
