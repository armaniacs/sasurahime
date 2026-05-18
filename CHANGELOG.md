# Changelog

All notable changes to sasurahime will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Changed

- Reformatted multi-line struct initializations and assertions across `src/cleaners/generic.rs`, `tests/generic.rs`, and `tests/npm.rs` for improved readability.
- `filetime` dependency added to `Cargo.lock`.

---

## [0.1.1] — YYYY-MM-DD

_No changes since 0.1.0 yet._

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
