# Xcode DeviceSupport Cleaner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Add a `device-support` cleaner that removes old iOS/tvOS/watchOS/visionOS DeviceSupport directories, keeping the most recent N versions.

**Architecture:** Dedicated `DeviceSupportCleaner` struct. Scans `~/Library/Developer/Xcode/<Platform> DeviceSupport/` for versioned directories. Keeps the highest N versions (default 2). Same version‑parsing + retention pattern as `BrowserCleaner`.

**Tech Stack:** Rust, walkdir (existing)

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/cleaners/device_support.rs` | Create | `DeviceSupportCleaner` |
| `src/cleaners/mod.rs` | Modify | Add `pub mod device_support;` |
| `src/main.rs` | Modify | Special variant + dispatch |
| `tests/device_support.rs` | Create | E2E tests |

---

### Task 1: Implement DeviceSupportCleaner

- [ ] **Step 1: Create `src/cleaners/device_support.rs`**

Uses `BrowserCleaner::find_old_versions` pattern adapted for DeviceSupport directory naming (e.g. `"16.4 (20E247)"` → major version `16`).

**Key behavior:**
- Scan `~/Library/Developer/Xcode/` for directories matching `*DeviceSupport`
- Each directory contains versioned subdirs like `"16.4 (20E247)"`, `"17.0 (21A328)"`
- Extract major version number from dir name; group by platform (iOS, watchOS, etc.)
- Keep the highest N major versions per platform (default: `--keep 2`)
- Delete the rest with `chflags + remove_dir_all`

### Task 2: CLI registration

- [ ] **Step 1: Special variant + dispatch (same pattern as `LibraryLogs`/`Xcode`)**

```rust
/// Remove old Xcode DeviceSupport directories, keeping recent N versions
#[command(name = "device-support")]
DeviceSupport {
    #[arg(long)]
    dry_run: bool,
    /// Number of recent versions to keep (default: 2)
    #[arg(long, default_value = "2")]
    keep: u32,
},
```

### Task 3: E2E tests

```rust
#[test]
fn clean_device_support_keeps_n_versions() {
    let tmp = TempDir::new().unwrap();
    let ios = tmp.path().join("Library/Developer/Xcode/iOS DeviceSupport");
    for v in &["14.0", "15.0", "16.0", "17.0"] {
        fs::create_dir_all(ios.join(v)).unwrap();
        fs::write(ios.join(v).join("dummy"), b"x").unwrap();
    }
    let output = sasurahime(tmp.path())
        .args(["clean", "device-support"])
        .output()
        .unwrap();
    assert!(output.status.success());
    // 4 versions → keep 2 highest → 14.0 and 15.0 deleted
    assert!(!ios.join("14.0").exists());
    assert!(!ios.join("15.0").exists());
    assert!(ios.join("16.0").exists());
    assert!(ios.join("17.0").exists());
}
```
