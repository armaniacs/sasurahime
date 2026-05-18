# Universal Version Display & Loading Spinners

**Date:** 2026-05-18
**Status:** Approved
**Branch:** `feature-0518b`

---

## Summary

Two UX improvements: (1) show the version number on **every** sasurahime
invocation, and (2) display a spinner with a descriptive message during
any detect/clean operation that may take ≥2 seconds.

---

## 1. Universal version display

**Files:**
- Modify: `src/main.rs`

Move the version banner from the `None` branch to the very top of `main()`,
before any subcommand dispatch:

```rust
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    println!("sasurahime v{}", env!("CARGO_PKG_VERSION"));   // ← always
    // ...
}
```

This causes `sasurahime scan`, `sasurahime clean <target>`,
`sasurahime targets`, `sasurahime --yes`, and bare `sasurahime` to all
print the version as their first line.

The existing `println!("sasurahime v{}", ...)` inside the `None` arm is
removed (no longer needed — the universal line runs first in all paths).

---

## 2. Loading spinner

### 2a. New module: `src/progress.rs`

A single public helper function:

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

Add `pub mod progress;` to `src/main.rs`.

### 2b. scan — `src/scanner.rs`

```rust
let results: Vec<_> = cleaners
    .iter()
    .map(|c| {
        let name = c.name();
        crate::progress::with_spinner(&format!("Scanning {name}..."), || c.detect())
    })
    .collect();
```

### 2c. clean — `src/interactive.rs`

**`run_auto`:**
```rust
for i in pruneable_indices {
    let name = cleaners[i].name();
    let result = crate::progress::with_spinner(
        &format!("Cleaning {name}..."),
        || cleaners[i].clean(false),
    );
    match result {
        Ok(r) => total_freed += r.bytes_freed,
        Err(e) => eprintln!("Error cleaning {}: {e}", cleaners[i].name()),
    }
}
```

**`run_interactive`:** Same wrapping pattern for the clean loop (after user confirmation).

### 2d. clean — `src/main.rs` direct-target arms

Each `CleanTarget` variant wraps its `cleaner.clean(dry_run)` call:

```rust
CleanTarget::Uv { dry_run } => {
    let cleaner = Box::new(cleaners::uv::UvCleaner::new(...));
    let result = crate::progress::with_spinner("Cleaning uv...", || cleaner.clean(dry_run))?;
    println!("Freed: {}", format::format_bytes(result.bytes_freed));
}
```

**All 14 targets** follow the same pattern. In `dry_run = true` the spinner
is still shown (the operation returns quickly, but the spinner is harmless).

---

## 3. Output examples

```
$ sasurahime scan
sasurahime v0.1.2
⠋ Scanning uv...
⠋ Scanning brew...
⠋ Scanning mise...
⠋ Scanning browsers...
✔ Scanning uv... ✓
✔ Scanning brew... ✓
✔ Scanning mise... ✓
✔ Scanning browsers... ✓

Category       Size       Status
...
```

```
$ sasurahime --yes
sasurahime v0.1.2
⠋ Cleaning uv...
✔ Cleaning uv... ✓
⠋ Cleaning brew...
✔ Cleaning brew... ✓
...
Total freed: 42.0 GB
```

---

## 4. Files changed

| File | Change |
|------|--------|
| `src/progress.rs` | **Create** — `with_spinner()` helper |
| `src/main.rs` | Add `mod progress;`; move version println to top; remove duplicate from None arm; wrap each CleanTarget.clean() with spinner |
| `src/scanner.rs` | Wrap `c.detect()` with `with_spinner()` |
| `src/interactive.rs` | Wrap `clean()` calls in `run_auto()` and `run_interactive()` with `with_spinner()` |
| `tests/interactive.rs` | Add version-display E2E tests for scan, clean, targets |

---

## 5. Test plan

| Test | Type | File | Assertion |
|------|------|------|-----------|
| `version_display_on_scan` | E2E | `tests/interactive.rs` | `sasurahime scan` stdout starts with `sasurahime v0.1.2` |
| `version_display_on_clean_dry_run` | E2E | `tests/interactive.rs` | `sasurahime clean uv --dry-run` starts with `sasurahime v0.1.2` |
| `version_display_on_targets` | E2E | `tests/interactive.rs` | `sasurahime targets` starts with `sasurahime v0.1.2` |
| `with_spinner_returns_value` | Unit | `src/progress.rs` | `with_spinner("test", \|\| 42)` returns `42` |

---

## 6. Self-review

- [x] No placeholders or TBDs
- [x] Architecture matches existing patterns (modular, no trait changes)
- [x] Scope is focused (version + spinner, nothing else)
- [x] All requirements unambiguous
- [x] Test plan covers all changes
