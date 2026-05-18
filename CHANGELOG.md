# Changelog

All notable changes to sasurahime will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.2] 2026-05-18

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
