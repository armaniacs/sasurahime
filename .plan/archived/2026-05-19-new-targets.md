# New Cache Targets Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add 8 new clean targets (Deno, Docker, Orbstack, CocoaPods, SwiftPM, Conda, Poetry, pipx, Cargo, Rustup, Gradle, JetBrains, Trash, Downloads) to sasurahime.

**Architecture:** Three implementation patterns: (1) external CLI delegation via `GenericCleaner`, (2) directory deletion via `GenericCleaner::DeleteDirs`, (3) version-aware cleaning via dedicated modules (browser/mise pattern). All targets follow the `Cleaner` trait with `detect` + `clean` + `--dry-run`.

**Tech Stack:** Rust + assert_cmd + tempfile

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/cleaners/generic.rs` | Add `deno`, `pipx`, `docker`, `orbstack`, `cocoapods`, `conda`, `poetry`, `spm_cache`, `cargo_ registry` factory methods |
| `src/cleaners/cargo.rs` | **Create** — Cargo target dir scanning + registry cleanup |
| `src/cleaners/rustup.rs` | **Create** — Rustup toolchain management |
| `src/cleaners/gradle.rs` | **Create** — Gradle + JetBrains IDE cache cleanup |
| `src/cleaners/mod.rs` | Add `pub mod` for new modules |
| `src/main.rs` | Add `CleanTarget` variants, `all_cleaners()`, `SUPPORTED_TARGETS` |
| `tests/generic.rs` | E2E tests for CLI-delegation targets |
| `tests/cargo.rs` | **Create** — Cargo E2E tests |
| `tests/rustup.rs` | **Create** — Rustup E2E tests |
| `tests/gradle.rs` | **Create** — Gradle/JetBrains E2E tests |

---

### Task 0: Prepare CleanTarget enum, SUPPORTED_TARGETS, and module registration

**Files:**
- Modify: `src/main.rs` — add all CleanTarget variants + SUPPORTED_TARGETS entries
- Modify: `src/cleaners/mod.rs` — add `pub mod cargo;` etc.

- [ ] **Step 1: Write the failing test**

Add to `tests/interactive.rs`:

```rust
#[test]
fn targets_subcommand_includes_new_targets() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .arg("targets")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // New targets
    assert!(stdout.contains("cargo"), "stdout: {stdout}");
    assert!(stdout.contains("docker"), "stdout: {stdout}");
    assert!(stdout.contains("deno"), "stdout: {stdout}");
    assert!(stdout.contains("rustup"), "stdout: {stdout}");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test interactive targets_subcommand_includes_new_targets -- --nocapture`
Expected: FAIL — new targets not yet in SUPPORTED_TARGETS or CleanTarget.

- [ ] **Step 3: Add CleanTarget variants and SUPPORTED_TARGETS entries**

In `src/main.rs`:

**a)** Add to `Commands::Clean` subenum `CleanTarget`:

```rust
#[derive(Subcommand)]
enum CleanTarget {
    // Existing targets...
    /// Clean Cargo build cache
    Cargo { dry_run: bool },
    /// Clean Docker system cache
    Docker { dry_run: bool },
    /// Clean Orbstack cache
    Orbstack { dry_run: bool },
    /// Clean CocoaPods cache
    CocoaPods { dry_run: bool },
    /// Clean SwiftPM cache
    SwiftPM { dry_run: bool },
    /// Clean Conda package cache
    Conda { dry_run: bool },
    /// Clean Poetry cache
    Poetry { dry_run: bool },
    /// Clean pipx caches
    Pipx { dry_run: bool },
    /// Clean Rustup toolchains
    Rustup { dry_run: bool },
    /// Clean Gradle caches
    Gradle { dry_run: bool },
    /// Clean JetBrains IDE caches
    JetBrains { dry_run: bool },
    /// Report Trash size
    Trash { dry_run: bool },
    /// Clean old Downloads
    Downloads { dry_run: bool },
}
```

Add to `SUPPORTED_TARGETS`:

```rust
const SUPPORTED_TARGETS: &[(&str, &str)] = &[
    // Existing entries...
    ("cargo",    "Cargo registry cache + target/ directories"),
    ("docker",   "Docker system prune (images, containers, build cache)"),
    ("orbstack", "Orbstack prune"),
    ("cocoapods","CocoaPods cache clean --all"),
    ("spm",      "SwiftPM cache directory"),
    ("conda",    "Conda clean --all"),
    ("poetry",   "Poetry cache clear --all"),
    ("pipx",     "pipx cache and unused packages"),
    ("deno",     "Deno cache reload"),
    ("rustup",   "Unused Rust toolchains"),
    ("gradle",   "Gradle old version caches"),
    ("jetbrains","JetBrains IDE caches (old versions)"),
    ("trash",    "~/.Trash size (scan only)"),
    ("downloads","~/Downloads old files"),
];
```

**b)** Add module declarations in `src/cleaners/mod.rs`:

```rust
pub mod cargo;
pub mod rustup;
pub mod gradle;
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --test interactive targets_subcommand_includes_new_targets -- --nocapture`
Expected: PASS — targets now listed.

- [ ] **Step 5: Add match arms for each new target in `main.rs`**

Add after existing targets in the `Some(Commands::Clean { target }) => match target {` block. Each arm follows the `run_clean_target` pattern:

```rust
CleanTarget::Cargo { dry_run } => {
    run_clean_target("cargo", |dry| {
        cleaners::cargo::CargoCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
CleanTarget::Docker { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::docker(Box::new(SystemCommandRunner));
    run_clean_target("docker", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::Orbstack { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::orbstack(Box::new(SystemCommandRunner));
    run_clean_target("orbstack", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::CocoaPods { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::cocoapods(Box::new(SystemCommandRunner));
    run_clean_target("cocoapods", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::SwiftPM { dry_run } => {
    run_clean_target("spm", |dry| {
        cleaners::generic::GenericCleaner::spm_cache(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
CleanTarget::Conda { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::conda(Box::new(SystemCommandRunner));
    run_clean_target("conda", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::Poetry { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::poetry(Box::new(SystemCommandRunner));
    run_clean_target("poetry", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::Pipx { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::pipx(Box::new(SystemCommandRunner));
    run_clean_target("pipx", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::Deno { dry_run } => {
    let cleaner = cleaners::generic::GenericCleaner::deno(Box::new(SystemCommandRunner));
    run_clean_target("deno", |dry| cleaner.clean(dry), dry_run)?;
}
CleanTarget::Rustup { dry_run } => {
    run_clean_target("rustup", |dry| {
        cleaners::rustup::RustupCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
CleanTarget::Gradle { dry_run } => {
    run_clean_target("gradle", |dry| {
        cleaners::gradle::GradleCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
CleanTarget::JetBrains { dry_run } => {
    run_clean_target("jetbrains", |dry| {
        cleaners::gradle::JetBrainsCleaner::new(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
CleanTarget::Trash { dry_run } => {
    run_clean_target("trash", |dry| {
        cleaners::generic::GenericCleaner::trash(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
CleanTarget::Downloads { dry_run } => {
    run_clean_target("downloads", |dry| {
        cleaners::generic::GenericCleaner::downloads(&home, Box::new(SystemCommandRunner)).clean(dry)
    }, dry_run)?;
}
```

**Note:** The `all_cleaners()` function currently returns cleaners used for scan/TUI. New scanners may need to be added there. Simple CLI-delegation targets can skip `all_cleaners()` (they're used via direct clean calls). Cargo, Rustup, Gradle, JetBrains need to be added to `all_cleaners()` for scan support.

- [ ] **Step 6: Run build and test**

Run: `cargo build`
Expected: succeeds (new types defined, arms may still reference unimplemented structs — use `todo!()` stubs if needed).

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat: add CleanTarget variants and SUPPORTED_TARGETS for 14 new targets"
```

---

### Task 1: Easy CLI targets — Deno, Docker, Orbstack, CocoaPods, Conda, Poetry, Pipx

**Files:**
- Modify: `src/cleaners/generic.rs` — add factory methods for CLI-delegation targets
- Test: `tests/generic.rs`

All follow this exact pattern (use `bun`/`go`/`pip` as reference):

```rust
pub fn deno(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("deno", "deno", &["cache", "-r"], runner)
}
pub fn pipx(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("pipx", "pipx", &["cache", "purge"], runner)  // verify CLI flags
}
pub fn docker(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("docker", "docker", &["system", "prune", "-af"], runner)
}
pub fn orbstack(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("orbstack", "orb", &["prune"], runner)
}
pub fn cocoapods(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("cocoapods", "pod", &["cache", "clean", "--all"], runner)
}
pub fn conda(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("conda", "conda", &["clean", "--all", "-y"], runner)
}
pub fn poetry(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("poetry", "poetry", &["cache", "clear", "--all"], runner)
}
```

_Note: Verify each tool's actual CLI flags before implementation. Some may differ from the above._

**E2E test for one target (add to `tests/generic.rs`):**

```rust
#[test]
fn clean_deno_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // Do NOT install fake deno — test "not found" path
    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "deno"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("skipping") || stdout.contains("not found"));
}
```

Repeat pattern for docker, orbstack, cocoapods, conda, poetry, pipx.

- [ ] **Step 1-7**: TDD cycle for each: write failing test → implement → verify → commit.

- [ ] **Step 8: Commit all CLI delegation targets**

```bash
git add -A && git commit -m "feat: add deno, docker, orbstack, cocoapods, conda, poetry, pipx cleaners"
```

---

### Task 2: SwiftPM cache + Cargo registry (directory deletion)

**Files:**
- Modify: `src/cleaners/generic.rs` — add `spm_cache()`, `cargo_registry()`

**Implementation (directory deletion via `DeleteDirs`):**

```rust
pub fn spm_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let cache = home.join("Library/Caches/org.swift.swiftpm");
    Self {
        display_name: "spm",
        method: CleanMethod::DeleteDirs(vec![cache]),
        runner,
    }
}

pub fn cargo_registry(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let cache = home.join(".cargo/registry/cache");
    Self {
        display_name: "cargo-registry",
        method: CleanMethod::DeleteDirs(vec![cache]),
        runner,
    }
}
```

**E2E test (add to `tests/generic.rs`):**

```rust
#[test]
fn clean_spm_cache_dry_run() {
    let tmp = TempDir::new().unwrap();
    let spm_dir = tmp.path().join("Library/Caches/org.swift.swiftpm");
    std::fs::create_dir_all(&spm_dir).unwrap();
    std::fs::write(spm_dir.join("dummy"), b"x").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "spm", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    // dry-run: file should still exist
    assert!(spm_dir.join("dummy").exists(), "dry-run must not delete");
}
```

- [ ] **Step 1-7**: TDD cycle.
- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: add SwiftPM and Cargo registry cache cleaners"
```

---

### Task 3: Cargo target/ directory scan cleaner

**Files:**
- Create: `src/cleaners/cargo.rs`
- Modify: `src/cleaners/mod.rs`
- Test: `tests/cargo.rs`

**Module structure (follows `browser.rs` pattern):**

```rust
use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;

pub struct CargoCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl CargoCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self { home: home.to_path_buf(), runner }
    }

    /// Scans for `target/` directories up to max_depth=4 under HOME.
    /// Returns (path, size_in_bytes) pairs, excluding symlinks.
    fn find_target_dirs(home: &Path) -> Vec<(PathBuf, u64)> {
        let mut targets = vec![];
        for entry in walkdir::WalkDir::new(home)
            .max_depth(4)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let fname = entry.file_name().to_string_lossy();
            if fname == "target" && entry.file_type().is_dir() {
                // Avoid following symlinks within (cargo's own target/ could be a mount)
                let size = dir_size(entry.path());
                targets.push((entry.path().to_path_buf(), size));
            }
        }
        targets
    }
}

impl Cleaner for CargoCleaner {
    fn name(&self) -> &str { "cargo" }

    fn detect(&self) -> ScanResult {
        // Registry cache
        let reg = self.home.join(".cargo/registry/cache");
        let reg_size = if reg.exists() { dir_size(&reg) } else { 0 };

        // Target directories
        let targets = Self::find_target_dirs(&self.home);
        let target_size: u64 = targets.iter().map(|(_, s)| s).sum();

        let total = reg_size + target_size;
        ScanResult {
            name: self.name(),
            status: if total > 0 { ScanStatus::Pruneable(total) } else { ScanStatus::Clean },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        let mut freed: u64 = 0;

        // Clean registry cache
        let reg = self.home.join(".cargo/registry/cache");
        if reg.exists() {
            let size = dir_size(&reg);
            if dry_run {
                println!("[dry-run] would remove: {}", reg.display());
            } else {
                let path_str = reg.to_string_lossy();
                let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
                fs::remove_dir_all(&reg)?;
                freed += size;
                println!("Removed: {}", reg.display());
            }
        }

        // Clean target directories (user confirmation may be desired)
        let targets = Self::find_target_dirs(&self.home);
        for (path, size) in &targets {
            if dry_run {
                println!("[dry-run] would remove: {} ({})", path.display(), crate::format::format_bytes(*size));
            } else {
                let path_str = path.to_string_lossy();
                let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
                fs::remove_dir_all(path)?;
                freed += size;
                println!("Removed: {}", path.display());
            }
        }

        Ok(CleanResult { name: self.name(), bytes_freed: freed })
    }
}
```

**E2E test (`tests/cargo.rs`):**

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn cargo_detect_finds_target_dirs() {
    let tmp = TempDir::new().unwrap();
    let proj = tmp.path().join("my-project/target/debug");
    fs::create_dir_all(&proj).unwrap();
    fs::write(proj.join("deps.rlib"), b"x".repeat(4096)).unwrap();

    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cargo"), "scan must show cargo: {stdout}");
}

#[test]
fn cargo_clean_removes_registry_cache() {
    let tmp = TempDir::new().unwrap();
    let reg = tmp.path().join(".cargo/registry/cache/index.crates.io-xxx");
    fs::create_dir_all(&reg).unwrap();
    fs::write(reg.join("dummy.crate"), b"x").unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "cargo"])
        .output()
        .unwrap();
    assert!(output.status.success());
    // Registry cache root should be deleted
    assert!(!tmp.path().join(".cargo/registry/cache").exists(), "registry cache must be removed");
}
```

- [ ] **Step 1-7**: TDD cycle.
- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: add Cargo cleaner (registry + target dirs)"
```

---

### Task 4: Rustup toolchain cleaner

**Files:**
- Create: `src/cleaners/rustup.rs`
- Modify: `src/cleaners/mod.rs`
- Test: `tests/rustup.rs`

**Module structure (follows `mise.rs` pattern):**

```rust
pub struct RustupCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
}

impl RustupCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self { home: home.to_path_buf(), runner }
    }

    /// Parses `rustup toolchain list` output.
    /// Lines: "stable-aarch64-apple-darwin (default)", "nightly-2026-05-01-aarch64-apple-darwin"
    /// Active toolchain is marked with "(default)" or the currently selected one.
    fn parse_toolchains(stdout: &str) -> (Vec<String>, HashSet<String>) {
        let mut all = vec![];
        let mut active = HashSet::new();
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() { continue; }
            // Extract toolchain name (everything before first space)
            let name = trimmed.split_whitespace().next().unwrap_or("");
            if name.is_empty() { continue; }
            let is_active = trimmed.contains("(default)") || trimmed.contains("(override)");
            all.push(name.to_string());
            if is_active {
                active.insert(name.to_string());
            }
        }
        (all, active)
    }
}

impl Cleaner for RustupCleaner {
    fn name(&self) -> &str { "rustup" }

    fn detect(&self) -> ScanResult {
        if !self.runner.exists("rustup") {
            return ScanResult { name: self.name(), status: ScanStatus::NotFound };
        }
        let output = match self.runner.run("rustup", &["toolchain", "list"]) {
            Ok(o) => o,
            Err(_) => return ScanResult { name: self.name(), status: ScanStatus::NotFound },
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (all, active) = Self::parse_toolchains(&stdout);
        let unused: Vec<_> = all.iter().filter(|t| !active.contains(*t.as_str())).collect();
        // Size estimation: each toolchain is ~300MB
        let bytes = unused.len() as u64 * 300_000_000;
        ScanResult {
            name: self.name(),
            status: if bytes > 0 { ScanStatus::Pruneable(bytes) } else { ScanStatus::Clean },
        }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        if !self.runner.exists("rustup") {
            println!("rustup: not found, skipping");
            return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
        }
        let output = self.runner.run("rustup", &["toolchain", "list"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (all, active) = Self::parse_toolchains(&stdout);
        let mut freed: u64 = 0;

        for toolchain in &all {
            if active.contains(toolchain) { continue; }
            if dry_run {
                println!("[dry-run] would remove toolchain: {toolchain}");
            } else {
                self.runner.run("rustup", &["toolchain", "remove", toolchain])?;
                freed += 300_000_000; // estimated
                println!("Removed toolchain: {toolchain}");
            }
        }
        Ok(CleanResult { name: self.name(), bytes_freed: freed })
    }
}
```

**E2E test (`tests/rustup.rs`):**

```rust
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn install_fake_rustup(bin_dir: &Path, output: &str) {
    let script = format!("#!/bin/sh\necho '{}'", output.replace('\'', "'\\''"));
    fs::write(bin_dir.join("rustup"), script).unwrap();
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join("rustup"), fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn rustup_dry_run_shows_unused() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_rustup(&bin_dir,
        "stable-aarch64-apple-darwin (default)\nnightly-2026-05-01-aarch64-apple-darwin\n");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "rustup", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nightly"), "dry-run must show nightly: {stdout}");
    assert!(stdout.contains("dry-run"), "must be dry-run: {stdout}");
}
```

- [ ] **Step 1-7**: TDD cycle.
- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: add Rustup toolchain cleaner"
```

---

### Task 5: Gradle + JetBrains IDE cache cleaner

**Files:**
- Create: `src/cleaners/gradle.rs`
- Test: `tests/gradle.rs`

**Overview:** Two cleaners in one module:
- `GradleCleaner` — removes old version directories from `~/.gradle/caches/`
- `JetBrainsCleaner` — removes old IDE caches from `~/Library/Caches/JetBrains/`

Both follow the `browser.rs` "keep highest version" pattern.

**`GradleCleaner::find_old_versions`** — reads `~/.gradle/caches/` subdirectories, extracts version numbers (e.g. `8.10.1`, `8.12.0`), keeps the highest.

**`JetBrainsCleaner::find_old_versions`** — reads `~/Library/Caches/JetBrains/` subdirectories, extracts IDE name + version (e.g. `GoLand2024.3`, `IntelliJIdea2025.1`), keeps the highest version per IDE family.

- [ ] **Step 1: Write failing E2E tests** in `tests/gradle.rs` (follow existing patterns).
- [ ] **Step 2-7**: Implement and verify.
- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: add Gradle and JetBrains IDE cache cleaners"
```

---

### Task 6: Trash + Downloads reporter

**Files:**
- Modify: `src/cleaners/generic.rs` — add `trash()` and `downloads()`

**Implementation notes:**
- **Trash**: scan-only. `detect()` reports `dir_size("~/.Trash")`. `clean()` prints a warning "Use Finder to empty Trash" and returns 0.
- **Downloads**: `detect()` reports `dir_size("~/Downloads")`. `clean()` with `--dry-run` lists files older than 30 days. Real clean requires confirmation (use `cli.yes` flag or prompt).

**Add factory methods:**

```rust
pub fn trash(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let trash_dir = home.join(".Trash");
    Self {
        display_name: "trash",
        method: CleanMethod::DeleteDirs(vec![trash_dir]),
        runner,
    }
}

pub fn downloads(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let dl_dir = home.join("Downloads");
    Self {
        display_name: "downloads",
        method: CleanMethod::DeleteDirs(vec![dl_dir]),
        runner,
    }
}
```

**Override `detect` and `clean` behavior:** Since Trash and Downloads need special handling (scan-only, confirmation), create custom `Cleaner` implementations that delegate to GenericCleaner but override clean.

Actually, simplest approach: handle them directly in main.rs match arms, calling dir_size for detect and printing warnings for clean. No custom cleaner needed.

- [ ] **Step 1-7**: Implement detect/clean in the match arms directly.
- [ ] **Step 8: Commit**

```bash
git add -A && git commit -m "feat: add Trash and Downloads reporters"
```

---

### Task 7: Final verification

- [ ] **Step 1: Run full quality gates**

```bash
cargo fmt --check
cargo clippy --tests -- -D warnings
cargo test
```

Expected: all pass, 0 warnings, 0 failures.

- [ ] **Step 2: Verify the binary runs**

```bash
cargo run -- targets | head -20
```

Expected: all 14 new entries listed alongside existing targets.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "chore: final verification — all 14 new targets"
```
