# Changelog

All notable changes to sasurahime will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.28] — 2026-05-28

### Added

- **Test coverage gaps — 6 new unit test suites.** Added targeted unit tests
  for `is_skippable_error()` (C01), `GradleCleaner`/`JetBrainsCleaner`
  `find_old_caches` (H01, M02), `CargoCleaner::find_target_dirs` (H03),
  `MiseCleaner` parse/scan (M01), and `run_clean_target` dispatch (M03).
- **`--no-unicode` CLI flag.** Disables unicode and spinner characters in
  terminal output for better compatibility with log captures and non-unicode
  terminal emulators.
- **History retention cap (max 1000 entries).** `history.json` now trims to
  the 1000 most recent entries on write, preventing unbounded file growth.
  Configurable via `[history].max_entries` in `config.toml`.

### Fixed

- **`MockRunner.NotFound` order-dependency bug.** Changed from a method-based
  mock to a `bool` fallback field, fixing a test ordering issue where
  subsequent tests could see stale `NotFound` state.
- **Pre-commit test env var pollution.** Tests modifying `PRE_COMMIT_HOME` or
  `XDG_CACHE_HOME` now properly isolate via the shared `EnvGuard` guard,
  eliminating parallel-test flakiness in the `pre_commit` test suite.

### Internal

- **488 tests total** (329 unit + 159 integration/E2E across 25 test files),
  0 failures — 46 new tests (+10.4%) since v0.1.27.
- **`src/test_helpers.rs`** introduced as a shared test infrastructure module,
  housing the `EnvGuard` Drop-based environment variable guard extracted from
  `pre_commit.rs`.
- **Archived 5 completed plan documents:** PBI A-G docs (6 files), colima
  cleaner plan, command-timeout-hint plan, and coverage-gaps test spec + plan.
- **PBI process documentation updated.** `pbi/AGENTS.md` now includes a
  detailed AI agent quick-reference guide; `pbi/PBI-process.md` streamlined
  with explicit TDD and mock guidelines.

---

## [0.1.27] — 2026-05-27

### Added

- **`is_safe_delete_target()` path validation.** New public function in
  `src/cleaners/generic.rs` that blocks deletion of system-critical paths
  (`/`, `/etc`, `/System`, `/var/log`, etc.) while allowing macOS tempdirs
  (`/var/folders/...`) and user cache directories. Used by `CustomPathCleaner`
  and the `terraform()`/`flutter()` factory methods.
- **Downloads default 30-day age filter.** `sasurahime clean downloads` now
  skips files modified within the last 30 days by default. Override via
  config `[cleaner.downloads].older_than_days`.
- **terraform/flutter env var safety validation.** `TF_PLUGIN_CACHE_DIR` and
  `PUB_CACHE` environment variables are validated against
  `is_safe_delete_target()`. If they point to an unsafe path (e.g. `/etc`),
  the cleaner falls back to the default directory with a warning.
- **history.json schema versioning.** `HistoryEntry` gains a `version: 1`
  field with `#[serde(default)]` for forward compatibility.
- **`build_selection_items()` + `compute_selected_total()` extracted** from
  `interactive.rs` as pure functions, enabling unit testing of TUI selection
  logic without a TTY. 6 new unit tests covering sub-target rendering,
  mixed cleaners, and selected-size computation.
- **`tests/apfs_snapshot.rs`** — new E2E test file for the APFS snapshot
  cleaner CLI entry point (dry-run and tool-not-found paths).
- **`--config <path>` documentation** added to `docs/HOWTO-USE.md` (EN + JA).

### Security

- **System path deletion protection (Checking Team: Red Team).** `terraform()`
  and `flutter()` constructors no longer blindly trust `TF_PLUGIN_CACHE_DIR`
  and `PUB_CACHE` environment variables. Unreasonable paths are rejected with
  a warning before the cleaner builds its directory list.

### Internal

- **Checking Team multi-perspective review.** 22 agents across 3 waves
  (core + specialists + test) reviewed the `pbi-2026-05-25` branch.
  Score: 82.4/100 (Rank A). Report in `plans/2026-05-27-0637-review-pbi-2026-05-25.md`.
- **Coverage gap audit.** Systematic analysis of all 24 test files.
  Score: 8.5/10. No HIGH gaps found. Report in `plans/ln-634-coverage-audit.md`.
- **442 tests total** (288 unit + 154 integration/E2E across 24 test files),
  0 failures.
- **All PBI A–G completed** (22 SP total). Sprint 5 ships PBI-D Phase 2,
  PBI-E (config.toml), PBI-F (--yes), and PBI-G (stats).
- **Docs DoD finalized.** All PBI-E/G documentation items marked complete
  (exclude, custom, per-cleaner, stats, --config documented in EN + JA).

### Fixed

- **`--permanent` help text was self-contradictory.** Flag named `--permanent`
  but claimed to "Move deleted files to Trash". Clarified to accurately describe
  the behavior: "Permanently delete files instead of moving to Trash. By default,
  files are moved to macOS Trash for safety."
- **Rust 1.95.0 clippy `items_after_test_module`.** Moved `#[cfg(test)] mod tests`
  in `src/cleaner.rs` to the end of the file, after the `pub trait Cleaner`
  definition.
- **Rust 1.95.0 clippy `needless_borrows_for_generic_args`.** Removed unnecessary
  `&` from `&log_dir.join(...)` in `tests/config.rs`.

---

## [0.1.26] — 2026-05-26

### Added

- **Xcode subcategory TUI expansion (PBI-D Phase 2).** `XcodeCleaner` now
  overrides `sub_targets()` to expose DerivedData and Archives as expandable
  subcategories in the interactive TUI. The interactive.rs generic subcategory
  logic automatically renders indented checkboxes for each subcategory and
  dispatches `sasurahime clean xcode --sub <name>` for selected items. Non-
  existent subcategories are filtered out (not shown).
- **Config file `exclude` field (PBI-E).** `~/.config/sasurahime/config.toml`
  now supports `exclude = ["cleaner-name"]` to exclude cleaners from scan and
  TUI listing. Direct `sasurahime clean <target>` bypasses the filter.
- **`--config <path>` CLI flag (PBI-E).** Override the config file path.
  Falls back to defaults with a warning if the file doesn't exist.
- **`[[custom]]` user-defined cache targets (PBI-E).** Add arbitrary
  directories as scan/clean targets via `[[custom]]` sections in config.toml.
  `CustomPathCleaner` deletes sub-contents (not root) with macOS `uchg` flag
  handling, trash support, and progress reporting.
- **Per-cleaner config filters (PBI-E).** `[cleaner.<name>]` sections in
  config.toml support `older_than_days` and `larger_than_mb` filters. These
  apply to DeleteDirs-based cleaners (act, colima, downloads, etc.) and
  `[cleaner.logs]` (as `keep_days`). Command-based cleaners show a runtime
  warning that the filter is not supported.
- **`sasurahime stats` command (PBI-G).** New subcommand showing aggregated
  deletion history: total freed bytes, run count, and a table of recent
  cleanups. Supports `--last N` to limit output to the most recent N entries.
- **Automatic history logging (PBI-G).** Every successful clean operation
  (`bytes_freed > 0`, not `--dry-run`) appends a record to
  `~/.local/share/sasurahime/history.json` with timestamp, cleaner name,
  freed bytes, and skipped count. History writing is atomic (temp file +
  rename) and silently ignores filesystem errors.
- **`cleaner::Cleaner::sub_targets()` trait method.** Default returns empty
  vec; any cleaner can override to expose sub-targets for TUI expansion.

### Internal

- **Comprehensive test suite:** 436 tests (270 unit + 145 integration/E2E),
  22 test files, 0 failures.
- **`CustomPathCleaner`** in `src/cleaners/custom.rs` with `Cleaner` trait
  implementation, chflags nouchg handling, dry-run support, and 6 unit tests.
- **`src/history.rs`** (362 lines) with `HistoryEntry`, `StatsSummary`,
  atomic `append_history()`, `load_history()` (corruption-tolerant),
  `compute_stats()`, `format_stats()`, and 9 unit tests.
- **`GenericCleaner` builder methods:** `with_older_than()`,
  `with_larger_than()`, `with_config()` for per-cleaner filter configuration.
- **`Config::effective_logs_keep_days()`** resolves per-cleaner
  `[cleaner.logs] older_than_days` over the global `logs.keep_days`.
- **`Config::default_history_dir()`** for centralized path resolution.
- **SCM:** `.gitignore` updated for `.worktrees/` directory.
- **3 new E2E tests** for PBI-D Phase 2: `sub_targets_integration`,
  `sub_targets_returns_only_existing`, `sub_targets_filters_zero_size_entries`.
- **11 E2E tests** for PBI-E: exclude (3), --config (1), [[custom]] (2),
  per-cleaner age/size filters (3), logs age filter (1), scan display (1).
- **6 E2E tests** for PBI-G: stats aggregations, --last N, dry-run guard,
  corrupted file handling, empty state, clean auto-logging.

---

## [0.1.25] — 2026-05-25

### Added

- **Xcode subcategory selection — CLI core (PBI-D Phase 1).** `XcodeCleaner` now
  supports partial cleanup of Xcode caches via the `--sub` flag, letting users
  target only DerivedData, Archives, or both instead of cleaning everything.
- **`XcodeSubcategory` enum** with `DerivedData` and `Archives` variants, plus
  `from_str()` parser accepting `derived-data`, `deriveddata`, and `archives`.
- **`--sub derived-data|archives` CLI flag** on `sasurahime clean xcode`.
  Accepts comma-separated values (`--sub derived-data,archives`). When omitted,
  defaults to `DerivedData` only (backwards-compatible).
- **`Cleaner::sub_targets()` trait method.** Default returns empty vec; any
  cleaner can override to expose sub-targets for TUI expansion. The
  `interactive.rs` TUI already has generic logic to render sub-targets as
  indented checkboxes and dispatch `sasurahime clean <name> --sub <sub_name>`
  for each selected sub-target.
- **`XcodeCleaner::detect_subcategories()`** returns `Vec<SubcategoryInfo>`
  with per-subcategory path and size, used by the CLI and available for the
  future `sub_targets()` override.
- **`XcodeCleaner::with_subcategories()` builder** — constructs an
  `XcodeCleaner` scoped to specific subcategories for targeted cleaning.
- **`XcodeCleaner::is_xcode_running()`** — checks via `pgrep -x Xcode` for
  safe DerivedData deletion with a confirmation prompt when Xcode is active.
- **PBI-D document updated** to reflect current implementation status (Phase 1
  complete, Phase 2 TUI pending) and scope decision (Simulators handled by the
  standalone `simulator` cleaner, not an Xcode subcategory).

### Internal

- **5 E2E tests** in `tests/xcode.rs`: clean with `--sub derived-data`,
  `--sub archives`, default (no sub), `--dry-run`, and missing-data handling.
- **10 unit/integration tests** in `src/cleaners/xcode.rs`: `subcategory_all`,
  `subcategory_path`, `from_str` variants, `display_name`,
  `detect_subcategories`, `clean_selected_subcategory_only`,
  `is_xcode_running`, detection status.
- **`SubcategoryInfo` struct** for returning per-subcategory metadata from
  `detect_subcategories()`.

---

## [0.1.24] — 2026-05-25

### Added

- **Trash warning UI (PBI-C).** After cleaning files via macOS Trash, `sasurahime`
  now shows `"Note: Moved X to Trash. Run 'Empty Trash' to reclaim disk space."`
  to inform users that Trash must be emptied to actually free disk space.
- **Large file pre-warning.** When ≥1 GB of files will be moved to Trash, a
  prominent `"Note: Files will be moved to Trash (not immediately freed)."`
  warning is shown before the clean operation begins.
- **`CleanResult.uses_trash` field.** Each cleaner reports whether it used the
  macOS Trash (`delete_path`) or permanent deletion. CLI-based cleaners (brew,
  uv, rustup, etc.) set `false`; directory-deletion cleaners set `true`.
- **`format_trash_warning()` / `format_large_trash_warning()` helpers.** Pure
  functions in `src/cleaner.rs` with 7 unit tests covering threshold boundaries,
  dry-run suppression, and trash/non-trash modes.
- **`LARGE_TRASH_THRESHOLD_BYTES`** constant (1 GiB) for threshold logic.

### Changed

- **`run_clean_target` pre/post hooks:** Added trash notice print before clean
  (when `is_trash_mode() && !dry_run`) and size-specific note after clean
  (when `is_trash_mode() && bytes_freed > 0`).

### Internal

- **All CleanResult construction sites** updated across 16+ source files to
  provide the new `uses_trash` field.
- **3 E2E tests** in `tests/trash.rs`: trash note visible, suppressed with
  `--permanent`, suppressed with `--dry-run`.

---

## [0.1.23] — 2026-05-25

### Added

- **Robust error handling (PBI-B).** All 20+ cleaners now catch permission errors
  (`EPERM`), file-lock errors (`EBUSY`), and access errors (`EACCES`) during
  deletion instead of panicking or aborting the entire cleanup. Affected files
  are recorded as `skipped` and processing continues with remaining targets.

- **`SkippedEntry` struct + `CleanResult.skipped` field.** Each cleaner's
  `clean()` method now returns a `Vec<SkippedEntry>` listing files/directories
  that could not be deleted due to permission or lock issues, along with the
  error reason.

- **Exit code semantics.** `sasurahime clean <target>` now exits with code 1
  when ALL files failed (nothing freed, only errors), and code 0 when at least
  some data was freed or nothing was skippable. Partial success (some freed,
  some skipped) exits 0.

- **Skip summary display.** After each clean operation, a summary of skipped
  files is printed to stderr: `N file(s) skipped: /path: Permission denied`.

- **`is_skippable_error()` helper.** New public function in `src/cleaner.rs`
  that checks if an `anyhow::Error` wraps a skippable IO error
  (`PermissionDenied`, `WouldBlock`, `AlreadyExists`), with fallback message
  matching for `trash` layer errors.

### Changed

- **All deletion paths updated.** Every `crate::trash::delete_path()` call,
  `fs::remove_dir_all()` call, and `rm -rf` delegate across all cleaners
  (browser, xcode, cargo, log, uv, mise, ios_backup, library_logs,
  device_support, ollama, rustup, gradle + JetBrains, GenericCleaner)
  now handles skippable errors gracefully via the new pattern.

### Internal

- **`src/trash.rs`**: Changed `map_err` to `.with_context(…)` to preserve
  original `io::Error` in the error chain so `is_skippable_error()` can
  downcast correctly.
- **`CleanResult::exit_code()`**: Returns 1 when `bytes_freed == 0` and
  `skipped` is non-empty, 0 otherwise.
- All existing `CleanResult` construction sites updated to provide the new
  `skipped` field.

---

## [0.1.22] — 2026-05-25

### Added

- **Parallel scan optimization (PBI-A).** `sasurahime scan` now runs cleaner
  `detect()` calls in parallel via `rayon::par_iter()`, with a consolidated
  progress bar showing scan completion count. Scan time is bounded by the
  slowest cleaner rather than the sum of all cleaners.
- **`Cleaner::is_available()` trait method.** Binary-checking cleaners (uv,
  brew, mise, rustup, apfs-snapshot, ollama, and all GenericCleaner command
  variants) now expose availability via `self.runner.exists("tool")`. The scan
  phase pre-filters unavailable cleaners, skipping `detect()` entirely for
  tools not installed — eliminating I/O overhead for 40+ unused targets.
- **Parallel scan in interactive/auto modes.** Both `sasurahime --yes` and the
  interactive TUI scan phases use the same parallel `with_parallel_scan()`
  progress bar.

### Changed

- **Scanner progress display:** Replaced per-cleaner sequential spinners
  (`"Scanning uv..."`, `"Scanning brew..."`, …) with a single consolidated
  bar: `[{spinner} {bar:20}] Scanning... (3/12)`.
- **`with_spinner()` retained** for potential future use (marked
  `#[expect(dead_code)]`).

### Internal

- **New dependency:** `rayon = "1"` added to `Cargo.toml`.
- **New progress helper:** `with_parallel_scan(total, |pb| …)` in
  `src/progress.rs` — accepts `&ProgressBar` for rayon-thread-safe progress
  updates.
- **3 unit tests** in `scanner.rs`: empty cleaners, mixed availability,
  default `is_available()` returns true.
- **2 integration tests** in `tests/scan.rs`: table headers, size display
  with existing cache dir.
- Updated `tests/interactive.rs` spinner assertions to match consolidated
  progress output; updated VERSION constant to `0.1.22`.

---

## [0.1.21] — 2026-05-25

### Added

- **primary_target now available for all 17 cleaners.** The remaining 5 cleaners
  (browser, cargo, device_support, log, apfs_snapshot) now report their primary
  path when `--verbose` is active. browser → `~/.cache/puppeteer/chrome`,
  cargo → `~/.cargo/registry/cache`, device_support → `~/Library/Developer/Xcode`.
  log and apfs_snapshot explicitly return `None` (multiple targets / no user
  HOME directory).
- **14 new unit tests** covering `primary_target` behavior for all 5
  previously-missing cleaners (both verbose-on and verbose-off cases).

### Changed

- **Non-running app cache cleanup (hint.rs):** `sasurahime --yes` and interactive
  auto-clean now prompt for non-running apps with known cache directories and
  delete them directly (no quit/relaunch needed). Previously only running apps
  were offered for cleanup.
- **Double confirmation suppressed (interactive.rs):** `run_auto` and
  `run_interactive` now suppress secondary confirmation prompts inside cleaners
  via `set_skip_confirm(true)` since the TUI already asked "Proceed?".

### Internal

- **Completed plans archived:** Moved 3 completed implementation plans
  (lucky-panda, explore-command, ios-backup-apfs-snapshot) to `.plan/archived/`.

---

## [0.1.20] — 2026-05-24

### Added

- **`sasurahime explore` — OmniDiskSweeper-style disk explorer.** Scans
  `~/Library/Application Support/`, `~/Library/Caches/`, `~/.cache/`, and
  `~/.local/share/` at first-level depth, groups by app name, and sorts by
  size descending. Unlike `scan`, `explore` covers every app folder —
  not just registered cleaners — answering "who is eating my disk?" without
  prior knowledge of the culprit.

  The interactive output has two sections:

  - **Managed** — paths owned by a registered sasurahime cleaner. Select
    entries to run `sasurahime clean <target>` in-session. After cleaning,
    the managed table is re-scanned and reprinted with updated sizes.
  - **Not managed** — everything else. Select entries to display the full
    path (for copy-paste) and optionally open the folder in Finder.

  Options: `--top N` (default 20, per section), `--all` (show everything),
  `--dir PATH` (repeatable; replaces default roots entirely), `--dry-run`
  (forwarded to any clean subprocess). Requires an interactive TTY; exits
  with code 1 in non-TTY environments.

- **`src/explorer.rs`** new module implementing:
  - `MANAGED_PATTERNS` — 18 hardcoded path patterns mapping cache directories
    to their `sasurahime clean <target>` names, including glob-prefix support
    (`ms-playwright*`).
  - `collect_entries` — first-level directory scan; skips missing roots,
    permission errors, and size-0 entries silently.
  - `apply_top` — `sort_unstable_by_key(Reverse(size))` + truncate.
  - `explore_results` — `pub(crate)` testable core (no dialoguer dependency).
  - `run_explore` — full interactive flow with TTY guard, `dialoguer::MultiSelect`
    for both sections, subprocess spawn via `std::env::current_exe()`.

### Documentation

- **HOWTO-USE.md** (EN + JA): added `sasurahime explore` section with
  annotated output example, two-section behaviour description, options table,
  and usage examples.

### Internal

- 18 new unit tests in `src/explorer.rs` covering all pure functions
  (`is_managed`, `default_roots`, `collect_entries`, `apply_top`,
  `explore_results`) with `tempfile::TempDir` fixtures. No real `~/Library`
  access in tests.

---

## [0.1.19] — 2026-05-23

### Added

- **`--verbose` flag for detailed operation output.** When `--verbose` is set:
  - `sasurahime scan` displays a 4th "Target" column showing the primary cache
    directory each cleaner monitors (e.g. `~/.cache/uv`, `~/.colima`).
  - `sasurahime clean` prints per-file/dir removal details for every cleaner.
  - The flag is global and works with all subcommands (scan, clean, targets).
- **`--dry-run` global flag.** Shows what would be cleaned without deleting
  anything. Works alongside `--verbose` for previewing cleanup operations.

### Changed

- **`sasurahime targets` output is now sorted alphabetically.** Previously the
  target list appeared in declaration order (macro order + manual entries
  appended). Now all targets are sorted by name for easier browsing.
- **Scan result table now includes a "Target" column.** This column is always
  present in the table but shows `"-"` when `--verbose` is not active. Each
  cleaner reports its primary monitored path when verbose mode is on.

## [0.1.18] — 2026-05-22

### Internal

- **Applied `cargo fmt` to fix formatting drift** across
  `src/cleaners/generic.rs` and `tests/interactive.rs` after the v0.1.16
  colima review changes.

## [0.1.17] — 2026-05-22

### Internal

- **Archived completed design specs:** Moved 3 fully-implemented spec files
  (`ios-backup-apfs-snapshot-design`, `command-timeout-hint-design`,
  `running-process-hint-design`) from `docs/superpowers/specs/` to
  `.plan/archived/`.

## [0.1.16] — 2026-05-22

### Added

- **colima deletion confirmation prompt:** `sasurahime clean colima` now shows a
  confirmation prompt on interactive TTYs before running `colima prune --all --force`,
  warning about deletion of all stopped VM disk data (containers, images, volumes).
  In `--yes` (non-TTY) mode the confirmation is skipped automatically.
- **colima fallback deletion:** When the `colima` CLI is not installed but
  `~/.colima/` exists, `sasurahime clean colima` now falls back to directly
  deleting the directory via Trash (with `chflags nouchg` handling).
- **colima non-zero exit warning:** If `colima prune` exits with a non-zero
  status, sasurahime now prints a warning with the exit code and stderr output
  instead of silently treating it as success.

### Changed

- **GenericCleaner now supports `confirm_message` and `fallback_delete`:**
  Two new optional fields on `GenericCleaner` enable any command-based cleaner
  to request interactive confirmation and/or fall back to direct directory
  deletion when the CLI tool is missing. `colima_prune` is the first consumer.

### Fixed

- **colima prune hangs on confirmation prompt:** `colima prune --all` was invoked
  without the `--force` flag, causing it to wait for `y/N` input indefinitely.
  Added `--force` to skip the interactive prompt and proceed non-interactively.

### Documentation

- **SUPPORTED.md note on estimate accuracy:** Added a note to the colima entry
  (EN + JA) that the scan report is an estimated maximum — actual freed space
  depends on which VM disk images are pruned.
- **Colima target description updated:** Changed from `"Colima VM disk cache
  prune"` to `"Colima VM disk images (inactive) prune"` to more accurately
  reflect the deletion scope.
- **Documentation command examples updated:** All `colima prune --all`
  references in SUPPORTED.md, HOWTO-USE.md, and CHANGELOG.md updated to
  `colima prune --all --force`.

### Internal

- **Test version string deduplication:** `tests/interactive.rs` version
  assertions now use a shared `const VERSION` instead of 7 hardcoded strings.
- **Test refactoring:** `command_with_detect_dir_returns_not_found_when_tool_missing`
  replaced with `command_with_detect_dir_fallback_reports_pruneable_when_tool_missing`
  (using factory method) and `command_without_fallback_returns_not_found_when_tool_missing`.
- **Testing team auto-fixes:** E2E test assertion strengthened from
  `calls.contains("prune --all")` to `calls.contains("prune --all --force")`.
  Unit test args updated from `&["prune", "--all"]` to `&["prune", "--all", "--force"]`.

## [0.1.15] — 2026-05-22

### Fixed

- **colima prune hangs on confirmation prompt:** `colima prune --all` was invoked
  without the `--force` flag, causing it to wait for `y/N` input indefinitely.
  Added `--force` to skip the interactive prompt and proceed non-interactively.

## [0.1.14] — 2026-05-22

### Added

- **Command timeout hint:** When a command-based cleaner (colima, docker, brew,
  etc.) times out (>30s), the error message now includes the full command and a
  hint to run it manually: `You can run this command manually in another terminal:
  $ colima prune --all --force`. Applies to all 20+ cleaners that delegate to external
  CLIs.
- **`with_spinner_result()`:** New progress helper that prints `[FAILED]` instead
  of the misleading `[OK]` when a clean operation fails (e.g. due to timeout).

### Fixed

- **Misleading `[OK]` on timeout:** Previously `Cleaning colima... [OK]` was
  printed even when the command timed out and was killed. Now shows
  `[FAILED]` with the error details.

## [0.1.13] — 2026-05-21

### Fixed

- **Rust 1.95.0 clippy warnings:** Removed unnecessary borrows (`&[0u8; N]` →
  `[0u8; N]`) in test code across `src/cleaners/rustup.rs`, `src/cleaners/uv.rs`,
  and `src/format.rs` to satisfy the new `needless_borrows_for_generic_args`
  lint.
- **Test version strings:** Updated version assertions in
  `tests/interactive.rs` from `0.1.12` to `0.1.13` to match the new version.

## [0.1.12] — 2026-05-21

### Added

- **IosCleaner (PBI-012):** Scan and remove iOS device backups from
  `~/Library/Application Support/MobileSync/Backup/`. Interactive only (never
  runs in `--yes` mode) — backups are sent to Trash with `chflags nouchg`
  handling. Reachable via `sasurahime clean ios-backup`.
- **ApfsSnapshotCleaner (PBI-013):** List and delete APFS local Time Machine
  snapshots via `tmutil deletelocalsnapshot`. Interactive only — warns about
  losing local Time Machine protection. Reachable via
  `sasurahime clean apfs-snapshot`.

## [0.1.11] — 2026-05-21

### Fixed

- **Sparse file size over-reporting (colima):** `dir_size()` now uses physical
  disk blocks (`st_blocks × 512`) instead of logical file size (`st_size`).
  This fixes `sasurahime scan` showing wildly inflated sizes for sparse VM disk
  images — e.g. colima dropped from 100.3 GB to the correct 9.3 GB, matching
  `du`.
- **LogCleaner logical-size bug:** `detect()` and `clean()` were using
  `m.len()` (logical size) instead of `m.blocks() × 512` (physical blocks),
  inconsistent with all other cleaners after the `dir_size()` fix.
- **CommandWithDetectDir false-positive scan:** `detect()` for colima,
  simulator, maven, and flutter now checks that the cleaning tool is actually
  installed before reporting the directory as reclaimable. Previously, if the
  tool was removed but `~/.[colima|maven|flutter]` remained, scan would show a
  large pruneable size but `clean` would silently skip.
- **UvCleaner under-reporting:** `detect()` was only measuring the
  `archive-v0` subdirectory but `clean()` also frees old `simple-vN` index
  dirs. `detect()` now measures the full cache dir, matching `clean()`.
- **RustupCleaner hardcoded size estimate:** Replaced the fixed 300 MB per
  toolchain guess with actual `dir_size()` measurement of each unused
  toolchain in `~/.rustup/toolchains/<name>/`.

### Changed

- **Audited all 16+ cleaners for size-reporting accuracy.** No other
  discrepancies found beyond the above. Lower-severity observations (e.g.
  `brew` uses two different measurement sources, `huggingface` CLI path
  credits full dir size) are documented as inherent design trade-offs rather
  than bugs.

## [0.1.10] — 2026-05-21

### Changed

- **Dependency version bumps:** Updated four crate dependencies to latest
  compatible versions — dialoguer 0.11→0.12, dirs 5→6, indicatif 0.17→0.18,
  toml 0.8→1.1. No source code changes were required; all APIs remain
  compatible.

### Fixed

- **rustup test flake:** `rustup_not_found_skips` now restricts PATH to
  `/usr/bin:/bin` so the test does not accidentally find `rustup` installed
  on the host machine and proceed to clean real toolchains.

---

## [0.1.8] — 2026-05-21

### Added

- **4 new clean targets:** `volta`, `sbt`, `tree-sitter`, `vscode-extensions`.
- **vscode-logs** built-in target added to LogCleaner (`~/Library/Application Support/Code/logs`).
- SUPPORTED.md updated to document all 43 clean targets (previously undocumented: `colima`, `simulator`, `vscode-extensions`, `maven`, `terraform`, `flutter`, `ollama`, `device-support`).
- HOWTO-USE.md target tables (EN + JA) updated with 11 missing entries: `colima`, `device-support`, `flutter`, `maven`, `ollama`, `sbt`, `simulator`, `terraform`, `tree-sitter`, `volta`, `vscode-extensions`.

### Changed

- Cleaner count in SUPPORTED.md: English 35 → 43, Japanese 32 → 43.
- Japanese section of SUPPORTED.md now includes translations for `volta`, `sbt`, `tree-sitter`. (These existed in English-only since 0.1.7.)

---

## [0.1.7] — 2026-05-21

### Added

- Docs site navigation integration: language-aware nav labels in doc pages
  (`switchLang()` JS を拡張、JA表示時に「使い方」「ターゲット一覧」に切替).
- `.lang-btn` CSS class added to shared stylesheet for unified language switcher
  appearance across landing pages and doc pages.
- Top link in doc page footer for quick navigation to the top of the site.
- Cross-reference links (`See also` / `関連ドキュメント`) added to both EN and
  JA sections of HOWTO-USE.md.
- Language switcher visual unified across all pages: landing pages now use
  the same `.lang-btn` button style as doc pages (labels `EN`/`JA`).
- Footer expanded on both landing pages to link all 3 documentation pages
  (How to Use, Supported Targets, Add a Target) in the appropriate language.
- `<html dir="ltr">` attribute on all pages for explicit text direction.
- `og:url` meta tags on both landing pages for correct language-specific URLs.
- Author metadata (`authors`) added to `Cargo.toml`.
- `docs/_site/` and `.superpowers/` added to `.gitignore`.
- **Progress bar with ETA and per-file speed** for all multi-entry cleaners:
  shows `removing BloatApp, 45.2 MB/s (3/15)` during cleanup.
- `ProgressReporter` trait with three implementations: `VerboseProgress`
  (indicatif progress bar), `SuppressReporter` (spinner only),
  `DeepSuppressReporter` (silent).
- `--suppress` CLI flag: hides progress bar, keeps spinner and freed summary.
- `--deep-suppress` CLI flag: hides all output (spinner, freed line, progress).
- `suppress` / `deep_suppress` config options in `~/.config/sasurahime/config.toml`.
- `format_speed()` helper for per-file MB/s calculation in progress bar.

### Changed

- `Cleaner` trait `clean()` method now accepts `&dyn ProgressReporter` parameter.
- `run_clean_target()` updated to conditionally show spinner based on reporter.
- Documentation navigation reorganized: bilingual Markdown files consolidated,
  EN/JA language switcher added to doc pages, dark theme unified across all
  doc pages and landing pages.
- Landing page nav links renamed: `Documentation` → `How to Use`, with
  additional links to `Supported Targets` and `Add a Target`.
- Redundant `.lang-btn` CSS removed from inline `<style>` in `_layouts/doc.html`
  (moved to shared `docs/assets/style.css`).
- Redundant `.lang-switch a/span` CSS rules removed (superseded by `.lang-btn`).

---

## [0.1.6] — 2026-05-20

### Added

- GitHub Pages site at `armaniacs.github.io/sasurahime` with landing page and documentation.
- `Cargo.toml` `repository` and `description` fields.

### Changed

- License changed from MIT to Apache-2.0.
- Landing page redesigned with dark theme, Japanese version added.
- `.gitignore` cleaned up (deduplicated `target/`, added `*.bak`, `*.log`, `tags`, `__pycache__/`, `.env`, `.playwright-mcp/`).

---

## [0.1.5] — 2026-05-20

### Added

- `sasurahime targets` subcommand listing all supported clean targets.
- Version banner now visible in `sasurahime -h` / `--help` output and at interactive startup.
- Blog articles for Zenn (`blog/compact.md`, `blog/full.md`) covering usage walkthrough and full feature guide.

### Changed

- **Version banner moved from stdout to stderr.** All commands now print
  `sasurahime v0.1.5` on stderr at startup. Scripts using `sasurahime scan | grep`
  or similar no longer see the version string in their piped output.
- **CleanTarget match arms refactored.** 14 duplicated arms extracted into
  `run_clean_target()` helper. New target additions now need only 1 line per arm.
- **ProgressStyle cached** via `OnceLock`. Template parsing happens once, not
  per spinner instance.
- **Spinner completion marker** changed from `✓` to `[OK]` for screen-reader
  compatibility.
- **Release profile tuned:** `opt-level` changed from `z` to `s` (better CLI
  speed/size balance), `lto` changed from full to `"thin"`.
- **Xcode --yes mode:** warning printed to stderr when Xcode is running.
- **MiseCleaner non-directory entries** (`.DS_Store`, `.mise.backend`) are now
  skipped. `remove_with_uchg` errors are logged and skipped instead of stopping
  the entire clean.
- `Cargo.toml` `license = "MIT"` added.
- Repository published at `github.com/armaniacs/sasurahime`.

### Dependencies

- `windows-sys` versions deduplicated via `cargo update`.

---

## [0.1.4] — 2026-05-20

### Added

- `--permanent` flag: bypass Trash and permanently delete files.
- `trash_mode` config option in `~/.config/sasurahime/config.toml`.

### Changed

- **Trash mode is now the default.** All deleted files are sent to macOS Trash
  instead of being permanently removed. Set `trash_mode = false` in config or
  pass `--permanent` to opt out.
- **`--yes --permanent` shows a confirmation prompt** before permanently deleting
  all pruneable targets (moved from `--yes` with old `--trash` flag).
- `--trash` flag renamed to `--permanent` (inverted semantics).
- **Release profile further optimized for binary size:** `lto = "thin"` → `true`,
  `panic = "abort"` added, `opt-level = "s"` → `"z"`. Binary reduced from
  1,349 KB to 872 KB (35% reduction).

---


## [0.1.2] — 2026-05-18

### Added

- `sasurahime --version` / `sasurahime -V` — prints version number.
- `sasurahime targets` — lists all 14 supported clean targets with descriptions.
- Version banner displayed on interactive (`sasurahime`) and auto (`sasurahime --yes`) startup.
- Test coverage gap audit plan (`docs/coverage-gap-plan.md`, `docs/coverage-gap-summary.json`).
- **MiseCleaner (GAP-001):** `.mise.toml` pinning cross-check — versions pinned in
  `~/.config/mise/config.toml` or any `.mise.toml` under HOME (max depth 5) are
  protected from deletion. Added `MiseCleaner::scan_pinned_versions` with TOML
  parser (`parse_tools_section`, `parse_toml_kv`). (`MiseCleaner` now stores a
  `home` field for robust path resolution.)
- **E2E test:** `clean_mise_pinned_version_not_deleted` — validates that a
  `.mise.toml`-pinned version is preserved and an unpinned version is removed.
- **Unit tests:** `unused_versions_pinned_is_protected` for the `pinned` set
  intersection. `expand_tilde_tilde_alone` for `Config::expand_tilde("~")`.
  `version_key_empty_string_returns_empty` and
  `find_old_versions_skips_unparseable_dir_name` for `BrowserCleaner`.
  `find_old_versions_skips_symlinks` for browser symlink guard.
- Uv, Xcode, Log edge-case unit tests (symlink guard, `--yes` bypass E2E,
  DST/time boundaries, missing metadata).

### Changed

- **Critical** `MiseCleaner::remove_with_uchg` (GAP-002): chflags errors are now
  propagated via `?` instead of being silently ignored with `let _ = ...`.
- **Medium** `BrewCleaner::parse_size_str` (GAP-003): now accepts lowercase units
  (`gb`, `mb`, `kb`) and space-separated forms (`194.3 MB`).
- **High** `BrowserCleaner::find_old_versions` (GAP-004 / GAP-005): filters out
  empty version keys (panics guard) and symlinks before processing entries.
- **Medium** `GenericCleaner::clean` (GAP-010): `DeleteDirs` path now calls
  `chmod -R nouchg` before `remove_dir_all`.
- **Low** `UvCleaner::detect_old_indexes` (GAP-006): skips symlink entries.
- Reformatted multi-line struct initializations and assertions across
  `src/cleaners/generic.rs`, `tests/generic.rs`, and `tests/npm.rs` for improved
  readability.

### Dependencies

- `filetime` added to `Cargo.lock` (dev-dependency, used in `tests/log.rs`).

---

## [0.1.0] — 2026-05-18

### Added

- `sasurahime scan` — scans all known cache locations and prints a summary table with reclaimable sizes.
- `sasurahime clean <target>` — cleans individual cache targets by name.
- `sasurahime clean <target> --dry-run` — previews what would be removed without deleting anything.
- **uv (PBI-002):** Removes stale `simple-vN` index directories (keeps only the latest) and runs `uv cache prune --force`.
- **Homebrew (PBI-003):** Runs `brew cleanup -s --prune=all` and reports freed disk space.
- **mise (PBI-004):** Calls `mise ls --current` to determine active versions, then removes unused versions from `~/.local/share/mise/installs/` with macOS `uchg` flag handling.
- **Browsers (PBI-005):** Removes old Puppeteer Chrome / Chrome-Headless-Shell directories and old Playwright (`ms-playwright*)` builds while keeping the highest version.

### Architecture

- Introduced the `Cleaner` trait (`name`, `detect`, `clean`) as the core abstraction for all cleaners.
- Introduced the `CommandRunner` trait to allow test-time mocking of external command execution (uv, brew, mise, chflags).
- Added `sasurahime --yes` entry point for non-interactive full-cleaning (placeholder for Sprint 4 TUI integration).
- Interactive TUI via `dialoguer::MultiSelect` — UI placeholder (PBI-008, Sprint 4).
- Full E2E test suite (assert\_cmd + tempfile fixtures) for every cleaner.
- Unit tests for pure helper functions (e.g. `parse_active_versions`, `version_key`, `find_old_versions`).
