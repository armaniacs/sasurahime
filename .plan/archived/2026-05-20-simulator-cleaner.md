# iOS Simulator Cache Cleaner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Add a `simulator` cleaner that detects `~/Library/Developer/CoreSimulator/` size and runs `xcrun simctl delete unavailable`.

**Architecture:** GenericCleaner with `CommandWithDetectDir` (same pattern as `colima`). `detect()` reports dir_size of CoreSimulator. `clean()` runs `xcrun simctl delete unavailable`.

**Tech Stack:** Rust, GenericCleaner, CommandRunner

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/cleaners/generic.rs` | Modify | Add `simulator()` factory |
| `src/main.rs` | Modify | Add to `define_cleaners!` (standard variant) |
| `tests/generic.rs` | Modify | Add E2E tests |

---

### Task 1: Add `simulator()` factory

- [ ] **Step 1: Add factory to `src/cleaners/generic.rs`**

```rust
pub fn simulator(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    Self {
        display_name: "simulator",
        method: CleanMethod::CommandWithDetectDir {
            program: "xcrun",
            args: &["simctl", "delete", "unavailable"],
            detect_dir: home.join("Library/Developer/CoreSimulator"),
        },
        runner,
    }
}
```

### Task 2: CLI registration

- [ ] **Step 1: Add to `define_cleaners!` in `src/main.rs`**

```rust
Simulator : "simulator" => "iOS Simulator cache (xcrun simctl delete unavailable)";
(|home, _config| cleaners::generic::GenericCleaner::simulator(home, Box::new(SystemCommandRunner))),
```

### Task 3: E2E tests

- [ ] **Step 1: Add to `tests/generic.rs`**

```rust
#[test]
fn clean_simulator_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "simulator"])
        .output()
        .unwrap();
    assert!(output.status.success());
}
```
