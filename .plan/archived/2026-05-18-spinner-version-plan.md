# Universal Version Display & Loading Spinners — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show version number on every invocation and display loading spinners during detect/clean operations.

**Architecture:** Move version banner to top of `main()`, create `src/progress.rs` with a `with_spinner()` helper wrapping `indicatif::ProgressBar`, then wrap detect/clean calls in scanner, interactive, and main modules.

**Tech Stack:** Rust + indicatif 0.17 (already in dependencies) + assert_cmd + tempfile

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/progress.rs` | **Create** — `with_spinner()` helper |
| `src/main.rs` | **Modify** — move version to top, wrap clean arms |
| `src/scanner.rs` | **Modify** — wrap `c.detect()` calls |
| `src/interactive.rs` | **Modify** — wrap `clean()` calls |
| `tests/interactive.rs` | **Modify** — add version-display E2E tests |

---

### Task 1: Universal version display

**Files:**
- Modify: `src/main.rs` — move version println to top, remove from None arm
- Test: `tests/interactive.rs` — 3 new E2E tests

- [ ] **Step 1: Write the failing tests**

Add to `tests/interactive.rs`:

```rust
#[test]
fn version_display_on_scan() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["scan"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "scan output must start with version, got: {stdout}"
    );
}

#[test]
fn version_display_on_targets() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["targets"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "targets output must start with version, got: {stdout}"
    );
}

#[test]
fn version_display_on_clean_dry_run() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("sasurahime v0.1.2"),
        "clean output must start with version, got: {stdout}"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test version_display_on_scan version_display_on_targets version_display_on_clean_dry_run -- --nocapture`
Expected: FAIL — scan/targets/clean currently do not output version banner.

- [ ] **Step 3: Move version to top of main(), remove from None arm**

In `src/main.rs`:

**a)** Add `println!` right after `Cli::parse()`:

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!("sasurahime v{}", env!("CARGO_PKG_VERSION"));   // ← new
    let home = home();
    // … rest unchanged
```

**b)** Remove the duplicate `println!` from the `None => {` arm (line 185 in current code):

```rust
None => {
    // delete this line: println!("sasurahime v{}", env!("CARGO_PKG_VERSION"));
    let cleaners = all_cleaners(&home, &config);
    if cli.yes {
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test version_display_on_scan version_display_on_targets version_display_on_clean_dry_run startup_version_display_yes -- --nocapture`
Expected: PASS — all 4 tests pass (scan, targets, clean dry-run, interactive startup).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: show version on every sasurahime invocation"
```

---

### Task 2: Create `src/progress.rs` with `with_spinner()`

**Files:**
- Create: `src/progress.rs`
- Modify: `tests/interactive.rs` — no change needed (unit test is inline)
- Test: `src/progress.rs` inline unit test

- [ ] **Step 1: Write the failing unit test**

Create `src/progress.rs` with just a stub function to start (TDD: write test first, then implement):

```rust
pub fn with_spinner<R>(_msg: &str, f: impl FnOnce() -> R) -> R {
    f() // stub
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_spinner_returns_value() {
        let result = with_spinner("test", || 42);
        assert_eq!(result, 42);
    }
}
```

- [ ] **Step 2: Add `mod progress;` to main.rs and run test**

In `src/main.rs`, add with the other mod declarations:

```rust
mod cleaner;
mod cleaners;
mod command;
mod config;
mod format;
mod interactive;
mod progress;
mod scanner;
```

Run: `cargo test with_spinner_returns_value -- --nocapture`
Expected: PASS — the stub returns 42 correctly.

- [ ] **Step 3: Implement full `with_spinner()` with indicatif**

Replace the stub in `src/progress.rs`:

```rust
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Runs `f` while displaying a spinner with `msg`.
/// On completion the spinner is replaced with a check-mark line.
pub fn with_spinner<R>(msg: &str, f: impl FnOnce() -> R) -> R {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    let result = f();
    pb.finish_with_message(format!("{msg} ✓"));
    result
}
```

- [ ] **Step 4: Run test to verify it still passes**

Run: `cargo test with_spinner_returns_value -- --nocapture`
Expected: PASS — with_spinner now uses indicatif but still returns the value.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add progress::with_spinner helper"
```

---

### Task 3: Add spinner to scanner

**Files:**
- Modify: `src/scanner.rs` — wrap detect calls with `with_spinner()`

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn scan_shows_progress_spinner() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["scan"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain spinner completion lines with ✓
    assert!(stdout.contains("Scanning"), "stdout: {stdout}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test scan_shows_progress_spinner -- --nocapture`
Expected: FAIL — scanner currently outputs the table directly without "Scanning" messages.

- [ ] **Step 3: Wrap detect() with with_spinner in scanner**

In `src/scanner.rs`, change:

```rust
let results: Vec<_> = cleaners.iter().map(|c| c.detect()).collect();
```

to:

```rust
let results: Vec<_> = cleaners
    .iter()
    .map(|c| {
        let name = c.name();
        crate::progress::with_spinner(&format!("Scanning {name}..."), || c.detect())
    })
    .collect();
```

Also add `mod progress;` import — since `crate::progress` resolves via main.rs `mod progress;`, no additional `use` is needed in scanner.rs.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test scan_shows_progress_spinner -- --nocapture`
Expected: PASS — stdout contains "Scanning uv... ✓" etc.

Run: `cargo test` (full suite)
Expected: ALL PASS — existing tests not affected.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add spinner to scan command"
```

---

### Task 4: Add spinner to interactive run_auto and run_interactive

**Files:**
- Modify: `src/interactive.rs`

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn yes_flag_shows_progress_spinner() {
    let tmp = TempDir::new().unwrap();

    // Create a minimal uv cache so there's something to clean
    let uv_cache = tmp.path().join(".cache/uv/archive-v0");
    fs::create_dir_all(&uv_cache).unwrap();
    fs::write(uv_cache.join("dummy"), b"x".repeat(1024)).unwrap();

    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    for tool in &["uv", "brew", "mise", "bun", "go", "pip", "npm", "yarn", "pnpm"] {
        fs::write(bin_dir.join(tool), "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::PermissionsExt::set_permissions(
            fs::Permissions::from_mode(0o755),  // won't compile — see note
        );
    }

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .arg("--yes")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Must start with version
    assert!(stdout.starts_with("sasurahime v0.1.2"), "stdout: {stdout}");
    // Must contain spinner messages
    assert!(stdout.contains("Cleaning"), "stdout: {stdout}");
}
```

**Note:** The permissions API call above is wrong — use the existing `install_fake_tool()` pattern from `tests/interactive.rs` instead. Write the test using the same fake-tool helper.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test yes_flag_shows_progress_spinner -- --nocapture`
Expected: FAIL — run_auto currently outputs clean results without spinner messages.

- [ ] **Step 3: Wrap clean() with with_spinner in run_auto**

In `src/interactive.rs`:

**a)** `run_auto` — wrap each `clean(false)` call:

```rust
for i in pruneable_indices {
    let name = cleaners[i].name();
    let result = crate::progress::with_spinner(
        &format!("Cleaning {}...", name),
        || cleaners[i].clean(false),
    );
    match result {
        Ok(r) => total_freed += r.bytes_freed,
        Err(e) => eprintln!("Error cleaning {}: {e}", cleaners[i].name()),
    }
}
```

**b)** `run_interactive` — wrap the clean loop after user confirmation:

```rust
for &si in &selected {
    let cleaner_idx = pruneable_indices[si];
    let name = cleaners[cleaner_idx].name();
    let result = crate::progress::with_spinner(
        &format!("Cleaning {}...", name),
        || cleaners[cleaner_idx].clean(false),
    );
    match result {
        Ok(r) => freed += r.bytes_freed,
        Err(e) => eprintln!("Error: {e}"),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test yes_flag_shows_progress_spinner -- --nocapture`
Expected: PASS — stdout contains "Cleaning uv... ✓" etc.

Run: `cargo test` (full suite)
Expected: ALL PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add spinner to --yes and interactive clean"
```

---

### Task 5: Add spinner to direct clean target arms

**Files:**
- Modify: `src/main.rs` — wrap each CleanTarget.clean() with spinner

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn clean_uv_subcommand_shows_spinner() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // Install fake uv that exits successfully
    let fake_uv = bin_dir.join("uv");
    fs::write(&fake_uv, "#!/bin/sh\nexit 0\n").unwrap();
    #[cfg(unix)]
    std::os::unix::fs::PermissionsExt::set_permissions(
        &fake_uv,
        std::fs::Permissions::from_mode(0o755),
    ).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "uv", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Cleaning"), "stdout: {stdout}");
    assert!(stdout.contains("uv"), "stdout: {stdout}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test clean_uv_subcommand_shows_spinner -- --nocapture`
Expected: FAIL — `sasurahime clean uv --dry-run` currently outputs directly without spinner.

- [ ] **Step 3: Wrap each CleanTarget arm with with_spinner**

In `src/main.rs`, for each `CleanTarget` variant, wrap the clean call:

```rust
CleanTarget::Uv { dry_run } => {
    let cleaner = Box::new(cleaners::uv::UvCleaner::new(&home, Box::new(SystemCommandRunner)));
    let result = crate::progress::with_spinner("Cleaning uv...", || cleaner.clean(dry_run))?;
    println!("Freed: {}", format::format_bytes(result.bytes_freed));
}
```

Apply the same pattern to all 14 targets. The message format is `"Cleaning {target_name}..."` where target_name matches the enum variant name (lowercase). Example mapping:

| Variant | Message |
|---------|---------|
| `Uv` | `"Cleaning uv..."` |
| `Brew` | `"Cleaning brew..."` |
| `Mise` | `"Cleaning mise..."` |
| `Browsers` | `"Cleaning browsers..."` |
| `Bun` | `"Cleaning bun..."` |
| `Go` | `"Cleaning go..."` |
| `Pip` | `"Cleaning pip..."` |
| `NodeGyp` | `"Cleaning node-gyp..."` |
| `Npm` | `"Cleaning npm..."` |
| `Yarn` | `"Cleaning yarn..."` |
| `Pnpm` | `"Cleaning pnpm..."` |
| `Caches` | `"Cleaning caches..."` |
| `Logs` | `"Cleaning logs..."` |
| `Xcode` | `"Cleaning xcode..."` |

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test clean_uv_subcommand_shows_spinner -- --nocapture`
Expected: PASS — stdout contains "Cleaning uv... ✓" etc.

Run: `cargo test` (full suite)
Expected: ALL PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: add spinner to direct clean subcommand targets"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run full quality gates**

```bash
cargo fmt --check
cargo clippy --tests -- -D warnings
cargo test
```

Expected: all pass, 0 warnings, 0 failures.

- [ ] **Step 2: Commit

```bash
git add -A
git commit -m "chore: final verification passes"
```
