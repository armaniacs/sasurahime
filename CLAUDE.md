# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project overview

sasurahime is a macOS developer cache cleaner written in Rust. It scans known cache locations (uv, Homebrew, mise runtimes, Playwright/Puppeteer browsers, bun, Go, pip, etc.), reports disk usage, and selectively removes stale data. See `README.md` for the user-facing description.

This project is in early development. The `pbi/` directory contains the full product backlog with BDD acceptance scenarios that drive implementation.

## Commands

Once the Rust project is initialized:

```bash
cargo build                  # build
cargo test                   # run all tests
cargo test <test_name>        # run a single test by name
cargo test -p sasurahime      # run tests for a specific package (if workspace)
cargo clippy -- -D warnings   # lint (must pass with zero warnings)
cargo fmt --check             # check formatting
```

Target platform: macOS (arm64 + x86_64). Do not add Linux/Windows support unless explicitly requested.

## Architecture

### Core trait

Every cleaner implements a shared `Cleaner` trait:

```rust
trait Cleaner {
    fn name(&self) -> &str;
    fn detect(&self) -> ScanResult;   // returns size and status, no side effects
    fn clean(&self, dry_run: bool) -> Result<CleanResult>;
}
```

`detect()` must never delete anything. `clean(dry_run: true)` must never delete anything either.

### Cleaners (one per PBI)

| PBI | Cleaner | What it touches |
|-----|---------|----------------|
| 002 | `UvCleaner` | `~/.cache/uv/` — calls `uv cache prune --force`, removes old `simple-vN` dirs |
| 003 | `BrewCleaner` | runs `brew cleanup -s --prune=all`, parses freed bytes from stdout |
| 004 | `MiseCleaner` | `~/.local/share/mise/installs/` — reads global + per-project `.mise.toml` before removing |
| 005 | `BrowserCleaner` | `~/.cache/puppeteer/` and `~/Library/Caches/ms-playwright*/` — keeps highest build number only |
| 006 | `GenericCacheCleaner` | delegates to `bun pm cache rm`, `go clean -cache`, `pip cache purge`, removes `node-gyp` dirs |
| 007 | `LogCleaner` | `~/.local/share/kilo/log/` — deletes `*.log` older than N days, skips `dev.log` |

### Entry points

- `sasurahime scan` — runs `detect()` on all cleaners, prints table, exits
- `sasurahime clean <target>` — runs `clean()` on the named cleaner
- `sasurahime` (no args) — interactive TUI via `dialoguer::MultiSelect` (PBI-008)
- `sasurahime --yes` — non-interactive full clean for scripting

### Safety rules

- mise runtime deletion **must** cross-check global `~/.config/mise/config.toml` AND any `.mise.toml` found within HOME (max depth 5) before removing a version.
- When deleting on macOS, handle `uchg` immutable flags: run `chflags -R nouchg <path>` before `rm -rf`.
- External tools (uv, brew, bun, go, pip) are invoked via `std::process::Command`. If the tool is not in PATH, return a `NotFound` status rather than erroring.

## Testing approach (Outside-In TDD)

Start each PBI with an E2E test using a `tempdir` fixture, then add integration and unit tests inward.

- **E2E**: spawn the binary or call the top-level function with a `tempdir` as `HOME`; assert on exit code and stdout.
- **Integration**: construct a `Cleaner` with a fake root path; call `detect()` / `clean(dry_run=true)` and assert on `ScanResult`.
- **Unit**: pure functions like `parse_size_str`, `version_matches_spec`, `is_older_than`.

Mock external commands by injecting a `CommandRunner` trait rather than calling `Command` directly.

## Product backlog

`pbi/` contains 8 PBIs with Gherkin acceptance scenarios. Implement in priority order:

1. PBI-001 scan report (prerequisite for everything)
2. PBI-002 uv + PBI-003 brew (Sprint 1 MVP)
3. PBI-004 mise + PBI-005 browsers (Sprint 2)
4. PBI-006 generic caches + PBI-007 logs (Sprint 3)
5. PBI-008 interactive TUI (Sprint 4)

Recovery sizes in the PBIs are from araki's machine and are illustrative only.
