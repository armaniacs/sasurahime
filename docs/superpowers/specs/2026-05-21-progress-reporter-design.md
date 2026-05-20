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
pub struct VerboseReporter;
pub struct SuppressReporter;
pub struct DeepSuppressReporter;
```

| Implementation | `show_spinner()` | `report_delete()` |
|----------------|:---:|:---:|
| `VerboseReporter` | `true` | Prints `[{label}] removing {path} ({current}/{total})...` |
| `SuppressReporter` | `true` | No output |
| `DeepSuppressReporter` | `false` | No output |

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
    if cli.deep_suppress {
        Box::new(DeepSuppressReporter)
    } else if cli.suppress {
        Box::new(SuppressReporter)
    } else {
        Box::new(VerboseReporter)
    }
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
| `src/progress.rs` | Add `ProgressReporter` trait + 3 impls + `build_reporter()` | New file additions |
| `src/main.rs` | Add `--suppress` / `--deep-suppress` flags, update `run_clean_target`, pass reporter through `--yes` path | CLI + dispatch |
| `src/cleaner.rs` | Add `&dyn ProgressReporter` to `clean()` signature | Trait change |
| `src/cleaners/*.rs` | Mechanical: accept `_reporter` in `clean()` (17 files) | Signature update |
| `src/cleaners/library_logs.rs` | Call `reporter.report_delete()` in `clean()` and `clean_all()` | Verbose output |
| `tests/*.rs` | Update test `clean()` calls to pass a test reporter | Test compatibility |

## Non-Goals

- No per-entry progress for cleaners other than library-logs (future work)
- No estimated time remaining or percentage calculation
- No config file option for suppress mode (CLI-only)
