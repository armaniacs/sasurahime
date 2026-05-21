# Additional Simple Cleaners Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** Add 4 simple cleaners: VSCode extensions cache, Maven, Terraform, Flutter.

**Architecture:** All use `GenericCleaner`. VSCode extensions uses `DeleteDirs`. Others use `CommandWithDetectDir` (CLI + fallback) or `DeleteDirs`.

**Tech Stack:** Rust, GenericCleaner (existing)

---

## File Structure

| File | Change | Responsibility |
|------|--------|----------------|
| `src/cleaners/generic.rs` | Modify | Add `vscode_extensions()`, `maven()`, `terraform()`, `flutter()` |
| `src/main.rs` | Modify | Add 4 entries to `define_cleaners!` |
| `tests/generic.rs` | Modify | Add E2E tests |

---

### Task 1: VSCode extensions cache cleaner

**Pattern:** `DeleteDirs` (like `node-gyp`).

```rust
pub fn vscode_extensions(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let cache = home.join(".vscode/extensions");
    Self {
        display_name: "vscode-extensions",
        method: CleanMethod::DeleteDirs(vec![cache]),
        runner,
    }
}
```

**CLI:** `sasurahime clean vscode-extensions`

### Task 2: Maven local repository cleaner

**Pattern:** `CommandWithDetectDir`. `mvn dependency:purge-local-repository` or DeleteDirs fallback.

```rust
pub fn maven(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    Self {
        display_name: "maven",
        method: CleanMethod::CommandWithDetectDir {
            program: "mvn",
            args: &["dependency:purge-local-repository"],
            detect_dir: home.join(".m2/repository"),
        },
        runner,
    }
}
```

**CLI:** `sasurahime clean maven`

### Task 3: Terraform provider plugin cache cleaner

**Pattern:** `DeleteDirs` with `$TF_PLUGIN_CACHE_DIR` env var support.

```rust
pub fn terraform(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let cache = std::env::var("TF_PLUGIN_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".terraform.d/plugin-cache"));
    Self {
        display_name: "terraform",
        method: CleanMethod::DeleteDirs(vec![cache]),
        runner,
    }
}
```

**CLI:** `sasurahime clean terraform`

### Task 4: Flutter/Dart pub cache cleaner

**Pattern:** `CommandWithDetectDir`. `dart pub cache clean` or DeleteDirs fallback.

```rust
pub fn flutter(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    let cache = std::env::var("PUB_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".pub-cache"));
    Self {
        display_name: "flutter",
        method: CleanMethod::CommandWithDetectDir {
            program: "dart",
            args: &["pub", "cache", "clean"],
            detect_dir: cache,
        },
        runner,
    }
}
```

**CLI:** `sasurahime clean flutter`

---

### Task 5: CLI registration

Add all 4 to `define_cleaners!` in `src/main.rs`:

```rust
VscodeExtensions : "vscode-extensions" => "VS Code extensions cache";
(|home, _config| cleaners::generic::GenericCleaner::vscode_extensions(home, Box::new(SystemCommandRunner))),

Maven : "maven" => "Maven local repository (mvn dependency:purge-local-repository)";
(|home, _config| cleaners::generic::GenericCleaner::maven(home, Box::new(SystemCommandRunner))),

Terraform : "terraform" => "Terraform provider plugin cache";
(|home, _config| cleaners::generic::GenericCleaner::terraform(home, Box::new(SystemCommandRunner))),

Flutter : "flutter" => "Flutter/Dart pub cache (dart pub cache clean)";
(|home, _config| cleaners::generic::GenericCleaner::flutter(home, Box::new(SystemCommandRunner))),
```

---

### Task 6: E2E tests

Add not-found E2E tests for each (they all skip gracefully when the tool is absent):

```rust
#[test]
fn clean_vscode_extensions_not_found_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "vscode-extensions"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

// Same pattern for maven, terraform, flutter
```
