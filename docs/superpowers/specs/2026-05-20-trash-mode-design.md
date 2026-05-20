# Partial Safe Mode (Trash Mode) Design Doc

**Date:** 2026-05-20
**Status:** Approved
**Target Branch:** sprint5

---

## Overview

Add a "trash mode" where directly-deleted files are moved to the macOS Trash (`~/.Trash/`) instead of being permanently removed. This gives users a recovery window and makes sasurahime safer for exploratory use.

## Motivation

Currently, sasurahime permanently deletes files via `fs::remove_dir_all()` and `fs::remove_file()`. While `--dry-run` lets users preview, there's no undo for an actual `clean`. Moving to Trash provides:

- **Safety net**: Accidentally deleted caches can be restored from Trash
- **Confidence**: Users can try `clean` without fear of data loss
- **Transparency**: Finder shows what was moved, when, and how much

## Scope

**Applies to**: Cleaners that use direct filesystem deletion (`fs::remove_dir_all`, `fs::remove_file`).
**Does NOT apply to**: Cleaners that delegate to external CLI tools (`brew cleanup`, `bun pm cache rm`, `docker system prune`, `colima prune --all`, `xcrun simctl`, etc.).

## Architecture

**Approach:** Module-level `AtomicBool` flag + central `delete_path()` helper. No changes to the `Cleaner` trait.

```rust
// src/trash.rs (new file)

use std::sync::atomic::{AtomicBool, Ordering};
use std::path::Path;
use anyhow::Result;

static TRASH_MODE: AtomicBool = AtomicBool::new(false);

/// Called once at startup from main().
pub fn set_trash_mode(enabled: bool) {
    TRASH_MODE.store(enabled, Ordering::Relaxed);
}

/// Moves `path` to Trash if trash mode is enabled, otherwise calls fs::remove_dir_all.
/// Works for both files and directories.
pub fn delete_path(path: &Path) -> Result<()> {
    if TRASH_MODE.load(Ordering::Relaxed) {
        trash::delete(path).map_err(|e| anyhow::anyhow!("trash failed: {e}"))
    } else {
        fs::remove_dir_all(path)
            .map_err(|e| anyhow::anyhow!("remove_dir_all {:?}: {}", path, e))
    }
}
```

## Deletion Points (11 locations)

All `fs::remove_dir_all` / `fs::remove_file` calls replaced with `trash::delete_path`.

| # | File | Current | Change |
|:-:|------|---------|--------|
| 1 | `generic.rs` (DeleteDirs) | `fs::remove_dir_all(dir)` | `trash::delete_path(dir)?` |
| 2 | `generic.rs` (clean_cli_or_fallback) | `fs::remove_dir_all(dir)` | `trash::delete_path(dir)?` |
| 3 | `log.rs` | `fs::remove_file(path)` | `trash::delete_path(path)?` |
| 4 | `mise.rs` (remove_with_uchg) | `fs::remove_dir_all(path)` | `trash::delete_path(path)?` |
| 5 | `browser.rs` | `fs::remove_dir_all(&path)` | `trash::delete_path(&path)?` |
| 6 | `xcode.rs` | `fs::remove_dir_all(&dir)` | `trash::delete_path(&dir)?` |
| 7 | `cargo.rs` (2 locations) | `fs::remove_dir_all(...)` | `trash::delete_path(...)?` |
| 8 | `library_logs.rs` (2 locations + clean_all) | `fs::remove_dir_all(...)` | `trash::delete_path(...)?` |
| 9 | `device_support.rs` | `fs::remove_dir_all(p)` | `trash::delete_path(p)?` |
| 10 | `ollama.rs` | `fs::remove_dir_all(dir)` | `trash::delete_path(dir)?` |
| 11 | `main.rs` (LibraryLogs --all) | `std::fs::remove_dir_all(...)` | `trash::delete_path(...)?` |

**chflags stays**: `chflags -R nouchg` is retained immediately before each `delete_path()` call, as immutable flags also prevent trash operations.

## CLI & Config

### CLI Flag

```rust
#[derive(Parser)]
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

### Config File

```toml
# ~/.config/sasurahime/config.toml
trash_mode = true   # default: false
```

`Config` struct gains `trash_mode: bool` (default `false`).

### Priority

```
CLI --trash flag  >  config.toml trash_mode  >  default (false)
```

### Startup Logic (main.rs)

```rust
fn main() -> Result<()> {
    let cli = Cli::parse();
    // ... load config ...
    trash::set_trash_mode(cli.trash || config.trash_mode);
    // ...
}
```

### Interaction with --dry-run

- `--dry-run --trash` → valid: dry-run takes precedence, nothing deleted/trashed
- `--trash` alone → files moved to Trash
- Neither → direct permanent deletion (existing behavior)

### Interaction with --yes

- `--yes` alone → no confirmation, all pruneable targets cleaned permanently (existing behavior)
- `--yes --trash` → scan summary displayed, `Proceed? [y/N]` confirmation prompt. y proceeds with trash, N aborts. Safer than pure `--yes` since trashing is a bulk operation that may surprise users.

## Files Changed

| File | Change | Responsibility |
|------|--------|----------------|
| `Cargo.toml` | Add `trash = "5"` dependency | Trash crate |
| `src/trash.rs` | Create | `delete_path()` + `AtomicBool` flag |
| `src/main.rs` | Modify | Add `--trash` flag, call `set_trash_mode()`, replace 1 deletion point |
| `src/config.rs` | Modify | Add `trash_mode: bool` field |
| `tests/trash.rs` | Create | E2E tests for `--trash` flag behavior |
| `src/cleaners/generic.rs` | Modify | Replace `fs::remove_dir_all` (2 locations) |
| `src/cleaners/log.rs` | Modify | Replace `fs::remove_file` |
| `src/cleaners/mise.rs` | Modify | Replace `fs::remove_dir_all` |
| `src/cleaners/browser.rs` | Modify | Replace `fs::remove_dir_all` |
| `src/cleaners/xcode.rs` | Modify | Replace `fs::remove_dir_all` |
| `src/cleaners/cargo.rs` | Modify | Replace `fs::remove_dir_all` (2 locations) |
| `src/cleaners/library_logs.rs` | Modify | Replace `fs::remove_dir_all` (3 locations) |
| `src/cleaners/device_support.rs` | Modify | Replace `fs::remove_dir_all` |
| `src/cleaners/ollama.rs` | Modify | Replace `fs::remove_dir_all` |

## Testing Strategy (TDD)

All tests must be written **before** the corresponding production code and watched fail first.

### Unit: `src/trash.rs`

#### Test 1: `delete_path` with trash mode removes a file from source

```rust
#[test]
fn delete_path_in_trash_mode_removes_file_from_source() {
    trash::set_trash_mode(true);
    let tmp = TempDir::new().unwrap();
    let f = tmp.path().join("test.txt");
    fs::write(&f, b"hello").unwrap();

    trash::delete_path(&f).unwrap();

    assert!(!f.exists(), "file must be removed from source after trash");
}
```

#### Test 2: `delete_path` without trash mode removes a directory

```rust
#[test]
fn delete_path_in_normal_mode_removes_directory() {
    trash::set_trash_mode(false);
    let tmp = TempDir::new().unwrap();
    let d = tmp.path().join("testdir");
    fs::create_dir_all(&d).unwrap();

    trash::delete_path(&d).unwrap();

    assert!(!d.exists(), "directory must be removed");
}
```

#### Test 3: `delete_path` defaults to normal deletion when `set_trash_mode` is never called

```rust
#[test]
fn delete_path_defaults_to_normal_mode() {
    // set_trash_mode() was never called → default is false
    let tmp = TempDir::new().unwrap();
    let d = tmp.path().join("default_dir");
    fs::create_dir_all(&d).unwrap();

    trash::delete_path(&d).unwrap();

    assert!(!d.exists(), "default mode must be normal deletion (false)");
}
```

#### Test 4: `delete_path` propagates error when `trash::delete()` fails (unwritable path)

```rust
#[test]
fn delete_path_in_trash_mode_returns_error_on_failure() {
    trash::set_trash_mode(true);
    let result = trash::delete_path(Path::new("/nonexistent/path/that/cannot/be/trashed"));
    assert!(result.is_err(), "trash of nonexistent path must return Err");
}
```

### Unit: `src/config.rs`

#### Test 5: Config loads `trash_mode = true` from TOML

```rust
#[test]
fn config_loads_trash_mode_true() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("config.toml"), "trash_mode = true\n").unwrap();
    let cfg = Config::load(tmp.path()).unwrap();
    assert!(cfg.trash_mode, "trash_mode from config must be true");
}
```

#### Test 6: Config default `trash_mode` is `false`

```rust
#[test]
fn config_default_trash_mode_is_false() {
    let cfg = Config::default();
    assert!(!cfg.trash_mode, "default trash_mode must be false");
}
```

### E2E: `tests/trash.rs` (create this file)

#### Test 7: `--trash --dry-run` deletes nothing and shows "would move to Trash"

```rust
#[test]
fn trash_flag_with_dry_run_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/uv/simple-v16");
    fs::create_dir_all(&cache).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "uv", "--trash", "--dry-run"])
        .output().unwrap();

    assert!(output.status.success());
    assert!(cache.exists(), "--dry-run must prevent deletion/trashing");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("would move to Trash"), "stdout:\n{stdout}");
}
```

#### Test 8: `--trash` clean shows "moved to Trash" message

```rust
#[test]
fn trash_clean_shows_moved_to_trash_message() {
    let tmp = TempDir::new().unwrap();
    let cache = tmp.path().join(".cache/uv/simple-v16");
    fs::create_dir_all(&cache).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "uv", "--trash"])
        .output().unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("moved to Trash"), "stdout:\n{stdout}");
}
```

### Test Implementation Order (Outside-In TDD)

1. **RED**: Write Test 3 (`delete_path_defaults_to_normal_mode`) → fails: `trash` module not found
2. **GREEN**: Create `src/trash.rs` with minimal `delete_path()`
3. **RED**: Write Test 2 (`delete_path_in_normal_mode_removes_directory`) → fails if delete_path doesn't remove
4. **GREEN**: Implement `fs::remove_dir_all` branch
5. **RED**: Write Test 1 (`delete_path_in_trash_mode_removes_file_from_source`) → fails: `trash::delete` not called
6. **GREEN**: Add `trash` crate, implement `trash::delete` branch
7. **RED**: Write Test 4 → fails: error not propagated
8. **GREEN**: Ensure `?` propagation
9. **RED**: Write Test 5+6 (config) → fails: `trash_mode` field missing
10. **GREEN**: Add `trash_mode` to Config
11. **RED**: Write Test 7 (E2E dry-run) → fails: `--trash` flag unrecognized
12. **GREEN**: Add `--trash` to Cli, wire `set_trash_mode()`
13. **RED**: Write Test 8 (E2E message) → fails: message doesn't contain "moved to Trash"
14. **GREEN**: Update `run_clean_target` / `clean()` output to show trash message
15. Apply `delete_path()` to all 11 deletion points
16. Run full suite: all existing + 8 new tests must pass

---

## Deep Dig Findings — 2026-05-20

### 挑戦した仮定

| 仮定 | リスク | 発見 | 決定 |
|------|:---:|------|------|
| ゴミ箱移動しても空き容量が変わらずユーザーが混乱しない | 高 | `clean` 完了時に容量が変わっていないとユーザーが混乱する。「効いてない」と誤認する | 完了メッセージを `Freed: 0 B (18.2 GB moved to Trash)` に変更。ゴミ箱を空にすれば回収できることを明示する |
| `--yes --trash` の組み合わせが安全 | 高 | `--yes` + `--trash` で全 pruneable ターゲットが無確認でゴミ箱に移動する。数十GBが一気に放り込まれうる | `--yes --trash` 時はスキャン結果サマリーを表示し、`Proceed? [y/N]` の確認を入れる。pure `--yes` より1段階安全にする |
| `trash::delete()` 失敗時の挙動が自明 | 中 | 現状の cleaner によってエラー伝播 (`?`) と警告ログのみ (`if let Err`) が混在している | trash 失敗時はエラーを伝播する (`?`)。部分移動によるデータ不整合より明示的失敗を優先する |

### 決定事項

1. `clean()` 完了時の出力書式: `Freed: 0 B (N GB moved to Trash)`
2. `--dry-run --trash` 時の出力: `[dry-run] would move to Trash: N GB`
3. `--yes --trash` 時: scan 結果を表示し `Proceed? [y/N]` 確認プロンプト。y なら実行
4. `trash::delete()` のエラーは常に `?` で伝播する（全 cleaner 共通）
