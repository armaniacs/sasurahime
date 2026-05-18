# Version Display & Targets Subcommand Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--version` flag, startup version banner, and `targets` subcommand to sasurahime.

**Architecture:** Three self-contained changes in `src/main.rs`: (1) clap derive attribute for version, (2) startup println in the `None` match arm, (3) new `Targets` variant with a static const array and a print function. Three E2E tests in `tests/interactive.rs`.

**Tech Stack:** Rust + clap 4 (derive API) + assert_cmd + tempfile

---

### Task 1: `--version` / `-V` flag

**Files:**
- Modify: `src/main.rs:15-16` — add `version` to clap derive
- Test: `tests/interactive.rs` (new test)

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn version_flag_output() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .arg("--version")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sasurahime"), "stdout: {stdout}");
    assert!(stdout.contains("0.1.2"), "stdout: {stdout}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test version_flag_output -- --nocapture`
Expected: FAIL — clap outputs no version info yet, or `sasurahime 0.1.2` not found.

- [ ] **Step 3: Add `version` to clap derive**

In `src/main.rs`, change:

```rust
#[derive(Parser)]
#[command(name = "sasurahime", about = "macOS developer cache cleaner")]
struct Cli {
```

to:

```rust
#[derive(Parser)]
#[command(
    name = "sasurahime",
    version = env!("CARGO_PKG_VERSION"),
    about = "macOS developer cache cleaner"
)]
struct Cli {
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test version_flag_output -- --nocapture`
Expected: PASS — stdout contains `sasurahime 0.1.2`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add --version flag via clap derive"
```

---

### Task 2: Startup version banner

**Files:**
- Modify: `src/main.rs` — add `println!` in `None` branch
- Test: `tests/interactive.rs` (new test)

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn startup_version_display_yes() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .arg("--yes")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // The --yes path prints version first, then "Nothing to clean." (or cleanup output)
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "stdout must start with version, got: {stdout}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test startup_version_display_yes -- --nocapture`
Expected: FAIL — stdout currently starts with `Nothing to clean.` or cleanup output.

- [ ] **Step 3: Add startup banner**

In `src/main.rs`, find the `None => {` arm and add a `println!` before everything else:

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

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test startup_version_display_yes -- --nocapture`
Expected: PASS — stdout starts with `sasurahime v0.1.2`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: show version banner on interactive/auto startup"
```

---

### Task 3: `targets` subcommand

**Files:**
- Modify: `src/main.rs` — add `Targets` variant, `SUPPORTED_TARGETS` const, `print_targets()`, match arm
- Test: `tests/interactive.rs` (new test)

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn targets_subcommand_output() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .arg("targets")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain a selection of targets
    assert!(stdout.contains("uv"), "stdout: {stdout}");
    assert!(stdout.contains("brew"), "stdout: {stdout}");
    assert!(stdout.contains("logs"), "stdout: {stdout}");
    assert!(stdout.contains("xcode"), "stdout: {stdout}");
    // Should have descriptions
    assert!(stdout.contains("Stale"), "stdout: {stdout}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test targets_subcommand_output -- --nocapture`
Expected: FAIL — `sasurahime targets` is unrecognised by clap, exits with error.

- [ ] **Step 3: Add `Targets` variant, const data, and printing logic**

In `src/main.rs`:

**a) Add `Targets` to `Commands` enum:**

```rust
#[derive(Subcommand)]
enum Commands {
    /// Scan cache locations and report sizes
    Scan,
    /// Clean a specific cache target
    Clean {
        #[command(subcommand)]
        target: CleanTarget,
    },
    /// List supported cache targets
    Targets,
}
```

**b) Add `SUPPORTED_TARGETS` const near the top of the file (after imports, before `Cli`):**

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

**c) Add `Commands::Targets` match arm in `main()`:**

```rust
Commands::Targets => {
    for (name, desc) in SUPPORTED_TARGETS {
        println!("{:<12} {}", name, desc);
    }
}
```

Place this after the `Commands::Scan` arm and before `None =>`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test targets_subcommand_output -- --nocapture`
Expected: PASS — stdout contains `uv`, `brew`, `logs`, `xcode`, `Stale`.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add targets subcommand listing all supported clean targets"
```

---

### Task 4: Final verification

- [ ] **Step 1: Run full quality gates**

```bash
cargo fmt --check
cargo clippy --tests -- -D warnings
cargo test
```

Expected: all pass, 0 warnings, 0 failures, all existing tests plus 3 new ones pass.

- [ ] **Step 2: Manual smoke test (optional)**

```bash
cargo run -- --version
cargo run -- --yes
cargo run -- targets
```

Expected:
- `--version` → `sasurahime 0.1.2`
- `--yes` → first line `sasurahime v0.1.2`
- `targets` → table of 14 target names + descriptions

- [ ] **Step 3: Commit any final adjustments**

```bash
git add -A
git commit -m "chore: final verification passes"
```
