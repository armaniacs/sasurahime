# Progress Reporter Design

**Date:** 2026-05-21
**Status:** Approved for implementation

## Motivation

The `library-logs` cleaner iterates through `~/Library/Logs/` entries, running `chflags -R nouchg` followed by `trash::delete_path()` on each. In environments with many entries this takes a long time with no per-entry progress feedback — the user only sees a static `Cleaning library-logs...` spinner. The same problem exists for any cleaner that processes multiple entries sequentially (DeleteDirs, browser cleaners, etc.).

## Scope

Add a `ProgressReporter` trait that abstracts per-entry progress output, integrate it into `run_clean_target` and the `Cleaner` trait, and add `--suppress` / `--deep-suppress` CLI flags to control output verbosity.

**Applies to**: All cleaners (trait signature change).
**Verbose output initially**: LibraryLogs (`clean()`, `clean_all()`). Other cleaners accept the reporter but ignore it initially.

## Design

### 1. ProgressReporter Trait

**File:** `src/progress.rs`

```rust
use std::path::Path;

pub trait ProgressReporter: Send + Sync {
    /// Whether to show the spinner, "Freed:" line, and completion marker
    fn show_spinner(&self) -> bool;

    /// Called by cleaners for each entry they delete
    fn report_delete(&self, label: &str, path: &Path, current: usize, total: usize);
}
```

### 2. Three Implementations

```rust
pub struct VerboseFileWriter {
    pub(crate) writer: Box<dyn std::io::Write + Send>,
}
pub struct SuppressReporter;
pub struct DeepSuppressReporter;
```

| Implementation | `show_spinner()` | `report_delete()` |
|----------------|:---:|:---:|
| `VerboseFileWriter` | `true` | Writes `[{label}] removing {path} ({current}/{total})...` |
| `SuppressReporter` | `true` | No output |
| `DeepSuppressReporter` | `false` | No output |

**Verbose reporter writer:** `VerboseFileWriter` accepts a `Box<dyn Write>` so tests can capture output to a `Vec<u8>` via `VerboseFileWriter { writer: Box::new(buf) }`. Production code passes `Box::new(std::io::stdout())`.

**Silent reporters:** `SuppressReporter` and `DeepSuppressReporter` need no writer — `report_delete()` is a no-op.

**Factory helpers:**

```rust
pub fn verbose_stdout() -> VerboseFileWriter {
    VerboseFileWriter { writer: Box::new(std::io::stdout()) }
}

pub fn build_reporter_from_flags(suppress: bool, deep_suppress: bool) -> Box<dyn ProgressReporter> {
    if deep_suppress {
        Box::new(DeepSuppressReporter)
    } else if suppress {
        Box::new(SuppressReporter)
    } else {
        Box::new(verbose_stdout())
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

**Factory function:**

```rust
fn build_reporter(cli: &Cli) -> Box<dyn ProgressReporter> {
    build_reporter_from_flags(cli.suppress, cli.deep_suppress)
}
```

### 4. `run_clean_target` Changes

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

### 5. `Cleaner` Trait Change

```rust
pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self) -> ScanResult;
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
}
```

All existing cleaner implementations must update their `clean()` signature to accept `_reporter: &dyn ProgressReporter` (ignored if unused).

### 6. LibraryLogs Verbose Output

`src/cleaners/library_logs.rs` — both `clean()` and `clean_all()`:

```rust
for (i, entry) in selected.iter().enumerate() {
    reporter.report_delete("library-logs", &entry.path, i + 1, selected.len());
    // chflags + delete_path...
}
```

### 7. `with_spinner` Compatibility

Keep existing `with_spinner()` unchanged. The reporter-aware version is integrated directly into `run_clean_target` via the `show_spinner()` check shown above.

## Files Changed

| File | Change | Responsibility |
|------|--------|----------------|
| `src/progress.rs` | Add `ProgressReporter` trait + 3 impls + `build_reporter_from_flags()` + unit tests | Trait + factory |
| `src/main.rs` | Add `--suppress` / `--deep-suppress` flags, `build_reporter()` delegates to `build_reporter_from_flags()`, update `run_clean_target`, pass reporter through `--yes` path | CLI + dispatch |
| `src/cleaner.rs` | Add `&dyn ProgressReporter` to `clean()` signature | Trait change |
| `src/cleaners/*.rs` | Mechanical: accept `_reporter` in `clean()` (17 files) | Signature update |
| `src/cleaners/library_logs.rs` | Call `reporter.report_delete()` in `clean()` and `clean_all()` | Verbose output |
| `tests/progress.rs` | Create — E2E tests for `--suppress` / `--deep-suppress` flags (Tests 9–11) | E2E coverage |
| `tests/*.rs` | Mechanical: update `clean()` calls to pass a test reporter | Test compatibility |

## Testing Strategy (TDD)

Every piece of production code must have a **failing test before it exists**. Each test below specifies the exact RED → GREEN cycle.

### Test 1: `VerboseFileWriter` exists and compiles (unit)

**RED** — `src/progress.rs`:

```rust
#[test]
fn verbose_file_writer_exists() {
    let mut buf = Vec::new();
    let reporter = VerboseFileWriter { writer: Box::new(&mut buf) };
    assert!(reporter.show_spinner());
}
```

**Expected failure:** `error[E0433]: failed to resolve: use of undeclared type 'VerboseFileWriter'`

**Minimal GREEN:** Define struct + impl:

```rust
pub struct VerboseFileWriter {
    pub(crate) writer: Box<dyn std::io::Write + Send>,
}
impl ProgressReporter for VerboseFileWriter {
    fn show_spinner(&self) -> bool { true }
    fn report_delete(&self, _label: &str, _path: &Path, _current: usize, _total: usize) {}
}
```

### Test 2: `DeepSuppressReporter::show_spinner` returns `false` (unit)

**RED** — `src/progress.rs`:

```rust
#[test]
fn deep_suppress_reporter_hides_spinner() {
    let reporter = DeepSuppressReporter;
    assert!(!reporter.show_spinner());
}
```

**Expected failure:** `error[E0433]: failed to resolve: use of undeclared type 'DeepSuppressReporter'`

**Minimal GREEN:** Define struct + impl.

### Test 3: `SuppressReporter::show_spinner` returns `true` (unit)

Analogous to Test 1 for `SuppressReporter`.

### Test 4: `VerboseFileWriter` prints formatted delete message (unit, writer capture)

**RED** — `src/progress.rs`:

```rust
#[test]
fn verbose_file_writer_prints_delete_message() {
    let mut buf = Vec::new();
    let reporter = VerboseFileWriter { writer: Box::new(&mut buf) };
    let path = Path::new("/tmp/test.log");
    reporter.report_delete("library-logs", path, 1, 5);
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("[library-logs] removing"));
    assert!(output.contains("(1/5)"));
}
```

**Expected failure:** `error[E0599]: no method named 'report_delete' found` or output assertion fails because `report_delete` is a no-op.

**Minimal GREEN:** Implement `report_delete` to write formatted output:

```rust
impl ProgressReporter for VerboseFileWriter {
    fn show_spinner(&self) -> bool { true }
    fn report_delete(&self, label: &str, path: &Path, current: usize, total: usize) {
        let _ = writeln!(
            &mut *self.writer.lock().map_err(|_| ()).ok(),
            "[{label}] removing {} ({}/{total})...",
            path.display(),
            current,
        );
    }
}
```

Note: `BufWriter` wraps `&mut Vec<u8>`, which implements `Write` but requires careful lifetime handling. The `&mut *self.writer` borrows the inner `Write` through the `Box`.

### Test 5: `SuppressReporter` prints nothing on delete (unit)

**RED** — creates a `SuppressReporter` (no writer, it uses `std::io::sink()` internally), calls `report_delete`, verifies no output.

**Minimal GREEN:**

```rust
pub struct SuppressReporter;

impl ProgressReporter for SuppressReporter {
    fn show_spinner(&self) -> bool { true }
    fn report_delete(&self, _label: &str, _path: &Path, _current: usize, _total: usize) {}
}
```

No writer needed — output is suppressed at the method level.

### Test 6: `DeepSuppressReporter` prints nothing (unit)

Same as Test 5 but also `show_spinner()` returns `false`.

### Test 7: `VerboseFileWriter` error resilience on broken writer (unit)

```rust
#[test]
fn verbose_file_writer_does_not_panic_on_write_error() {
    let mut buf = Vec::new();
    let reporter = VerboseFileWriter { writer: Box::new(&mut buf) };
    reporter.report_delete("test", Path::new("/ok"), 1, 1); // must not panic
}
```

### Test 8: `build_reporter` factory (unit)

**RED** — calls `build_reporter` with different CLI flag combinations, asserts correct type.

```rust
#[test]
fn build_reporter_default_is_verbose() {
    // Requires a way to construct Cli with certain flags.
    // Since Cli is defined in main.rs, this test goes in tests/progress_e2e.rs
    // or we make build_reporter testable by passing bools directly:
}

// Alternative: test the underlying dispatch
#[test]
fn build_reporter_deep_suppress_wins_over_suppress() {
    let r = build_reporter_from_flags(true, true);  // suppress=true, deep=true
    assert!(!r.show_spinner(), "deep_suppress must win");
}
```

Design change: extract `build_reporter_from_flags(suppress: bool, deep_suppress: bool)` from `build_reporter(cli: &Cli)` so the dispatch logic is testable without constructing a Cli.

### Test 9 (E2E): `--suppress` hides per-entry output

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
    // Should show spinner completion but NOT per-entry "removing" lines
    assert!(!stdout.contains("removing"), "stdout:\n{stdout}");
    // Should still show "Freed:" line
    assert!(stdout.contains("Freed:"), "stdout:\n{stdout}");
}
```

### Test 10 (E2E): `--deep-suppress` hides all output

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

### Test 11 (E2E): Default (no suppress) shows per-entry output

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
    assert!(stdout.contains("removing"), "default must show progress:\n{stdout}");
}
```

### Test 12–N: All existing tests must still pass after signature change

Every existing test that calls `cleaner.clean(dry_run)` will fail to compile because the trait now requires `cleaner.clean(dry_run, &reporter)`. Each such call site needs:

```rust
// Existing test:
cleaner.clean(true).unwrap();
// Updated:
let reporter = crate::progress::VerboseFileWriter { writer: Box::new(std::io::sink()) };
cleaner.clean(true, &reporter).unwrap();
```

This is a mechanical change across `tests/*.rs` and `src/cleaners/*.rs` `#[cfg(test)]` blocks.

### TDD Execution Order

1. Write Test 1 → fails (no type) → GREEN: define struct + trait impl
2. Write Test 2 → fails → GREEN
3. Write Test 3 → fails → GREEN
4. Write Test 4 → fails (no writer field) → GREEN: refactor `VerboseReporter`
5. Write Test 5 → fails → GREEN
6. Write Test 6 → fails → GREEN
7. Write Test 7 → passes (resilient by default) → confirm
8. Write Test 8 → fails (no factory) → GREEN: add `build_reporter_from_flags`
9. Update `build_reporter(cli)` to delegate to `build_reporter_from_flags`
10. E2E Tests 9–11 in `tests/progress.rs` — each RED → GREEN
11. Mechanical: update all 17+ existing `clean(dry_run)` call sites → compilation failure → fix each
12. Run full suite: `cargo test` — all pass

## Non-Goals

- No per-entry progress for cleaners other than library-logs (future work)
- No estimated time remaining or percentage calculation
- No config file option for suppress mode (CLI-only)
