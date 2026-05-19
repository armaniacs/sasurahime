# Version Display & Targets Subcommand

**Date:** 2026-05-18
**Status:** Approved
**Branch:** `feature-0518`

---

## Summary

Add two UX improvements to sasurahime: (1) show the version number on
`--version` flag and on interactive startup, and (2) add a `targets`
subcommand listing all supported clean targets with descriptions.

---

## Features

### 1. `--version` / `-V` flag

**Implementation:** Add `version = env!("CARGO_PKG_VERSION")` to the
`#[command()]` attribute on the `Cli` struct.

```rust
#[derive(Parser)]
#[command(
    name = "sasurahime",
    version = env!("CARGO_PKG_VERSION"),
    about = "macOS developer cache cleaner"
)]
```

**Behaviour:**
- `sasurahime --version` → `sasurahime v0.1.2`
- `sasurahime -V` → `sasurahime v0.1.2`

No changes to `Cargo.toml` needed — `CARGO_PKG_VERSION` is set by the
build system.

---

### 2. Startup version banner

**Location:** `main()`, `None` branch (no subcommand given).

**Current:**
```rust
None => {
    let cleaners = all_cleaners(&home, &config);
    if cli.yes {
        interactive::run_auto(&cleaners)?;
    } else {
        interactive::run_interactive(&cleaners)?;
    }
}
```

**After:**
```rust
None => {
    println!("sasurahime v{}", env!("CARGO_PKG_VERSION"));
    let cleaners = all_cleaners(&home, &config);
    if cli.yes {
        interactive::run_auto(&cleaners)?;
    } else {
        interactive::run_interactive(&cleaners)?;
    }
}
```

**Behaviour:**
- `sasurahime --yes` → first line: `sasurahime v0.1.2`, then cleanup output
- `sasurahime` (TTY) → first line: `sasurahime v0.1.2`, then TUI

**Not affected:**
- `sasurahime scan` → no version banner (use `--version` to check)
- `sasurahime clean <target>` → no version banner
- `sasurahime --help` → clap's default help includes the version

---

### 3. `targets` subcommand

**New enum variant:**

```rust
#[derive(Subcommand)]
enum Commands {
    Scan,
    Clean { ... },
    /// List supported cache targets
    Targets,
}
```

**New const data:**

```rust
const SUPPORTED_TARGETS: &[(&str, &str)] = &[
    ("uv",       "Stale simple-vN index directories + uv cache prune"),
    ("brew",     "Homebrew download cache"),
    ("mise",     "Unused runtime versions"),
    ("browsers", "Old Puppeteer / Playwright builds"),
    ("bun",      "Bun package cache"),
    ("go",       "Go build cache"),
    ("pip",      "pip package cache"),
    ("node-gyp", "node-gyp build cache directories"),
    ("npm",      "npm package cache"),
    ("yarn",     "yarn cache"),
    ("pnpm",     "pnpm store"),
    ("caches",   "All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)"),
    ("logs",     "Log files older than N days"),
    ("xcode",    "Xcode DerivedData project directories"),
];
```

**Output format:** Name left-padded to 12 characters, description follows.

```
$ sasurahime targets
uv          Stale simple-vN index directories + uv cache prune
brew        Homebrew download cache
mise        Unused runtime versions
browsers    Old Puppeteer / Playwright builds
bun         Bun package cache
go          Go build cache
pip         pip package cache
node-gyp    node-gyp build cache directories
npm         npm package cache
yarn        yarn cache
pnpm        pnpm store
caches      All generic caches (bun/go/pip/node-gyp/npm/yarn/pnpm)
logs        Log files older than N days
xcode       Xcode DerivedData project directories
```

**Printing logic** (new function in `main.rs` or a dedicated module):

```rust
fn print_targets() {
    for (name, desc) in SUPPORTED_TARGETS {
        println!("{:<12} {}", name, desc);
    }
}
```

---

## Files changed

| File | Change |
|------|--------|
| `src/main.rs` | Add `version` to clap derive; add startup banner; add `Targets` variant; add `SUPPORTED_TARGETS` const; add `print_targets()` |
| `tests/interactive.rs` | Add 3 E2E tests (version flag, targets subcommand, startup banner) |

---

## Test plan

| Test | Method | Assertion |
|------|--------|-----------|
| `version_flag_output` | `assert_cmd::Command` with `--version` | stdout contains `v0.1.2` |
| `targets_subcommand_output` | `assert_cmd::Command` with `targets` | stdout contains `uv`, `brew`, `logs`, `xcode` |
| `startup_version_display_yes` | `assert_cmd::Command` with `--yes` | stdout starts with `sasurahime v0.1.2` |

All tests use `tempfile::TempDir` and prepend PATH as needed.

---

## Self-review

- [x] No placeholders, TBDs, or vague requirements
- [x] Architecture matches existing codebase patterns
- [x] Scope is focused on a single feature set (version + targets)
- [x] All requirements are unambiguous
- [x] Test plan covers all three changes
