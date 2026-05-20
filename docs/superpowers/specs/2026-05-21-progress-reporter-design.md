# Progress Reporter Design

**Date:** 2026-05-21
**Status:** Approved for implementation

## Motivation

The `library-logs` cleaner iterates through `~/Library/Logs/` entries, running `chflags -R nouchg` followed by `trash::delete_path()` on each. In environments with many entries this takes a long time with no per-entry progress feedback — the user only sees a static `Cleaning library-logs...` spinner. The same problem exists for any cleaner that processes multiple entries sequentially (DeleteDirs, browser cleaners, etc.).

## Scope

Add a `ProgressReporter` trait that manages an indicatif-based progress bar with ETA for multi-entry cleaners, integrate it into `run_clean_target` and the `Cleaner` trait, and add `--suppress` / `--deep-suppress` CLI flags to control output verbosity.

**Applies to**: All cleaners (trait signature change).
**Multi-entry cleaners with progress bar**: LibraryLogs, LogCleaner, GenericCacheCleaner (DeleteDirs), BrowserCleaner, XcodeCleaner, CargoCleaner, DeviceSupportCleaner, OllamaCleaner, MiseCleaner — all get `progress_init()`/`progress_tick()`/`progress_finish()` calls in their delete loops.
**Single-command cleaners** (Uv, Brew, CocoaPods, Conda, Poetry, Go, Pip, Bun, Colima, Simulator): accept the reporter parameter but do not call progress methods (no iteration to report).

## Design

### 1. ProgressReporter Trait

**File:** `src/progress.rs`

```rust
use std::path::Path;
use indicatif::ProgressBar;

pub trait ProgressReporter: Send + Sync {
    /// Whether to show any progress indication (spinner or progress bar)
    fn show_spinner(&self) -> bool;

    /// Initialize a progress bar for N total items. Called by multi-entry cleaners.
    fn progress_init(&self, label: &str, total: usize);

    /// Advance progress by one item. Called after each entry is processed.
    fn progress_tick(&self, path: &Path, current: usize);

    /// Finish the progress bar. Called after all entries processed.
    fn progress_finish(&self);
}
```

### 2. Three Implementations

```rust
/// Verbose: indicatif ProgressBar with elapsed/ETA display
pub struct VerboseProgress {
    pb: std::sync::Mutex<Option<ProgressBar>>,
}

/// Suppress: spinner only, no per-entry progress
pub struct SuppressReporter;

/// Deep-suppress: no output at all
pub struct DeepSuppressReporter;
```

| Implementation | `show_spinner()` | `progress_init()` | `progress_tick()` | `progress_finish()` |
|----------------|:---:|:---:|:---:|:---:|
| `VerboseProgress` | `true` | Creates `ProgressBar::new(total)` with ETA template | Increments bar, sets message to path name | Finishes & clears bar, prints `[OK]` |
| `SuppressReporter` | `true` | No-op | No-op | No-op |
| `DeepSuppressReporter` | `false` | No-op | No-op | No-op |

**Verbose progress bar style** (uses existing `spinner_style` pattern):

```rust
fn progress_style() -> indicatif::ProgressStyle {
    ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] {bar:30.cyan/blue} {pos}/{len} ETA {eta}")
        .expect("valid indicatif template")
        .progress_chars("=> ")
}
```

Displayed as: `⠋ [00:00:05] ████████████████░░░░░░  15/23 ETA 00:00:03`

The cleaner's current operation (path name) is shown in the message field.

**Suppress/DeepSuppress:** No progress bar, no output.

**Factory helpers:**

```rust
impl VerboseProgress {
    pub fn new() -> Self {
        Self { pb: std::sync::Mutex::new(None) }
    }
}

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

### 3. CLI Flags

**`src/main.rs`** — `Cli` struct:

```rust
#[arg(long, help = "Suppress per-entry progress output")]
suppress: bool,

#[arg(long, help = "Suppress all output including spinner")]
deep_suppress: bool,
```

Mutually exclusive. If both specified, `deep_suppress` wins.

**Factory function (delegates to testable helper):**

```rust
fn build_reporter(cli: &Cli, config: &Config) -> Box<dyn ProgressReporter> {
    build_reporter_from_flags(
        cli.suppress || config.suppress,
        cli.deep_suppress || config.deep_suppress,
    )
}
```

### 4. Config File Support

**`src/config.rs`** — `RawConfig` gets two new optional fields:

```rust
struct RawConfig {
    #[serde(default)]
    logs: LogsSection,
    trash_mode: Option<bool>,
    suppress: Option<bool>,       // NEW
    deep_suppress: Option<bool>,  // NEW
}
```

**`Config` struct:**

```rust
pub struct Config {
    pub logs_keep_days: u32,
    pub logs_extra_targets: Vec<ExtraLogTarget>,
    pub trash_mode: bool,
    pub suppress: bool,       // NEW
    pub deep_suppress: bool,  // NEW
}
```

**Default:** Both `false` (verbose mode).

**Loading in `Config::load()`:**

```rust
Ok(Self {
    logs_keep_days: raw.logs.keep_days.unwrap_or(7),
    logs_extra_targets: raw.logs.targets,
    trash_mode: raw.trash_mode.unwrap_or(true),
    suppress: raw.suppress.unwrap_or(false),           // NEW
    deep_suppress: raw.deep_suppress.unwrap_or(false),  // NEW
})
```

**Example config.toml:**

```toml
trash_mode = true
suppress = true
# deep_suppress = true   # overrides suppress if set
```

### 5. Priority (CLI > Config > Default)

```
CLI --deep-suppress   >  config.deep_suppress = true   >  default (false)
CLI --suppress        >  config.suppress = true         >  default (false)
```

`deep_suppress` always wins over `suppress` regardless of source.

**Startup logic (`main()`):**

```rust
let reporter = build_reporter(&cli, &config);
```

### 6. `run_clean_target` Changes

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
    // For verbose mode, the progress bar replaces the spinner entirely.
    // For suppress mode, the old spinner is shown.
    // For deep-suppress, nothing at all.
    let result = if reporter.show_spinner() {
        crate::progress::with_spinner(
            &format!("Cleaning {label}..."),
            || cleaner_fn(dry_run, reporter),
        )?
    } else {
        cleaner_fn(dry_run, reporter)?
    };

    // "Freed:" line: skipped for deep-suppress
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

Note: The Verbose reporter's `progress_init()` creates a new ProgressBar that visually replaces the spinner. The `with_spinner()` creates a spinner that runs until the closure returns. For verbose mode, the spinner runs briefly until `progress_init()` is called inside the cleaner, at which point the progress bar takes over visually. The spinner's `finish_and_clear()` at the end of `with_spinner` clears the spinner, and `progress_finish()` prints the `[OK]` marker. This produces a clean transition: spinner → progress bar → `[OK]`.

### 7. `Cleaner` Trait Change

```rust
pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self) -> ScanResult;
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
}
```

All existing cleaner implementations must update their `clean()` signature to accept `_reporter: &dyn ProgressReporter` (ignored if unused).

### 8. Multi-Entry Cleaner Integration

Each multi-entry cleaner adds three calls in its `clean()` / `clean_all()` methods:

```rust
fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
    let entries = self.scan();  // or selected entries
    if entries.is_empty() {
        return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
    }
    // Initialize progress bar with total count
    reporter.progress_init(self.name(), entries.len());

    let mut freed = 0u64;
    for (i, entry) in entries.iter().enumerate() {
        // Report current file (path, index)
        reporter.progress_tick(&entry.path, i + 1);

        // Perform deletion
        let path_str = entry.path.to_string_lossy();
        let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
        crate::trash::delete_path(&entry.path)?;
        freed += entry.size;
    }

    reporter.progress_finish();
    Ok(CleanResult { name: self.name(), bytes_freed: freed })
}
```

#### Affected cleaners:

| Cleaner | File | Loop location |
|---------|------|---------------|
| LibraryLogs (interactive) | `src/cleaners/library_logs.rs` | `clean()` — selected entries |
| LibraryLogs (--all) | `src/cleaners/library_logs.rs` | `clean_all()` — all entries |
| LogCleaner | `src/cleaners/log.rs` | `clean()` — per-target files |
| GenericCacheCleaner (DeleteDirs) | `src/cleaners/generic.rs` | `clean()` — dirs to delete |
| BrowserCleaner | `src/cleaners/browser.rs` | `clean()` — old versions |
| XcodeCleaner | `src/cleaners/xcode.rs` | `clean()` — device support dirs |
| CargoCleaner | `src/cleaners/cargo.rs` | `clean()` — cached files |
| DeviceSupportCleaner | `src/cleaners/device_support.rs` | `clean()` — device dirs |
| OllamaCleaner | `src/cleaners/ollama.rs` | `clean()` — model dirs |
| MiseCleaner | `src/cleaners/mise.rs` | `clean()` — unused versions |

### 9. `with_spinner` Compatibility

Keep existing `with_spinner()` unchanged. For verbose mode, the cleaner's `progress_init()` creates a fresh ProgressBar that coexists with the spinner. The spinner auto-clears when the cleaner function returns, and `progress_finish()` provides the completion marker in its place.

### 9. `with_spinner` Compatibility

Keep existing `with_spinner()` unchanged. The reporter-aware version is integrated directly into `run_clean_target` via the `show_spinner()` check shown above.

## Files Changed

| File | Change | Responsibility |
|------|--------|----------------|
| `src/progress.rs` | Add `ProgressReporter` trait + 3 impls + `build_reporter_from_flags()` + unit tests | Trait + factory |
| `src/main.rs` | Add `--suppress` / `--deep-suppress` flags, `build_reporter()` reads config + CLI, update `run_clean_target`, pass reporter through `--yes` path | CLI + dispatch |
| `src/config.rs` | Add `suppress` / `deep_suppress` fields to `RawConfig` + `Config` + `load()` + unit tests | Config file |
| `src/cleaner.rs` | Add `&dyn ProgressReporter` to `clean()` signature | Trait change |
| `src/cleaners/*.rs` | Mechanical: accept `_reporter` in `clean()` (all 17 files, single-command cleaners ignore) | Signature update |
| `src/cleaners/library_logs.rs` | Add `progress_init/tick/finish` in `clean()` and `clean_all()` | Progress bar |
| `src/cleaners/log.rs` | Add `progress_init/tick/finish` in `clean()` delete loops | Progress bar |
| `src/cleaners/generic.rs` | Add `progress_init/tick/finish` in DeleteDirs loop | Progress bar |
| `src/cleaners/browser.rs` | Add `progress_init/tick/finish` in version cleanup loop | Progress bar |
| `src/cleaners/xcode.rs` | Add `progress_init/tick/finish` in device dir deletion loop | Progress bar |
| `src/cleaners/cargo.rs` | Add `progress_init/tick/finish` in cache deletion loop | Progress bar |
| `src/cleaners/device_support.rs` | Add `progress_init/tick/finish` in deletion loop | Progress bar |
| `src/cleaners/ollama.rs` | Add `progress_init/tick/finish` in model deletion loop | Progress bar |
| `src/cleaners/mise.rs` | Add `progress_init/tick/finish` in version deletion loop | Progress bar |
| `tests/progress.rs` | Create — E2E tests for `--suppress` / `--deep-suppress` flags (Tests 13–15) | E2E coverage |
| `tests/*.rs` | Mechanical: update `clean()` calls to pass a test reporter | Test compatibility |

## Testing Strategy (TDD)

Every piece of production code must have a **failing test before it exists**. Each test below specifies the exact RED → GREEN cycle.

### Test 1: `VerboseProgress` exists and shows spinner (unit)

**RED** — `src/progress.rs`:

```rust
#[test]
fn verbose_progress_shows_spinner() {
    let reporter = VerboseProgress::new();
    assert!(reporter.show_spinner());
}
```

**Expected failure:** `error[E0433]: failed to resolve: use of undeclared type 'VerboseProgress'`

**Minimal GREEN:** Define struct + trait impl:

```rust
pub struct VerboseProgress {
    pb: std::sync::Mutex<Option<ProgressBar>>,
}

impl VerboseProgress {
    pub fn new() -> Self {
        Self { pb: std::sync::Mutex::new(None) }
    }
}

impl ProgressReporter for VerboseProgress {
    fn show_spinner(&self) -> bool { true }
    fn progress_init(&self, _label: &str, _total: usize) {}
    fn progress_tick(&self, _path: &Path, _current: usize) {}
    fn progress_finish(&self) {}
}
```

### Test 2: `DeepSuppressReporter` hides spinner (unit)

```rust
#[test]
fn deep_suppress_reporter_hides_spinner() {
    let reporter = DeepSuppressReporter;
    assert!(!reporter.show_spinner());
}
```

### Test 3: `SuppressReporter` shows spinner (unit)

```rust
#[test]
fn suppress_reporter_shows_spinner() {
    let reporter = SuppressReporter;
    assert!(reporter.show_spinner());
}
```

### Test 4: `VerboseProgress` progress lifecycle compiles and runs (unit)

```rust
#[test]
fn verbose_progress_lifecycle() {
    let reporter = VerboseProgress::new();
    let path = Path::new("/tmp/test.log");
    reporter.progress_init("test", 5);
    reporter.progress_tick(path, 1);
    reporter.progress_tick(path, 2);
    reporter.progress_finish();
    // Must not panic or deadlock
    assert!(reporter.show_spinner());
}
```

**Expected failure (first run):** `error[E0599]: no method named 'progress_init' found` — trait methods not implemented.

### Test 5–6: Suppress/DeepSuppress progress methods are no-ops

```rust
#[test]
fn suppress_reporter_progress_is_noop() {
    let reporter = SuppressReporter;
    reporter.progress_init("test", 5);   // must not panic
    reporter.progress_tick(Path::new("/x"), 1);
    reporter.progress_finish();
}

#[test]
fn deep_suppress_reporter_progress_is_noop() {
    let reporter = DeepSuppressReporter;
    reporter.progress_init("test", 5);
    reporter.progress_tick(Path::new("/x"), 1);
    reporter.progress_finish();
}
```

### Test 7: `build_reporter_from_flags` factory (unit)

```rust
#[test]
fn build_reporter_default_verbose() {
    let r = build_reporter_from_flags(false, false);
    assert!(r.show_spinner());
}

#[test]
fn build_reporter_deep_suppress_wins_over_suppress() {
    let r = build_reporter_from_flags(true, true);
    assert!(!r.show_spinner(), "deep_suppress must win");
}

#[test]
fn build_reporter_suppress_shows_spinner() {
    let r = build_reporter_from_flags(true, false);
    assert!(r.show_spinner(), "suppress still shows spinner");
}
```

### Test 8: Config loads `suppress = true` from TOML (unit)

**`src/config.rs`** — extend existing `#[cfg(test)]` block:

```rust
#[test]
fn config_loads_suppress_true() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("config.toml"), "suppress = true\n").unwrap();
    let cfg = Config::load(tmp.path()).unwrap();
    assert!(cfg.suppress, "suppress from config must be true");
}
```

**Expected failure:** `error[E0609]: no field 'suppress' on type 'Config'`

**Minimal GREEN:** Add `suppress: bool` to `RawConfig`, `Config`, and `Config::load()`.

### Test 9: Config defaults `suppress` to `false` (unit)

```rust
#[test]
fn config_default_suppress_is_false() {
    let cfg = Config::default();
    assert!(!cfg.suppress, "default suppress must be false");
}
```

### Test 10: Config loads `deep_suppress = true` from TOML (unit)

Analogous to Test 8 for `deep_suppress` field.

### Test 11: Merge flags logic (unit)

```rust
#[test]
fn merge_flags_cli_suppress_overrides_config() {
    // merge_flags(cli_suppress, cli_deep, cfg_suppress, cfg_deep)
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

Extract `fn merge_suppress_flags(...)` so the merge logic is testable without Cli/Config.

### Test 12 (E2E): `--suppress` hides progress bar

**Test file:** `tests/progress.rs`

```rust
#[test]
fn suppress_flag_hides_per_entry_output() {
    let tmp = TempDir::new().unwrap();
    // Create a large log entry that library-logs will clean
    let dir = tmp.path().join("Library/Logs/BloatApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash.log"), vec![0u8; (100 * 1024 * 1024) + 1]).unwrap();

    let output = assert_cmd::Command::cargo_bin("sasurahime").unwrap()
        .env("HOME", tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "library-logs", "--all", "--suppress"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show Freed line but NOT progress bar artifacts (ETA, "/")
    assert!(!stdout.contains("/"), "suppress must not show progress bar:\n{stdout}");
    assert!(stdout.contains("Freed:"), "stdout:\n{stdout}");
}
```

### Test 13 (E2E): `--deep-suppress` hides all output

```rust
#[test]
fn deep_suppress_flag_hides_all_output() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("Library/Logs/BloatApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash.log"), vec![0u8; (100 * 1024 * 1024) + 1]).unwrap();

    let output = assert_cmd::Command::cargo_bin("sasurahime").unwrap()
        .env("HOME", tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "library-logs", "--all", "--deep-suppress"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.is_empty(), "deep-suppress must produce no stdout:\n{stdout}");
}
```

### Test 14 (E2E): Default (no suppress) shows progress bar

Ensure the default behavior still works:

```rust
#[test]
fn default_shows_per_entry_output() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("Library/Logs/BloatApp");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("crash.log"), vec![0u8; (100 * 1024 * 1024) + 1]).unwrap();

    let output = assert_cmd::Command::cargo_bin("sasurahime").unwrap()
        .env("HOME", tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "library-logs", "--all"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show some progress indicator (progress bar chars or "ETA" or "Freed:")
    assert!(stdout.contains("Freed:"), "default must show freed line:\n{stdout}");
}
```

The terminal output for verbose mode contains ANSI escape codes from indicatif, making exact string matching brittle. The E2E tests focus on exit code, stdout content presence/absence at a high level, and file system state.

### Test 15–N: All existing tests must still pass after signature change

Every existing test that calls `cleaner.clean(dry_run)` will fail to compile because the trait now requires `cleaner.clean(dry_run, &reporter)`. Each such call site needs:

```rust
// Existing test:
cleaner.clean(true).unwrap();
// Updated:
let reporter = crate::progress::VerboseProgress::new();
cleaner.clean(true, &reporter).unwrap();
```

This is a mechanical change across `tests/*.rs` and `src/cleaners/*.rs` `#[cfg(test)]` blocks.

### TDD Execution Order

1. Write Test 1 → fails (no type) → GREEN: define `VerboseProgress` + trait impl with no-op methods
2. Write Test 2 → fails → GREEN: define `DeepSuppressReporter`
3. Write Test 3 → fails → GREEN: define `SuppressReporter`
4. Write Test 4 → fails (no progress methods) → GREEN: implement progress lifycycle (no-op body → compiles & runs)
5. Write Test 5–6 → no-ops pass → confirm
6. Write Test 7 → fails (no factory) → GREEN: add `build_reporter_from_flags`
7. Write Tests 8–10 (config) → fails → GREEN: add fields to `RawConfig`, `Config`, `load()`
8. Write Test 11 → fails → GREEN: extract `merge_suppress_flags()`
9. Update `build_reporter(cli, config)` to merge CLI + config
10. E2E Tests 12–14 in `tests/progress.rs` — each RED → GREEN
11. Mechanical: add `progress_init/tick/finish` to 9 multi-entry cleaners
12. Mechanical: update all 17+ existing `clean(dry_run)` call sites → compilation failure → fix each
13. Run full suite: `cargo test` — all pass

## Non-Goals

- No per-entry text output for dry-run mode (the existing per-entry listing format is kept)
- No per-file speed calculation (estimated time remaining is for the batch, not per file)
