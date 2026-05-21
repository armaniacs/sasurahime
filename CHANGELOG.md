# Changelog

All notable changes to sasurahime will be documented in this file. The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

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
