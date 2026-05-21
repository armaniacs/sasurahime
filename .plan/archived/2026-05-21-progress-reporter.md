# Progress Reporter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add ProgressReporter trait with progress bar (ETA, per-file speed) for all multi-entry cleaners, plus `--suppress`/`--deep-suppress` CLI flags and config file support.

**Architecture:** New `ProgressReporter` trait in `src/progress.rs` with three implementations: `VerboseProgress` (indicatif ProgressBar), `SuppressReporter` (spinner only), `DeepSuppressReporter` (silent). `Cleaner` trait gains `&dyn ProgressReporter` parameter. All 9 multi-entry cleaners call `progress_init/tick/finish` in their delete loops. Config file and CLI flags control verbosity.

**Tech Stack:** Rust, indicatif (existing dep), serde/toml (existing), assert_cmd (existing dev-dep)

---

### Task 1: ProgressReporter trait, implementations, and factory

**Files:**
- Modify: `src/progress.rs` (add ~130 lines)
- Test: inline `#[cfg(test)]` in `src/progress.rs`

- [ ] **Step 1: Add `use` imports and the `ProgressReporter` trait**

Append to `src/progress.rs` after existing imports:

```rust
use std::path::Path;
use std::sync::Mutex;
use indicatif::{ProgressBar, ProgressStyle};

pub trait ProgressReporter: Send + Sync {
    fn show_spinner(&self) -> bool;
    fn progress_init(&self, label: &str, total: usize);
    fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64);
    fn progress_finish(&self);
}
```

- [ ] **Step 2: Run test to verify it compiles (no test yet, just cargo check)**

Run: `cargo check`
Expected: OK

- [ ] **Step 3: Write Test 1 — VerboseProgress exists and shows spinner**

Add to the existing `#[cfg(test)] mod tests { ... }` block in `src/progress.rs`:

```rust
use super::*;

#[test]
fn verbose_progress_shows_spinner() {
    let reporter = VerboseProgress::new();
    assert!(reporter.show_spinner());
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo test -p sasurahime -- progress::tests::verbose_progress_shows_spinner --nocapture`
Expected: `error[E0433]: failed to resolve: use of undeclared type 'VerboseProgress'`

- [ ] **Step 5: Implement VerboseProgress struct + impl**

Before the test module, add:

```rust
pub struct VerboseProgress {
    pb: Mutex<Option<ProgressBar>>,
}

impl VerboseProgress {
    pub fn new() -> Self {
        Self { pb: Mutex::new(None) }
    }
}

impl ProgressReporter for VerboseProgress {
    fn show_spinner(&self) -> bool { true }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _current: usize, _size_bytes: u64) {}
    fn progress_finish(&self) {}
}
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p sasurahime -- progress::tests::verbose_progress_shows_spinner --nocapture`
Expected: PASS

- [ ] **Step 7: Write Test 2 — DeepSuppressReporter hides spinner**

```rust
#[test]
fn deep_suppress_reporter_hides_spinner() {
    let reporter = DeepSuppressReporter;
    assert!(!reporter.show_spinner());
}
```

- [ ] **Step 8: Run test to verify it fails**

Expected: `error[E0433]: use of undeclared type 'DeepSuppressReporter'`

- [ ] **Step 9: Implement DeepSuppressReporter**

```rust
pub struct DeepSuppressReporter;

impl ProgressReporter for DeepSuppressReporter {
    fn show_spinner(&self) -> bool { false }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _current: usize, _size_bytes: u64) {}
    fn progress_finish(&self) {}
}
```

- [ ] **Step 10: Run tests to verify passes**

Run: `cargo test -p sasurahime -- progress::tests::deep_suppress_reporter_hides_spinner --nocapture`
Expected: PASS

- [ ] **Step 11: Write Test 3 — SuppressReporter shows spinner**

```rust
#[test]
fn suppress_reporter_shows_spinner() {
    let reporter = SuppressReporter;
    assert!(reporter.show_spinner());
}
```

- [ ] **Step 12: Implement SuppressReporter**

```rust
pub struct SuppressReporter;

impl ProgressReporter for SuppressReporter {
    fn show_spinner(&self) -> bool { true }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _current: usize, _size_bytes: u64) {}
    fn progress_finish(&self) {}
}
```

- [ ] **Step 13: Run all 3 tests**

Run: `cargo test -p sasurahime -- progress::tests --nocapture`
Expected: 3 PASS

- [ ] **Step 14: Write Test 4 — VerboseProgress lifecycle runs without panic**

```rust
#[test]
fn verbose_progress_lifecycle() {
    let reporter = VerboseProgress::new();
    let path = Path::new("/tmp/test.log");
    reporter.progress_init("test", 5);
    reporter.progress_tick(path, 1, 1024);
    reporter.progress_tick(path, 2, 2048);
    reporter.progress_finish();
    assert!(reporter.show_spinner());
}
```

- [ ] **Step 15: Run test**

Expected: PASS (methods are no-ops, but they compile and run)

- [ ] **Step 16: Write Tests 5–6 — Suppress/DeepSuppress lifecycle no-ops**

```rust
#[test]
fn suppress_reporter_progress_is_noop() {
    let reporter = SuppressReporter;
    reporter.progress_init("test", 5);
    reporter.progress_tick(Path::new("/x"), 1, 512);
    reporter.progress_finish();
}

#[test]
fn deep_suppress_reporter_progress_is_noop() {
    let reporter = DeepSuppressReporter;
    reporter.progress_init("test", 5);
    reporter.progress_tick(Path::new("/x"), 1, 512);
    reporter.progress_finish();
}
```

- [ ] **Step 17: Run all tests**

Run: `cargo test -p sasurahime -- progress::tests --nocapture`
Expected: 6 PASS

- [ ] **Step 18: Implement `progress_init` with ProgressBar creation for VerboseProgress**

Replace the `VerboseProgress::progress_init` no-op:

```rust
fn progress_init(&self, label: &str, total: usize) {
    let pb = ProgressBar::new(total as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] {bar:30.cyan/blue} {pos}/{len} ETA {eta}")
            .expect("valid indicatif template")
            .progress_chars("=> "),
    );
    pb.set_message(format!("Cleaning {label}..."));
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    *self.pb.lock().unwrap() = Some(pb);
}
```

- [ ] **Step 19: Implement `progress_tick`**

Replace the no-op:

```rust
fn progress_tick(&self, path: &Path, current: usize, _size_bytes: u64) {
    if let Some(ref pb) = *self.pb.lock().unwrap() {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        pb.set_message(name.to_string());
        pb.set_position(current as u64);
    }
}
```

- [ ] **Step 20: Implement `progress_finish`**

Replace the no-op:

```rust
fn progress_finish(&self) {
    if let Some(pb) = self.pb.lock().unwrap().take() {
        pb.finish_and_clear();
    }
}
```

- [ ] **Step 21: Run all progress tests**

Run: `cargo test -p sasurahime -- progress::tests --nocapture`
Expected: 6 PASS (lifecycle test exercises real ProgressBar now)

- [ ] **Step 22: Write Test 7 — build_reporter_from_flags**

```rust
#[test]
fn build_reporter_default_verbose() {
    let r = build_reporter_from_flags(false, false);
    assert!(r.show_spinner());
}

#[test]
fn build_reporter_deep_suppress_wins_over_suppress() {
    let r = build_reporter_from_flags(true, true);
    assert!(!r.show_spinner());
}

#[test]
fn build_reporter_suppress_shows_spinner() {
    let r = build_reporter_from_flags(true, false);
    assert!(r.show_spinner());
}
```

- [ ] **Step 23: Run test to verify it fails**

Expected: `error[E0425]: cannot find function 'build_reporter_from_flags'`

- [ ] **Step 24: Implement `build_reporter_from_flags`**

```rust
pub fn build_reporter_from_flags(suppress: bool, deep_suppress: bool) -> Box<dyn ProgressReporter> {
    if deep_suppress {
        Box::new(DeepSuppressReporter)
    } else if suppress {
        Box::new(SuppressReporter)
    } else {
        Box::new(VerboseProgress::new())
    }
}
```

- [ ] **Step 25: Run test**

Run: `cargo test -p sasurahime -- progress::tests --nocapture`
Expected: 9 PASS

- [ ] **Step 26: Commit**

```bash
git add src/progress.rs
git commit -m "feat: add ProgressReporter trait with VerboseProgress, suppress, deep-suppress impls"
```

---

### Task 2: Config file fields

**Files:**
- Modify: `src/config.rs`
- Test: inline `#[cfg(test)]` in `src/config.rs`

- [ ] **Step 1: Write Test 8 — config loads `suppress = true` from TOML**

In the existing `#[cfg(test)] mod tests { }` block in `src/config.rs`:

```rust
#[test]
fn config_loads_suppress_true() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("config.toml"), "suppress = true\n").unwrap();
    let cfg = Config::load(tmp.path()).unwrap();
    assert!(cfg.suppress, "suppress from config must be true");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p sasurahime -- config::tests --nocapture`
Expected: `error[E0609]: no field 'suppress' on type 'Config'`

- [ ] **Step 3: Add `suppress` + `deep_suppress` fields to RawConfig**

```rust
struct RawConfig {
    #[serde(default)]
    logs: LogsSection,
    trash_mode: Option<bool>,
    suppress: Option<bool>,
    deep_suppress: Option<bool>,
}
```

- [ ] **Step 4: Add fields to Config struct**

```rust
pub struct Config {
    pub logs_keep_days: u32,
    pub logs_extra_targets: Vec<ExtraLogTarget>,
    pub trash_mode: bool,
    pub suppress: bool,
    pub deep_suppress: bool,
}
```

- [ ] **Step 5: Update Default impl**

```rust
impl Default for Config {
    fn default() -> Self {
        Self {
            logs_keep_days: 7,
            logs_extra_targets: vec![],
            trash_mode: true,
            suppress: false,
            deep_suppress: false,
        }
    }
}
```

- [ ] **Step 6: Update Config::load()**

```rust
Ok(Self {
    logs_keep_days: raw.logs.keep_days.unwrap_or(7),
    logs_extra_targets: raw.logs.targets,
    trash_mode: raw.trash_mode.unwrap_or(true),
    suppress: raw.suppress.unwrap_or(false),
    deep_suppress: raw.deep_suppress.unwrap_or(false),
})
```

- [ ] **Step 7: Write Test 9 — config default suppress is false**

```rust
#[test]
fn config_default_suppress_is_false() {
    let cfg = Config::default();
    assert!(!cfg.suppress, "default suppress must be false");
}
```

- [ ] **Step 8: Write Test 10 — config loads deep_suppress = true**

```rust
#[test]
fn config_loads_deep_suppress_true() {
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(tmp.path().join("config.toml"), "deep_suppress = true\n").unwrap();
    let cfg = Config::load(tmp.path()).unwrap();
    assert!(cfg.deep_suppress, "deep_suppress from config must be true");
}
```

- [ ] **Step 9: Run all config tests**

Run: `cargo test -p sasurahime -- config::tests --nocapture`
Expected: Test 8+9+10 + all existing config tests PASS

- [ ] **Step 10: Commit**

```bash
git add src/config.rs
git commit -m "feat: add suppress/deep-suppress fields to config"
```

---

### Task 3: Cleaner trait + merge_flags function + main.rs wiring

**Files:**
- Modify: `src/cleaner.rs`, `src/progress.rs`, `src/main.rs`

- [ ] **Step 1: Update Cleaner trait**

In `src/cleaner.rs`:

```rust
use crate::progress::ProgressReporter;

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self) -> ScanResult;
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
}
```

- [ ] **Step 2: Write Test 11 — merge_suppress_flags**

In `src/progress.rs` test module, add `use crate::config::Config;` (or test the function directly). Since `merge_suppress_flags` lives in `progress.rs`:

```rust
#[test]
fn merge_flags_cli_suppress_overrides_config() {
    let (s, d) = merge_suppress_flags(true, false, false, false);
    assert!(s);
    assert!(!d);
}

#[test]
fn merge_flags_config_suppress_applied() {
    let (s, d) = merge_suppress_flags(false, false, true, false);
    assert!(s);
}

#[test]
fn merge_flags_deep_wins_over_suppress() {
    let (s, d) = merge_suppress_flags(true, true, false, false);
    assert!(d, "cli deep_suppress wins");
}
```

- [ ] **Step 3: Run test to verify it fails**

Expected: `error[E0425]: cannot find function 'merge_suppress_flags'`

- [ ] **Step 4: Implement merge_suppress_flags**

In `src/progress.rs`, after `build_reporter_from_flags`:

```rust
pub fn merge_suppress_flags(
    cli_suppress: bool,
    cli_deep_suppress: bool,
    cfg_suppress: bool,
    cfg_deep_suppress: bool,
) -> (bool, bool) {
    let suppress = cli_suppress || cfg_suppress;
    let deep_suppress = cli_deep_suppress || cfg_deep_suppress;
    if deep_suppress {
        (false, true)
    } else {
        (suppress, false)
    }
}
```

- [ ] **Step 5: Run test**

Run: `cargo test -p sasurahime -- progress::tests --nocapture`
Expected: PASS (12 tests)

- [ ] **Step 6: Add `#![allow(unused_imports)]` or re-export ProgressReporter if needed**

The `cleaner.rs` needs access to `ProgressReporter`. Add `use crate::progress::ProgressReporter;` at the top of `cleaner.rs`.

- [ ] **Step 7: Update all cleaner `clean()` signatures — mechanical change**

Every file in `src/cleaners/*.rs` that implements `Cleaner` needs its `fn clean(&self, dry_run: bool)` changed to `fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter)`.

Files to update (17 files):
- `src/cleaners/brew.rs`
- `src/cleaners/browser.rs`
- `src/cleaners/bun.rs`
- `src/cleaners/cargo.rs`
- `src/cleaners/cocoapods.rs`
- `src/cleaners/colima.rs`
- `src/cleaners/conda.rs`
- `src/cleaners/device_support.rs`
- `src/cleaners/generic.rs`
- `src/cleaners/go.rs`
- `src/cleaners/library_logs.rs`
- `src/cleaners/log.rs`
- `src/cleaners/mise.rs`
- `src/cleaners/ollama.rs`
- `src/cleaners/pip.rs`
- `src/cleaners/poetry.rs`
- `src/cleaners/uv.rs`
- `src/cleaners/xcode.rs`

And the `Cleaner` trait impl in `src/trash.rs` mock if applicable.

For each file, add `use crate::progress::ProgressReporter;` at the top, and change the method signature.

Example (brew.rs):
```rust
use crate::progress::ProgressReporter;
// ...
fn clean(&self, dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
```

- [ ] **Step 8: Run cargo check to verify all signatures compile**

Run: `cargo check 2>&1 | head -50`
Expected: No errors. Any remaining errors point to files still needing updates.

- [ ] **Step 9: Add `--suppress` / `--deep-suppress` CLI flags**

In `src/main.rs`, `Cli` struct:

```rust
struct Cli {
    #[arg(long)]
    yes: bool,
    #[arg(long)]
    permanent: bool,

    /// Suppress per-entry progress output (spinner only)
    #[arg(long)]
    suppress: bool,

    /// Suppress all output including spinner
    #[arg(long)]
    deep_suppress: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}
```

- [ ] **Step 10: Add `build_reporter` function in main.rs**

```rust
use crate::config::Config;
use crate::progress::{build_reporter_from_flags, merge_suppress_flags, ProgressReporter};

fn build_reporter(cli: &Cli, config: &Config) -> Box<dyn ProgressReporter> {
    let (suppress, deep_suppress) = merge_suppress_flags(
        cli.suppress,
        cli.deep_suppress,
        config.suppress,
        config.deep_suppress,
    );
    build_reporter_from_flags(suppress, deep_suppress)
}
```

- [ ] **Step 11: Update main() to build reporter and pass through --yes path**

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    eprintln!("sasurahime v{}", env!("CARGO_PKG_VERSION"));
    let home = home();
    let config_dir = home.join(".config/sasurahime");
    let config = match config::Config::load(&config_dir) { /* existing */ };
    let reporter = build_reporter(&cli, &config);
```

In the `--yes` path, pass `reporter.as_ref()` to `run_clean_target` calls:

```rust
if cli.yes {
    let targets = /* existing */;
    for target in targets {
        run_clean_target(label, |dry, r| cleaner.clean(dry, r), dry_run, reporter.as_ref())?;
        //                                                            ^^^^^^^^^^^^^^^^ NEW
    }
}
```

- [ ] **Step 12: Update `run_clean_target` signature**

```rust
fn run_clean_target<F>(
    label: &str,
    cleaner_fn: F,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
) -> Result<()>
where
    F: FnOnce(bool, &dyn ProgressReporter) -> Result<CleanResult>,
{
    let result = if reporter.show_spinner() {
        crate::progress::with_spinner(
            &format!("Cleaning {label}..."),
            || cleaner_fn(dry_run, reporter),
        )?
    } else {
        cleaner_fn(dry_run, reporter)?
    };

    if reporter.show_spinner() {
        if crate::trash::is_trash_mode() && result.bytes_freed > 0 {
            println!(
                "Freed: 0 B ({} moved to Trash)",
                crate::format::format_bytes(result.bytes_freed)
            );
        } else {
            println!("Freed: {}", crate::format::format_bytes(result.bytes_freed));
        }
    }
    Ok(())
}
```

- [ ] **Step 13: Update all callers of `run_clean_target` to pass reporter**

Find every `run_clean_target(label, fn, dry_run)` call in `src/main.rs` and add the reporter parameter (available as `reporter.as_ref()` or `reporter: &dyn ProgressReporter` depending on context).

- [ ] **Step 14: Run cargo check**

Run: `cargo check`
Expected: No errors

- [ ] **Step 15: Commit**

```bash
git add src/cleaner.rs src/progress.rs src/main.rs src/cleaners/
git commit -m "feat: update Cleaner trait, add CLI flags, wire reporter through main"
```

---

### Task 4: Multi-entry cleaner progress bar integration

**Files:**
- Modify: 9 cleaner files

For each cleaner, the pattern is:
1. Add `use crate::progress::ProgressReporter;` (if not already added)
2. In `clean()` / `clean_all()`, after scanning entries, call `reporter.progress_init(self.name(), entries.len())`
3. In the delete loop, call `reporter.progress_tick(&entry.path, i + 1)` before each deletion
4. After the loop, call `reporter.progress_finish()`

#### 4a. LibraryLogs — clean() and clean_all()

`src/cleaners/library_logs.rs`:

In `clean()`:
```rust
let selected = self.interactive_select(&entries)?;
// ... existing empty check ...
reporter.progress_init(self.name(), selected.len());
for (i, entry) in selected.iter().enumerate() {
    reporter.progress_tick(&entry.path, i + 1, entry.size);
    // ... existing chflags + delete_path ...
}
```

#### 4b. LogCleaner

`src/cleaners/log.rs`:

In `clean()`, aggregate total files first, then track across all targets:

```rust
fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
    let mut all_old: Vec<(String, PathBuf)> = Vec::new();
    for target in self.all_targets() {
        for path in Self::find_old_logs(target.path, self.keep_days, target.exclude) {
            all_old.push((target.name.to_string(), path));
        }
    }
    let mut freed: u64 = 0;
    let mut deleted: u32 = 0;

    if dry_run {
        for (target_name, path) in &all_old {
            println!("[dry-run] [{target_name}] would remove: {}", path.display());
        }
        return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
    }

    reporter.progress_init(self.name(), all_old.len());
    for (i, (target_name, path)) in all_old.iter().enumerate() {
        let size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        reporter.progress_tick(path, i + 1, size);
        crate::trash::delete_path(path)?;
        freed += size;
        deleted += 1;
        println!("[{target_name}] Removed: {}", path.display());
    }
    reporter.progress_finish();

    println!("Removed {deleted} log files");
    Ok(CleanResult { name: self.name(), bytes_freed: freed })
}
```

Note: dry-run keeps the per-entry listing unchanged per spec.

#### 4c–4i. Remaining 7 cleaners

For each of these, the pattern is identical — find the deletion loop in `clean()`, add the three progress calls:

- `generic.rs`: DeleteDirs loop
- `browser.rs`: old versions loop
- `xcode.rs`: device directory loop
- `cargo.rs`: cache files loop
- `device_support.rs`: device support dirs loop
- `ollama.rs`: model dirs loop
- `mise.rs`: unused versions loop

For each file:
```rust
reporter.progress_init(self.name(), entries.len());
for (i, entry) in entries.iter().enumerate() {
    reporter.progress_tick(&entry.path, i + 1, entry.size);
    // ... existing deletion ...
}
reporter.progress_finish();
```

- [ ] **Step 1–9: Implement each of the 9 cleaners**

For each one, run `cargo check` after each to catch compilation errors early.

- [ ] **Step 10: Run full test suite**

Run: `cargo test`
Expected: All tests pass

- [ ] **Step 11: Commit**

```bash
git add src/cleaners/
git commit -m "feat: add progress bar to all multi-entry cleaners"
```

---

### Task 5: E2E tests for suppress/deep-suppress

**Files:**
- Create: `tests/progress.rs`

- [ ] **Step 1: Write Test 12 — suppress flag hides progress bar artifacts**

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd.env("PATH", "/usr/bin:/bin");
    cmd
}

fn create_large_log(home: &std::path::Path) {
    let dir = home.join("Library/Logs/BloatApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash.log"), vec![0u8; (100 * 1024 * 1024) + 1]).unwrap();
}

#[test]
fn suppress_flag_hides_progress_bar() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all", "--suppress"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // --suppress should not show ETA (progress bar artifact)
    assert!(!stdout.contains("ETA"), "--suppress should hide ETA:\n{stdout}");
    // Should still show Freed line
    assert!(stdout.contains("Freed:"), "--suppress should show Freed:\n{stdout}");
}
```

- [ ] **Step 2: Run to verify it fails (or passes if --suppress works)**

Run: `cargo test --test progress -- suppress_flag_hides_progress_bar --nocapture`
Expected outcome depends on whether Task 3 wiring is complete. If the overall binary has the flag wired, PASS. If not, fix wiring first.

- [ ] **Step 3: Write Test 13 — deep-suppress hides all stdout**

```rust
#[test]
fn deep_suppress_flag_hides_all_stdout() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all", "--deep-suppress"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "deep-suppress should produce no stdout:\n{stdout}");
}
```

- [ ] **Step 4: Run test**

Run: `cargo test --test progress -- deep_suppress_flag_hides_all_stdout --nocapture`
Expected: PASS

- [ ] **Step 5: Write Test 14 — default shows progress bar (Freed line presence)**

```rust
#[test]
fn default_shows_freed_line() {
    let tmp = TempDir::new().unwrap();
    create_large_log(tmp.path());

    let output = sasurahime(tmp.path())
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Freed:"), "default should show Freed:\n{stdout}");
}
```

- [ ] **Step 6: Run all progress E2E tests**

Run: `cargo test --test progress -- --nocapture`
Expected: 3 PASS

- [ ] **Step 7: Commit**

```bash
git add tests/progress.rs
git commit -m "test: add E2E tests for suppress/deep-suppress CLI flags"
```

---

### Task 6: Update existing tests for new clean() signature

**Files:**
- Modify: `tests/*.rs` (all files that call `cleaner.clean(dry_run)`)

Every E2E test that calls a cleaner directly (e.g., in mock-based tests) needs a reporter:

```rust
// Before:
cleaner.clean(true).unwrap();

// After:
let reporter = crate::progress::VerboseProgress::new();
cleaner.clean(true, &reporter).unwrap();
```

For black-box tests that invoke the binary (most `tests/*.rs`), no change needed — the binary handles reporter creation internally.

- [ ] **Step 1–N: Find and fix all direct `clean()` calls in test files**

```bash
# Find candidates
grep -rn '\.clean(' tests/ --include='*.rs'
```

For each match, add the reporter parameter.

- [ ] **Step 2: Run full test suite**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 3: Commit**

```bash
git add tests/
git commit -m "test: update clean() calls to pass ProgressReporter"
```

---

### Task 7: Implement per-file speed in VerboseProgress

**Files:**
- Modify: `src/progress.rs`

The `progress_tick` already accepts `size_bytes`. Now we need to track time between ticks to calculate speed.

- [ ] **Step 1: Add timing field to VerboseProgress**

```rust
use std::time::Instant;

pub struct VerboseProgress {
    pb: Mutex<Option<ProgressBar>>,
    last_tick: Mutex<Option<Instant>>,
}

impl VerboseProgress {
    pub fn new() -> Self {
        Self {
            pb: Mutex::new(None),
            last_tick: Mutex::new(None),
        }
    }
}
```

- [ ] **Step 2: Implement speed calculation in `progress_tick`**

```rust
fn progress_tick(&self, path: &Path, current: usize, size_bytes: u64) {
    if let Some(ref pb) = *self.pb.lock().unwrap() {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let speed_str = self.last_tick.lock().unwrap()
            .and_then(|start| {
                let elapsed = start.elapsed().ok()?;
                let secs = elapsed.as_secs_f64().max(0.001);
                let mb = size_bytes as f64 / 1_048_576.0;
                Some(format!(", {:.1} MB/s", mb / secs))
            })
            .unwrap_or_default();
        *self.last_tick.lock().unwrap() = Some(Instant::now());
        pb.set_message(format!("{name}{speed_str} ({}/{})", current, pb.length().unwrap_or(0)));
        pb.set_position(current as u64);
    }
}
```

- [ ] **Step 3: Run full test suite**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 4: Commit**

```bash
git add src/progress.rs
git commit -m "feat: add per-file speed to progress bar"
```

---

### Verification

After all tasks:

```bash
# Full test suite
cargo test

# Binary builds
cargo build --release

# Quick smoke test: help shows new flags
cargo run -- --help 2>&1 | grep -A1 suppress
cargo run -- --help 2>&1 | grep -A1 deep-suppress

# Quick smoke test: scan still works
cargo run -- scan 2>&1
```
