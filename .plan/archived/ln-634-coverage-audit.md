# Coverage Gaps Audit: sasurahime

**Date:** 2026-05-27
**Tool:** Cache cleaner CLI (Rust)
**Total tests:** 440+ (all passing)
**Source files:** 12 modules + 17 cleaners
**Test files:** 24 E2E files

---

## Score: 8.5/10

| Category | Coverage | Weight |
|----------|:--------:|:------:|
| File Deletion Safety | 10/10 | 30% |
| --dry-run Guarantee | 10/10 | 25% |
| Config Integrity | 9/10 | 15% |
| History/Stats Integrity | 8/10 | 15% |
| Core Journeys (scan/clean) | 7/10 | 15% |

---

## Findings

### HIGH — None

### MEDIUM — 1 found

#### [Medium] `apfs_snapshot` cleaner has no E2E test file
- **Priority:** 15 (Core Flow)
- **Location:** `tests/apfs_snapshot.rs` — does not exist
- **Current coverage:** 12 unit tests in `src/cleaners/apfs_snapshot.rs` covering all code paths (detect not-found, clean, dry-run, clean when tmutil missing)
- **Why downgraded:** Unit tests cover all branches. E2E would require `tmutil` which is macOS-only and not available in all CI environments. All 4 detect/clean paths are unit-tested. **Downgraded from HIGH to MEDIUM.**
- **Suggestion:** Add a minimal E2E that runs `sasurahime clean apfs-snapshot --dry-run` to verify the CLI entry point parses correctly.

### LOW — 4 found

#### [Low] `interactive.rs` has no unit tests (0 tests)
- **Priority:** 10 (Core Flow — TUI code)
- **Location:** `src/interactive.rs` — 0 `#[test]` annotations
- **Impact:** The TUI selection logic (selection_mapping construction, sub-target rendering, size calculation) is not unit-tested. Only E2E tests via `--yes` cover parts of the flow.
- **Why downgraded:** `dialoguer::MultiSelect` requires a TTY. The functions are tightly coupled to dialoguer I/O. Refactoring into testable pure functions would be needed first. The `--yes` path (run_auto) is E2E tested with 5+ tests.
- **Suggestion:** Extract `compute_selection_items()` and `compute_total_size()` as testable pure functions.

#### [Low] `main.rs` has 0 unit tests (command dispatch logic)
- **Priority:** 10 (Core Flow — CLI dispatch)
- **Location:** `src/main.rs` — no `#[cfg(test)]` module, 0 `#[test]`
- **Impact:** The command dispatch (`Commands` match arms) and config loading flow are not unit-tested.
- **Why downgraded:** E2E tests in `tests/interactive.rs`, `tests/config.rs`, `tests/history.rs` cover all CLI entry points (scan, clean, stats, explore, targets, --yes). The dispatch logic is a thin wrapper.
- **Suggestion:** Move config loading logic into `src/config.rs` (already done) and add a dispatch test for the `Stats` subcommand's empty-history branch.

#### [Low] History write errors are silently swallowed
- **Priority:** 10 (Data Integrity)
- **Location:** `src/history.rs:212` — `let _ = append_history(&entry, &history_dir);`
- **Impact:** If history.json can't be written (disk full, permission error), the clean operation continues silently without the user knowing history was lost.
- **Why downgraded:** This is intentional design per PBI-G spec: "History writing silently ignores filesystem errors." Adding error reporting could cause noise in cron/CI.
- **Suggestion:** Optionally print a warning on failure for interactive mode (non `--yes`).

#### [Low] `act` cleaner has no E2E tests
- **Priority:** 10 (Core Flow)
- **Location:** `tests/act.rs` exists (7 tests), but `tests/generic.rs` has 0 references to `act`
- **Current coverage:** 1 unit test (`act_path_validates_env_var_and_falls_back`). E2E tests exist in `tests/act.rs` but use a different test file.
- **Why downgraded:** `tests/act.rs` has 7 E2E tests covering all act-specific behaviors. This is adequate coverage.

---

## Coverage Summary by Module

| Module | Unit tests | E2E tests | Total | Status |
|--------|:---------:|:---------:|:----:|:------:|
| `apfs_snapshot` | 12 | 0 | 12 | ✅ Good coverage |
| `brew` | 10 | 3 | 13 | ✅ |
| `browser` | 11 | 5 | 16 | ✅ |
| `cargo` | 2 | 2 | 4 | ✅ Thin but adequate |
| `config` (src) | 23 | 11 | 34 | ✅ Excellent |
| `custom` | 7 | 0 | 7 | ✅ Unit coverage complete |
| `device_support` | 3 | 3 | 6 | ✅ |
| `explorer` | 18 | 0 | 18 | ✅ |
| `generic` | 31 | 29 | 60 | ✅ Heavily tested |
| `hint` | 16 | 0 | 16 | ✅ |
| `history` | 9 | 6 | 15 | ✅ |
| `interactive` | **0** | 17 | 17 | ⚠️ See findings |
| `log` | 12 | 6 | 18 | ✅ |
| `progress` | 19 | 9 | 28 | ✅ |
| `scanner` | 0 | 6 | 6 | ✅ Covered by E2E |
| `trash` | 7 | 10 | 17 | ✅ |
| `xcode` | 14 | 5 | 19 | ✅ |

**Key strength areas:**
- Every cleaner has `detect()` + `clean()` + `dry_run` tests
- Config module has 23 unit + 11 E2E = 34 tests (comprehensive)
- History module has 9 unit + 6 E2E = 15 tests
- GenericCleaner has 60 tests covering all factory methods
- Cross-cutting concerns (dry-run, trash, progress) all well-tested

---

## Verdict

**Score: 8.5/10** — Strong coverage with no critical gaps. All cleaners have dry-run safety tests, config has exhaustive coverage, history has atomic write + corruption handling tests. The remaining gaps (interactive.rs unit tests, main.rs dispatch tests) are architectural limitations common to CLI tools with TUI code.
