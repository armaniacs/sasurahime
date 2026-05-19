# Colima Cleaner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Add a `colima` cleaner to sasurahime that detects `~/.colima/` size and runs `colima prune --all`.

**Architecture:** New `CleanMethod::CommandWithDetectDir` variant on `GenericCleaner`. `detect()` returns `dir_size(~/.colima/)`. `clean()` delegates to `colima prune --all`. Standard macro registration.

**Tech Stack:** Rust, GenericCleaner (existing), assert_cmd + tempfile (existing)

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/cleaners/generic.rs` | Modify | Add `CleanMethod::CommandWithDetectDir` variant + `colima_prune()` factory |
| `src/main.rs` | Modify | Add `Colima` to `define_cleaners!` macro (standard variant) |
| `tests/generic.rs` | Modify | Add 4 E2E tests |

---

### Task 1: Add `CleanMethod::CommandWithDetectDir` variant

**Files:**
- Modify: `src/cleaners/generic.rs`

- [ ] **Step 1: Add the new variant to `CleanMethod`**

Replace the existing `CleanMethod` enum:

```rust
pub enum CleanMethod {
    Command {
        program: &'static str,
        args: &'static [&'static str],
    },
    CommandWithDetectDir {
        program: &'static str,
        args: &'static [&'static str],
        detect_dir: PathBuf,
    },
    DeleteDirs(Vec<PathBuf>),
}
```

- [ ] **Step 2: Update `detect()` to handle the new variant**

Add before `CleanMethod::DeleteDirs` arm in `GenericCleaner::detect()`:

```rust
CleanMethod::CommandWithDetectDir { detect_dir, .. } => {
    if !detect_dir.exists() {
        return ScanResult {
            name: self.name(),
            status: ScanStatus::NotFound,
        };
    }
    let bytes = dir_size(detect_dir);
    ScanResult {
        name: self.name(),
        status: if bytes > 0 {
            ScanStatus::Pruneable(bytes)
        } else {
            ScanStatus::Clean
        },
    }
}
```

- [ ] **Step 3: Update `clean()` to handle the new variant**

Add before `CleanMethod::DeleteDirs` arm in `GenericCleaner::clean()`:

```rust
CleanMethod::CommandWithDetectDir { program, args, detect_dir } => {
    let size_before = if detect_dir.exists() { dir_size(detect_dir) } else { 0 };

    if !self.runner.exists(program) {
        println!("{}: not found, skipping", self.display_name);
        return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
    }
    if dry_run {
        println!("[dry-run] would run: {program} {}", args.join(" "));
        if size_before > 0 {
            println!("[dry-run] would free: {}", crate::format::format_bytes(size_before));
        }
        return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
    }
    self.runner.run(program, args)?;
    let size_after = if detect_dir.exists() { dir_size(detect_dir) } else { 0 };
    let freed = size_before.saturating_sub(size_after);
    if freed > 0 {
        println!("Freed: {}", crate::format::format_bytes(freed));
    }
    Ok(CleanResult { name: self.name(), bytes_freed: freed })
}
```

- [ ] **Step 4: Add `colima_prune()` factory method**

Add to `impl GenericCleaner`:

```rust
pub fn colima_prune(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    Self {
        display_name: "colima",
        method: CleanMethod::CommandWithDetectDir {
            program: "colima",
            args: &["prune", "--all"],
            detect_dir: home.join(".colima"),
        },
        runner,
    }
}
```

- [ ] **Step 5: Build to verify compilation**

Run: `cargo build 2>&1`
Expected: Build succeeds.

- [ ] **Step 6: Run existing tests to check for regressions**

Run: `cargo test --lib cleaners::generic::tests 2>&1 | tail -10`
Expected: All existing unit tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/cleaners/generic.rs
git commit -m "feat: add CleanMethod::CommandWithDetectDir for colima prune --all"
```

---

### Task 2: CLI registration via macro

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Add to `define_cleaners!` macro**

In `src/main.rs`, add after `Downloads` entry in the standard cleaners list:

```rust
Colima : "colima" => "Colima VM disk cache prune";
(|home, _config| cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner))),
```

- [ ] **Step 2: Build and smoke test**

Run: `cargo build 2>&1`
Expected: Build succeeds.

Run: `cargo run -- targets 2>&1 | grep colima`
Expected: Shows `colima    Colima VM disk cache prune`

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: register colima cleaner in define_cleaners! macro"
```

---

### Task 3: E2E tests

**Files:**
- Modify: `tests/generic.rs`

- [ ] **Step 1: Add colima E2E tests**

Add to `tests/generic.rs`:

```rust
#[test]
fn clean_colima_calls_prune_all() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // Fake colima that records args
    let calls_file = bin_dir.join("calls_colima.txt");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
        calls_file.display()
    );
    fs::write(bin_dir.join("colima"), &script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join("colima"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    // Create fake ~/.colima dir
    fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
    fs::write(tmp.path().join(".colima/_lima/colima/dummy.img"), b"x").unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "colima"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = fs::read_to_string(&calls_file).unwrap_or_default();
    assert!(calls.contains("prune --all"), "expected 'prune --all', got: {calls}");
}

#[test]
fn clean_colima_dry_run_does_not_invoke() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let calls_file = bin_dir.join("calls_colima.txt");
    let script = format!(
        "#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n",
        calls_file.display()
    );
    fs::write(bin_dir.join("colima"), &script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join("colima"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "colima", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    // colima should not have been called in dry-run
    assert!(!calls_file.exists(), "colima must not be invoked in dry-run");
}

#[test]
fn clean_colima_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "colima"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not found") || stdout.contains("skipping"));
}

#[test]
fn scan_shows_colima_for_existing_dir() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
    fs::write(tmp.path().join(".colima/colima.yaml"), b"config").unwrap();

    let output = sasurahime(tmp.path())
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("colima"), "scan output should include colima:\n{stdout}");
}
```

- [ ] **Step 2: Run E2E tests**

Run: `cargo test --test generic clean_colima -- --nocapture 2>&1`
Expected: 4 new tests pass.

- [ ] **Step 3: Run full test suite**

Run: `cargo test 2>&1 | grep -E "(FAILED|test result:)"`
Expected: All tests pass.

- [ ] **Step 4: Clippy**

Run: `cargo clippy --tests -- -D warnings 2>&1`
Expected: 0 warnings.

- [ ] **Step 5: Commit**

```bash
git add src/cleaners/generic.rs src/main.rs tests/generic.rs
git commit -m "feat: add colima VM cache cleaner with prune --all"
```
