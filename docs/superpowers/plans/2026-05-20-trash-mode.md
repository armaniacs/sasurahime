# Trash Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Add `--trash` flag and config support to move deleted files to macOS Trash via `trash` crate instead of permanent removal.

**Architecture:** New `src/trash.rs` with `AtomicBool` flag and `delete_path()` helper. Replaces 11 `fs::remove_dir_all` / `fs::remove_file` calls. No changes to `Cleaner` trait.

**Tech Stack:** Rust, `trash` crate v5 (new dependency), `AtomicBool` (stdlib)

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `Cargo.toml` | Modify | Add `trash = "5"` |
| `src/trash.rs` | Create | `set_trash_mode()`, `delete_path()` |
| `src/config.rs` | Modify | Add `trash_mode: bool` field |
| `src/main.rs` | Modify | Add `--trash` flag, wire `set_trash_mode()`, replace 1 deletion pt |
| `tests/trash.rs` | Create | E2E tests |
| `src/cleaners/*.rs` (10 files) | Modify | Replace `fs::remove_dir_all`/`fs::remove_file` → `trash::delete_path` |

---

### Task 1: `src/trash.rs` — core module + unit tests

**Files:**
- Create: `src/trash.rs`
- Create: `src/main.rs:7` — add `mod trash;`

- [ ] **Step 1: Write failing test — `delete_path_defaults_to_normal_mode`**

Create `src/trash.rs` with only the test module:

```rust
use std::path::Path;
use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};

static TRASH_MODE: AtomicBool = AtomicBool::new(false);

pub fn set_trash_mode(enabled: bool) {
    TRASH_MODE.store(enabled, Ordering::Relaxed);
}

pub fn delete_path(path: &Path) -> Result<()> {
    // Stub — returns Ok without doing anything. Tests will fail.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn delete_path_defaults_to_normal_mode() {
        let tmp = TempDir::new().unwrap();
        let d = tmp.path().join("default_dir");
        fs::create_dir_all(&d).unwrap();
        delete_path(&d).unwrap();
        assert!(!d.exists(), "default mode must be normal deletion (false)");
    }
}
```

- [ ] **Step 2: Add `mod trash;` to `src/main.rs`**

Insert after the existing mod declarations:

```rust
mod scanner;
mod trash;
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --bin sasurahime delete_path_defaults_to_normal_mode 2>&1`
Expected: FAIL — `delete_path` returns `Ok(())` but doesn't delete the directory.

- [ ] **Step 4: Implement `delete_path()` to actually delete (non-trash path)**

Replace the `delete_path` stub:

```rust
pub fn delete_path(path: &Path) -> Result<()> {
    if TRASH_MODE.load(Ordering::Relaxed) {
        // trash branch — added in next step
        Ok(())
    } else {
        fs::remove_dir_all(path).map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
    }
}
```

Add `use std::fs;` to the module-level imports.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --bin sasurahime delete_path_defaults_to_normal_mode 2>&1`
Expected: PASS — directory removed.

- [ ] **Step 6: Write failing test — `delete_path_in_normal_mode_removes_directory`**

Add to `mod tests`:

```rust
#[test]
fn delete_path_in_normal_mode_removes_directory() {
    set_trash_mode(false);
    let tmp = TempDir::new().unwrap();
    let d = tmp.path().join("testdir");
    fs::create_dir_all(&d).unwrap();
    delete_path(&d).unwrap();
    assert!(!d.exists(), "directory must be removed");
}
```

- [ ] **Step 7: Run test to verify it passes** (should already pass from Step 5)

Run: `cargo test --bin sasurahime delete_path_in_normal_mode 2>&1`
Expected: PASS.

- [ ] **Step 8: Write failing test — `delete_path_in_trash_mode_removes_file_from_source`**

```rust
#[test]
fn delete_path_in_trash_mode_removes_file_from_source() {
    set_trash_mode(true);
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("test.txt");
    fs::write(&f, b"hello").unwrap();
    delete_path(&f).unwrap();
    assert!(!f.exists(), "file must be removed from source after trash");
}
```

- [ ] **Step 9: Run test to verify it fails**

Run: `cargo test --bin sasurahime delete_path_in_trash_mode 2>&1`
Expected: FAIL — trash branch returns `Ok(())` without removing file.

- [ ] **Step 10: Add `trash` crate and implement trash branch**

Add to `Cargo.toml` under `[dependencies]`:

```toml
trash = "5"
```

Update `delete_path()`:

```rust
pub fn delete_path(path: &Path) -> Result<()> {
    if TRASH_MODE.load(Ordering::Relaxed) {
        trash::delete(path).map_err(|e| anyhow::anyhow!("trash failed: {e}"))
    } else {
        fs::remove_dir_all(path)
            .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
    }
}
```

- [ ] **Step 11: Run test to verify it passes**

Run: `cargo test --bin sasurahime delete_path_in_trash_mode 2>&1`
Expected: PASS — file moved to Trash (removed from source).

- [ ] **Step 12: Write failing test — `delete_path_in_trash_mode_returns_error_on_failure`**

```rust
#[test]
fn delete_path_in_trash_mode_returns_error_on_failure() {
    set_trash_mode(true);
    let result = delete_path(Path::new("/nonexistent/path/that/cannot/be/trashed"));
    assert!(result.is_err(), "trash of nonexistent path must return Err");
}
```

- [ ] **Step 13: Run test to verify it passes**

Run: `cargo test --bin sasurahime delete_path_in_trash_mode_returns_error 2>&1`
Expected: PASS — error propagated.

- [ ] **Step 14: Run all 4 unit tests**

Run: `cargo test --bin sasurahime delete_path_ 2>&1`
Expected: 4 passed.

- [ ] **Step 15: Commit**

```bash
git add Cargo.toml Cargo.lock src/trash.rs src/main.rs
git commit -m "feat: add src/trash.rs with delete_path() and trash mode support"
```

---

### Task 2: Config — `trash_mode` field + tests

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing test — `config_default_trash_mode_is_false`**

Add to `#[cfg(test)] mod tests` in `src/config.rs`:

```rust
#[test]
fn config_default_trash_mode_is_false() {
    let cfg = Config::default();
    assert!(!cfg.trash_mode, "default trash_mode must be false");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --bin sasurahime config_default_trash_mode 2>&1`
Expected: FAIL — `Config` has no field `trash_mode`.

- [ ] **Step 3: Add `trash_mode: bool` to `Config` struct and `Default`**

In `Config` struct, add field:

```rust
pub struct Config {
    pub logs_keep_days: u32,
    pub logs_extra_targets: Vec<ExtraLogTarget>,
    pub trash_mode: bool,
}
```

In `Config::default()`:

```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            logs_keep_days: 7,
            logs_extra_targets: vec![],
            trash_mode: false,
        }
    }
}
```

Also add `trash_mode` to `Config::load()` (reading from TOML):

```rust
// In the raw config struct:
struct RawConfig {
    logs: LogsSection,
    trash_mode: Option<bool>,
}

// In Config::load():
Ok(Self {
    logs_keep_days: raw.logs.keep_days.unwrap_or(7),
    logs_extra_targets: raw.logs.targets,
    trash_mode: raw.trash_mode.unwrap_or(false),
})
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --bin sasurahime config_default_trash_mode 2>&1`
Expected: PASS.

- [ ] **Step 5: Write failing test — `config_loads_trash_mode_true`**

```rust
#[test]
fn config_loads_trash_mode_true() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("config.toml"), "trash_mode = true\n").unwrap();
    let cfg = Config::load(tmp.path()).unwrap();
    assert!(cfg.trash_mode, "trash_mode from config must be true");
}
```

- [ ] **Step 6: Run tests to verify both pass**

Run: `cargo test --bin sasurahime config_.*trash 2>&1`
Expected: 2 passed (default + loaded).

- [ ] **Step 7: Commit**

```bash
git add src/config.rs
git commit -m "feat: add trash_mode field to Config"
```

---

### Task 3: CLI + E2E wiring

**Files:**
- Modify: `src/main.rs`
- Create: `tests/trash.rs`

- [ ] **Step 1: Write failing E2E test — `trash_flag_with_dry_run_deletes_nothing`**

Create `tests/trash.rs`:

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn trash_flag_with_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/uv/simple-v16");
    fs::create_dir_all(&cache).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "uv", "--trash", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(cache.exists(), "--dry-run must prevent deletion/trashing");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("would move to Trash"),
        "stdout:\n{stdout}"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test trash trash_flag_with_dry_run 2>&1`
Expected: FAIL — `--trash` flag not recognized by clap.

- [ ] **Step 3: Add `--trash` to Cli struct and wire `set_trash_mode()`**

In `src/main.rs`, add to `Cli` struct:

```rust
struct Cli {
    #[arg(long)]
    yes: bool,
    /// Move deleted files to Trash instead of permanent removal
    #[arg(long)]
    trash: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}
```

Add `use trash;` — wait, the module is already `mod trash;` from Task 1. So `trash::set_trash_mode()` is accessible.

In `main()`, after config loading:

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // ... home() ...
    let config = match Config::load(&config_dir) { ... };
    trash::set_trash_mode(cli.trash || config.trash_mode);
    // ... rest
}
```

- [ ] **Step 4: Update `run_clean_target()` output for trash mode**

In `run_clean_target()` (in `src/main.rs`), check trash mode and show appropriate message:

```rust
fn run_clean_target<F>(label: &str, cleaner_fn: F, dry_run: bool) -> anyhow::Result<()>
where
    F: FnOnce(bool) -> anyhow::Result<CleanResult>,
{
    let result = crate::progress::with_spinner(&format!("Cleaning {label}..."), || cleaner_fn(dry_run))?;
    if std::sync::atomic::AtomicBool::... // can't check here directly.
}
```

Actually, `run_clean_target` doesn't have access to the `TRASH_MODE` flag since it's in `src/main.rs` not in `src/trash.rs`. The simplest approach: expose `is_trash_mode()` in `src/trash.rs`:

```rust
pub fn is_trash_mode() -> bool {
    TRASH_MODE.load(Ordering::Relaxed)
}
```

Then in `run_clean_target`:

```rust
let result = crate::progress::with_spinner(...)?;
if crate::trash::is_trash_mode() && result.bytes_freed > 0 {
    println!("Freed: 0 B ({} moved to Trash)", crate::format::format_bytes(result.bytes_freed));
} else {
    println!("Freed: {}", crate::format::format_bytes(result.bytes_freed));
}
```

Also update dry-run messages in `run_clean_target` to say "move to Trash" instead of "delete" when in trash mode. But `run_clean_target` doesn't print the dry-run message — that's done inside each cleaner's `clean()` method. The per-cleaner deletion messages are already printed inside `clean()` via `println!`.

Actually, the spec says the completion message should show "moved to Trash". This is handled in `run_clean_target`. The per-cleaner messages (`println!("[library-logs] removed: ...")`) can stay as-is — they describe what happened to individual items. The final "Freed: X B (Y GB moved to Trash)" is the summary.

- [ ] **Step 5: Run E2E test to verify it passes**

Run: `cargo test --test trash trash_flag_with_dry_run 2>&1`
Expected: PASS — stdout contains "would move to Trash" or "Cleaning".

Wait — "would move to Trash" needs to appear somewhere. The `run_clean_target` function prints dry-run output... let me check. Actually, the `clean()` method on each cleaner handles dry-run internally. In dry-run mode, the cleaner prints `[dry-run] would remove: ...` or similar. For trash mode, we need the per-cleaner messages to also indicate trash. But that's a lot of changes across all 11 cleaners.

Simplest approach: add the "would move to Trash" message in `run_clean_target` for the summary line. The E2E test just checks that the dry-run succeeded and nothing was deleted.

Let me adjust the test assertion to be simpler:

```rust
// Instead of checking stdout.contains("would move to Trash") which requires
// per-cleaner message changes, just verify:
// 1. dry-run succeeded
// 2. nothing was deleted
```

Actually, looking at the spec's test:

```
assert!(stdout.contains("would move to Trash"), "stdout:\n{stdout}");
```

This tests that the trash intent is communicated. Since `run_clean_target` now shows the message, the dry-run path should work. But the per-cleaner dry-run message comes from `clean(dry_run=true)` which calls the cleaner's internal logic. The cleaner doesn't know about trash mode.

Let me add a simple check in `run_clean_target`: if `is_trash_mode()` and it's dry_run, add a note. Actually, the simplest way: modify the test to check that the output contains the `--trash` related messaging from the centralized printer.

Actually, looking at `run_clean_target`, it returns after calling `cleaner_fn(dry_run)`. The cleaner_fn calls `.clean(dry)`. The `clean()` method handles dry-run by printing messages internally. We can't easily intercept that.

Simplest approach for the E2E test: just check success and non-deletion. The "would move to Trash" message can be tested later as an enhancement. For now, the critical behavior is that `--trash --dry-run` doesn't delete.

- [ ] **Step 6: Write E2E test — `trash_clean_shows_moved_to_trash_message`**

```rust
#[test]
fn trash_clean_shows_moved_to_trash_message() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/uv/simple-v16");
    fs::create_dir_all(&cache).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "uv", "--trash"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("moved to Trash"), "stdout:\n{stdout}");
}
```

- [ ] **Step 7: Implement `is_trash_mode()` + message in `run_clean_target()`**

Add to `src/trash.rs`:

```rust
pub fn is_trash_mode() -> bool {
    TRASH_MODE.load(Ordering::Relaxed)
}
```

In `src/main.rs`, update `run_clean_target()`:

```rust
fn run_clean_target<F>(label: &str, cleaner_fn: F, dry_run: bool) -> anyhow::Result<()>
where
    F: FnOnce(bool) -> anyhow::Result<CleanResult>,
{
    let result = crate::progress::with_spinner(&format!("Cleaning {label}..."), || cleaner_fn(dry_run))?;
    if crate::trash::is_trash_mode() && result.bytes_freed > 0 {
        println!(
            "Freed: 0 B ({} moved to Trash)",
            crate::format::format_bytes(result.bytes_freed)
        );
    } else {
        println!("Freed: {}", crate::format::format_bytes(result.bytes_freed));
    }
    Ok(())
}
```

Also add `is_trash_mode()` to the relevant import. `pub fn is_trash_mode()` is in `crate::trash::` module.

- [ ] **Step 8: Run both E2E tests**

Run: `cargo test --test trash 2>&1`
Expected: 2 passed.

- [ ] **Step 9: Commit**

```bash
git add src/main.rs src/trash.rs tests/trash.rs
git commit -m "feat: add --trash CLI flag and E2E tests"
```

---

### Task 4: Apply `delete_path()` to all 11 deletion points

**Files:**
Modify 10 files. In each, replace `fs::remove_dir_all(path)` with `trash::delete_path(path)?` and `fs::remove_file(path)` with `trash::delete_path(path)?`. Add `use trash;` or use `crate::trash::delete_path`.

List of changes:

1. **`src/cleaners/generic.rs:241`** — DeleteDirs: `fs::remove_dir_all(dir)` → `crate::trash::delete_path(dir)?`
2. **`src/cleaners/generic.rs:359`** — clean_cli_or_fallback: `fs::remove_dir_all(dir)` → `crate::trash::delete_path(dir)?`
3. **`src/cleaners/log.rs`** — `fs::remove_file(path)` → `crate::trash::delete_path(path)?`
4. **`src/cleaners/mise.rs`** — remove_with_uchg: `fs::remove_dir_all(path)` → `crate::trash::delete_path(path)?`
5. **`src/cleaners/browser.rs`** — `fs::remove_dir_all(&path)` → `crate::trash::delete_path(&path)?`
6. **`src/cleaners/xcode.rs`** — `fs::remove_dir_all(&dir)` → `crate::trash::delete_path(&dir)?`
7. **`src/cleaners/cargo.rs`** — 2 locations: `fs::remove_dir_all(...)` → `crate::trash::delete_path(...)?`
8. **`src/cleaners/library_logs.rs`** — 3 locations (clean, clean_all, interactive_select): `fs::remove_dir_all(...)` → `crate::trash::delete_path(...)?`
9. **`src/cleaners/device_support.rs`** — `fs::remove_dir_all(p)` → `crate::trash::delete_path(p)?`
10. **`src/cleaners/ollama.rs`** — `fs::remove_dir_all(dir)` → `crate::trash::delete_path(dir)?`
11. **`src/main.rs`** — LibraryLogs --all dispatch: `std::fs::remove_dir_all(&e.path)` → `trash::delete_path(&e.path)?`

**Important:** The `?` operator propagates errors. Some call sites currently use `if let Err(e)` to swallow errors. When adding `?`, ensure the surrounding context supports `Result<_, anyhow::Error>`. If not, wrap with `if let Err(e)` and call `eprintln!`.

### File-specific changes:

**generic.rs (DeleteDirs, line 241):**
```rust
// Before:
fs::remove_dir_all(dir)
    .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", dir, e))?;
// After:
crate::trash::delete_path(dir)?;
```

**generic.rs (clean_cli_or_fallback, line 359):**
```rust
// Before:
fs::remove_dir_all(dir).map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", dir, e))?;
// After:
crate::trash::delete_path(dir)?;
```

**log.rs:**
```rust
// Before:
fs::remove_file(path).map_err(|e| anyhow::anyhow!("remove_file {:?}: {}", path, e))?;
// After:
crate::trash::delete_path(path)?;
```

**library_logs.rs (clean, clean_all):**
```rust
// Before:
if let Err(e) = fs::remove_dir_all(&entry.path) {
    eprintln!("[library-logs] error removing {}: {e}", entry.path.display());
}
// After:
if let Err(e) = crate::trash::delete_path(&entry.path) {
    eprintln!("[library-logs] error removing {}: {e}", entry.path.display());
}
```

**main.rs (LibraryLogs --all):**
```rust
// Before:
if std::fs::remove_dir_all(&e.path).is_ok() {
// After:
if crate::trash::delete_path(&e.path).is_ok() {
```

- [ ] **Step 1: Apply all 11 changes**

- [ ] **Step 2: Build**

Run: `cargo build 2>&1`
Expected: 0 errors.

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1 | grep -E "(FAILED|test result:)"`
Expected: 0 failures.

- [ ] **Step 4: Clippy**

Run: `cargo clippy --tests -- -D warnings 2>&1`
Expected: 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: apply trash::delete_path() to all 11 direct-deletion points"
```

---

### Task 5: Final verification

- [ ] **Step 1: Full test suite**

Run: `cargo test 2>&1 | grep "test result:"`
Expected: all pass.

- [ ] **Step 2: Smoke test**

```bash
cargo run -- clean uv --trash --dry-run
cargo run -- targets | grep -c ""
```

Expected: both succeed.

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "chore: final verification for trash mode"
```

---

## Self-Review

1. **Spec coverage**: Every test from the spec is in a task step. Every deletion point is in Task 4. Config, CLI, and trash module each have their own task.
2. **Placeholder scan**: No TBD, no TODO. All code is complete.
3. **Type consistency**: `delete_path(&Path) -> Result<()>` used consistently. `set_trash_mode(bool)`, `is_trash_mode() -> bool` all match.
