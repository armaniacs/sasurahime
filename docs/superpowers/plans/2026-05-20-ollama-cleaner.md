# Ollama Model Cache Cleaner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Add an `ollama` cleaner that detects `~/.ollama/models/` size and interactively selects models for removal via `ollama rm`.

**Architecture:** Dedicated `OllamaCleaner` struct (like `LibraryLogsCleaner`). `detect()` reports `~/.ollama/models/` size. `clean()` queries `ollama list`, presents models with size tags, and runs `ollama rm <model>` for selected ones. Falls back to direct deletion if `ollama` is not in PATH.

**Tech Stack:** Rust, dialoguer (existing), CommandRunner (existing)

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/cleaners/ollama.rs` | Create | `OllamaCleaner` struct + Cleaner impl |
| `src/cleaners/mod.rs` | Modify | Add `pub mod ollama;` |
| `src/main.rs` | Modify | Add special variant + dispatch match arm |
| `tests/ollama.rs` | Create | E2E tests |

---

### Task 1: Implement OllamaCleaner

**Files:**
- Create: `src/cleaners/ollama.rs`
- Modify: `src/cleaners/mod.rs`

- [ ] **Step 1: Create `src/cleaners/ollama.rs`**

```rust
use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus};
use crate::command::CommandRunner;
use crate::format::dir_size;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::fs;

pub struct OllamaCleaner {
    models_dir: PathBuf,
    runner: Box<dyn CommandRunner>,
}

/// A model entry discovered by `ollama list`.
#[derive(Debug, Clone)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
}

impl OllamaCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            models_dir: home.join(".ollama/models"),
            runner,
        }
    }

    /// Runs `ollama list` and parses the output into model entries.
    /// Each line format: NAME    ID    SIZE    MODIFIED
    /// We extract name and size (e.g. "4.7 GB").
    pub fn list_models(&self) -> Result<Vec<OllamaModel>> {
        if !self.runner.exists("ollama") {
            // Fallback: use directory entries to guess model info
            return Ok(vec![]);
        }
        let output = self.runner.run("ollama", &["list"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut models = Vec::new();
        for line in stdout.lines().skip(1) { // skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[0].to_string();
                // Size is typically like "4.7GB" — parse it
                let size = parse_model_size(parts.get(2).unwrap_or(&"0B"));
                models.push(OllamaModel { name, size });
            }
        }
        Ok(models)
    }

    /// Returns total size of all models (from CLI if available, else from dir_size).
    fn total_size(&self) -> u64 {
        if let Ok(models) = self.list_models() {
            let cli_total: u64 = models.iter().map(|m| m.size).sum();
            if cli_total > 0 { return cli_total; }
        }
        if self.models_dir.exists() { dir_size(&self.models_dir) } else { 0 }
    }
}

/// Parses "4.7GB" → bytes, "234MB" → bytes, etc.
fn parse_model_size(s: &str) -> u64 {
    let s = s.trim();
    if let Some(n) = s.strip_suffix("GB") {
        let v: f64 = n.trim().parse().unwrap_or(0.0);
        (v * 1_073_741_824.0) as u64
    } else if let Some(n) = s.strip_suffix("MB") {
        let v: f64 = n.trim().parse().unwrap_or(0.0);
        (v * 1_048_576.0) as u64
    } else if let Some(n) = s.strip_suffix("KB") {
        let v: f64 = n.trim().parse().unwrap_or(0.0);
        (v * 1_024.0) as u64
    } else {
        0
    }
}

impl Cleaner for OllamaCleaner {
    fn name(&self) -> &'static str {
        "ollama"
    }

    fn detect(&self) -> ScanResult {
        let bytes = self.total_size();
        if bytes == 0 {
            return ScanResult { name: self.name(), status: ScanStatus::NotFound };
        }
        ScanResult { name: self.name(), status: ScanStatus::Pruneable(bytes) }
    }

    fn clean(&self, dry_run: bool) -> Result<CleanResult> {
        // Try CLI path
        if self.runner.exists("ollama") {
            let models = self.list_models()?;
            if models.is_empty() {
                if self.models_dir.exists() && dir_size(&self.models_dir) > 0 {
                    // CLI reports no models but dir has data → fallback to direct deletion
                    return self.clean_fallback(dry_run);
                }
                println!("[ollama] no models found");
                return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
            }

            if dry_run {
                println!("[ollama] dry-run: {} models", models.len());
                for m in &models {
                    println!("  would remove: {} ({})", m.name, crate::format::format_bytes(m.size));
                }
                return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
            }

            // Interactive selection
            let items: Vec<String> = models.iter().map(|m| {
                format!("{:<24}  {}", m.name, crate::format::format_bytes(m.size))
            }).collect();
            let defaults: Vec<bool> = vec![true; models.len()];

            println!("\nOllama models in ~/.ollama/models/:\n");
            let selections = dialoguer::MultiSelect::new()
                .items(&items)
                .defaults(&defaults)
                .interact()?;

            if selections.is_empty() {
                println!("[ollama] nothing selected");
                return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
            }

            let mut total: u64 = 0;
            for &i in &selections {
                let m = &models[i];
                self.runner.run("ollama", &["rm", &m.name])?;
                total += m.size;
                println!("[ollama] removed: {} (freed {})", m.name, crate::format::format_bytes(m.size));
            }
            return Ok(CleanResult { name: self.name(), bytes_freed: total });
        }

        // Fallback: direct deletion
        self.clean_fallback(dry_run)
    }
}

impl OllamaCleaner {
    fn clean_fallback(&self, dry_run: bool) -> Result<CleanResult> {
        let dir = &self.models_dir;
        if !dir.exists() {
            return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
        }
        let size = dir_size(dir);
        if dry_run {
            println!("[ollama] would remove: {} ({})", dir.display(), crate::format::format_bytes(size));
            return Ok(CleanResult { name: self.name(), bytes_freed: 0 });
        }
        let path_str = dir.to_string_lossy();
        let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
        fs::remove_dir_all(dir)?;
        println!("[ollama] removed: {}", dir.display());
        Ok(CleanResult { name: self.name(), bytes_freed: size })
    }
}
```

- [ ] **Step 2: Register module in `src/cleaners/mod.rs`**

```rust
pub mod ollama;
```

- [ ] **Step 3: Build**

Run: `cargo build 2>&1`
Expected: Build succeeds.

- [ ] **Step 4: Add unit tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandRunner, SystemCommandRunner};

    struct MockOllamaRunner {
        list_output: String,
    }
    impl CommandRunner for MockOllamaRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
            assert_eq!(program, "ollama");
            if args == ["list"] {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: self.list_output.as_bytes().to_vec(),
                    stderr: vec![],
                })
            } else if args.first() == Some(&"rm") {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: vec![],
                    stderr: vec![],
                })
            } else {
                panic!("unexpected args: {args:?}");
            }
        }
        fn exists(&self, program: &str) -> bool { program == "ollama" }
    }

    #[test]
    fn list_models_parses_ollama_output() {
        let output = "NAME\tID\tSIZE\tMODIFIED\nllama3.2:3b\tabc123\t2.0GB\t2 days ago\n";
        let runner = MockOllamaRunner { list_output: output.to_string() };
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = OllamaCleaner::new(tmp.path(), Box::new(runner));
        let models = cleaner.list_models().unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].name, "llama3.2:3b");
        assert_eq!(models[0].size, (2.0_f64 * 1_073_741_824.0) as u64);
    }

    #[test]
    fn parse_model_size_gb() {
        assert_eq!(parse_model_size("4.7GB"), (4.7_f64 * 1_073_741_824.0) as u64);
    }

    #[test]
    fn parse_model_size_mb() {
        assert_eq!(parse_model_size("234MB"), (234.0_f64 * 1_048_576.0) as u64);
    }
}
```

---

### Task 2: CLI registration

- [ ] **Step 1: Add special variant to `define_cleaners!` in `src/main.rs`**

Add in the special variants section (after `Trash`):

```rust
/// Analyze and clean Ollama model cache
Ollama {
    #[arg(long)]
    dry_run: bool,
},
```

- [ ] **Step 2: Update `extra_targets()`**

```rust
("ollama", "Ollama model cache"),
```

- [ ] **Step 3: Update `impl CleanTarget` methods**

Add `CleanTarget::Ollama { .. } => "ollama"` to `command_name()`.
Add `CleanTarget::Ollama { dry_run } => *dry_run` to `dry_run()`.

- [ ] **Step 4: Add dispatch match arm**

Inside the special-targets block in `main()`:

```rust
CleanTarget::Ollama { dry_run } => {
    let cleaner = cleaners::ollama::OllamaCleaner::new(&home, Box::new(SystemCommandRunner));
    run_clean_target("ollama", move |dry| cleaner.clean(dry), dry_run)?;
}
```

- [ ] **Step 5: Add to `all_cleaners()`**

```rust
Box::new(cleaners::ollama::OllamaCleaner::new(home, Box::new(SystemCommandRunner))),
```

---

### Task 3: E2E tests

- [ ] **Step 1: Create `tests/ollama.rs`**

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

#[test]
fn clean_ollama_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "ollama"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn clean_ollama_dry_run_no_models_shows_nothing() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let script = "#!/bin/sh\necho 'NAME ID SIZE MODIFIED'\nexit 0\n";
    fs::write(bin_dir.join("ollama"), script).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::PermissionsExt::set_permissions(
        bin_dir.join("ollama"), std::fs::Permissions::from_mode(0o755)
    ).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["clean", "ollama", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn scan_shows_ollama_in_output() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".ollama/models/blobs")).unwrap();
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ollama"), "scan should include ollama:\n{stdout}");
}
```
