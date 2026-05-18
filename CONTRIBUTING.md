# Contributing

Thank you for your interest in sasurahime. This document covers the
project structure, development workflow, and conventions.

---

## Table of Contents

- [Project structure](#project-structure)
- [Architecture overview](#architecture-overview)
  - [The `Cleaner` trait](#the-cleaner-trait)
  - [The `CommandRunner` trait](#the-commandrunner-trait)
  - [Scanner and formatting](#scanner-and-formatting)
- [Testing approach](#testing-approach)
- [Adding a new cleaner](#adding-a-new-cleaner)
- [Development workflow](#development-workflow)
- [Quality gates](#quality-gates)
- [Branch naming](#branch-naming)
- [Commit messages](#commit-messages)
- [Pull requests](#pull-requests)

---

## Project structure

```
src/
  main.rs             # CLI definition (clap), entry point, wiring
  cleaner.rs          # Core Cleaner trait, ScanResult / CleanResult types
  command.rs          # CommandRunner trait + SystemCommandRunner impl
  config.rs           # Config file loader (TOML)
  format.rs           # dir_size, format_bytes helpers
  scanner.rs          # Scan table output (comfy_table)
  interactive.rs      # TUI (dialoguer::MultiSelect) and --yes runner
  cleaners/
    mod.rs
    uv.rs             # Cleaner for uv cache (simple-vN indexes)
    brew.rs           # Cleaner for Homebrew downloads
    mise.rs           # Cleaner for mise runtime versions
    browser.rs        # Cleaner for Puppeteer / Playwright browsers
    generic.rs        # Generic cleaners: bun, go, pip, node-gyp, npm, yarn, pnpm
    log.rs            # Cleaner for log files (kilo, opencode, claude-code + extras)
    xcode.rs          # Cleaner for Xcode DerivedData
tests/
  *.rs                # E2E integration tests (assert_cmd + tempfile)
pbi/
  *.md                # Product Backlog Items with Gherkin acceptance scenarios
```

---

## Architecture overview

### The `Cleaner` trait

Every cleaner implements this trait (`src/cleaner.rs`):

```rust
pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self) -> ScanResult;
    fn clean(&self, dry_run: bool) -> Result<CleanResult>;
}
```

**Contract (enforced in CI):**
- `detect()` is **read-only**. It must never create, modify, or delete files.
- `clean(true)` (dry-run) is **side-effect-free**. It must not delete anything.
- `name()` returns a short kebab‑case identifier used in CLI and output.

`ScanResult` carries the cleaner name and a `ScanStatus`:

```rust
pub enum ScanStatus {
    Pruneable(u64),   // Bytes that can be reclaimed
    Clean,            // Nothing to clean
    NotFound,         // Target directory or tool not present
    PermissionDenied, // Cannot read the target
}
```

`CleanResult` reports how many bytes were actually freed.

### The `CommandRunner` trait

External tool invocations (`uv`, `brew`, `mise`, `chflags`, etc.) go through
this trait (`src/command.rs`):

```rust
pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output>;
    fn exists(&self, program: &str) -> bool;
}
```

**Why this exists:** Tests mock `CommandRunner` instead of invoking real
tools. This makes E2E tests fast, deterministic, and safe to run on any
machine.

Two implementations exist:
- `SystemCommandRunner` — real `std::process::Command` for production.
- Various `NoopRunner` / `PgrepRunner` — test stubs in `#[cfg(test)]` blocks.

### Scanner and formatting

`scanner::run_scan` calls `detect()` on every cleaner and prints a
`comfy_table::Table`. No side effects.

Utility functions live in `format.rs`:
- `dir_size(path)` — walks a directory tree, sums file sizes.
- `format_bytes(bytes)` — converts to human-readable string (binary units).

---

## Testing approach

sasurahime follows **Outside‑In TDD** with three layers:

### 1. E2E tests (`tests/*.rs`)

Each cleaner has at least one E2E test. These spawn the real binary via
`assert_cmd::Command` with a `tempfile::TempDir` as `HOME`.

Pattern:

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_<target>_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    // … set up fixture directories …
    let output = sasurahime(tmp.path())
        .args(["clean", "<target>", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    // … assert files still exist …
}
```

**Mocking external tools**: Place a fake shell script in `bin/` under the
tempdir and prepend it to `PATH`:

```rust
fn install_fake_tool(bin_dir: &Path, name: &str) {
    fs::write(bin_dir.join(name), "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join(name),
            fs::Permissions::from_mode(0o755)).unwrap();
    }
}

let output = sasurahime(tmp.path())
    .env("PATH", format!("{}:{}", bin_dir.display(), original_path))
    .args(["clean", "<target>"])
    .output()
    .unwrap();
```

### 2. Integration tests (inline `#[cfg(test)]` modules)

Test a cleaner's `detect()` or `clean()` with a mock `CommandRunner`,
using a `TempDir` for filesystem fixtures. Found inside each
`src/cleaners/<name>.rs`.

### 3. Unit tests (inline `#[cfg(test)]` modules)

Pure function tests — no filesystem or process dependency.
Examples: `parse_simple_version`, `version_key`, `parse_size_str`,
`parse_active_versions`, `is_older_than`.

---

## Adding a new cleaner

1. **Create a PBI** in `pbi/` with a Gherkin scenario (see existing PBIs
   for format).

2. **Add the cleaner module** `src/cleaners/<name>.rs`:

   - Define a struct with a `runner: Box<dyn CommandRunner>` field.
   - Implement `Cleaner` for it.
   - Add `pub mod <name>;` to `src/cleaners/mod.rs`.

3. **Wire it into the CLI** in `src/main.rs`:

   - Add a variant to `CleanTarget` enum.
   - Add a match arm in `fn all_cleaners()`.
   - Add a match arm in the `Clean { target }` branch of `main()`.

4. **Add tests**, inside-out:

   - Unit tests for any pure helper functions.
   - Integration tests in the module's `#[cfg(test)]` block.
   - E2E test in `tests/<name>.rs` (tempdir + assert_cmd).

5. **Verify** with the quality gates below.

---

## Development workflow

1. Pick a PBI from `pbi/` and create a branch:
   ```
   git checkout -b feat/PBI-NNN-description
   ```

2. Write the E2E test first (it will fail — that is expected).

3. Implement the cleaner until the test passes.

4. Add integration and unit tests.

5. Run quality gates locally before pushing.

---

## Quality gates

Every PR must pass all of the following. CI enforces them automatically:

```bash
# Formatting
cargo fmt --check

# Lints (zero warnings)
cargo clippy --tests -- -D warnings

# All tests (unit + integration + E2E)
cargo test
```

Additionally, verify that:
- `detect()` never deletes files:
  ```
  cargo test <cleaner>_dry_run
  ```
- `clean(true)` never deletes files:
  ```
  cargo clean <target> --dry-run
  sasurahime clean <target> --dry-run   # from built binary
  ```

---

## Branch naming

| Type       | Pattern                  | Example                      |
|------------|--------------------------|------------------------------|
| New feature | `feat/PBI-NNN-description` | `feat/PBI-002-uv-cache`    |
| Bug fix    | `fix/description`         | `fix/mise-home-resolution`  |
| Chore      | `chore/description`       | `chore/update-deps`         |
| Docs       | `docs/description`        | `docs/add-howto`            |
| Test       | `test/description`        | `test/coverage-gaps`        |

Test branches (like `gap-2026-05-18`) are also used for test-coverage
campaigns.

---

## Commit messages

Use a conventional prefix:

```
feat:     new feature
fix:      bug fix
chore:    tooling, dependencies, CI
docs:     documentation
test:     test additions or changes
refactor: code restructuring with no behaviour change
```

Examples:

```
feat(PBI-002): implement uv cache cleaner with dry-run support
fix: resolve home path resolution in MiseCleaner
docs: add HOWTO-USE and SUPPORTED reference
test: add E2E test for mise pinned version protection
```

The commit body should explain **why** the change was made, not just what
changed. Reference PBI numbers when applicable.

---

## Pull requests

1. **One PBI per PR.** Each PR addresses a single concern.

2. **Reference the PBI** in the title:
   ```
   feat(PBI-002): uv cache cleaner
   ```

3. **Use the PR description** to summarise what changed and why.

4. **Open an issue first** for new features or significant changes,
   so we can discuss scope before implementation.

5. **Keep PRs small.** If a PBI is large, break it into sub-tasks and
   submit multiple PRs against the feature branch.
