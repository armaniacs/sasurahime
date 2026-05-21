# Command Timeout Manual Command Hint — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a command-based cleaner (colima, docker, brew, etc.) times out (>30s), print a `[FAILED]` status (instead of misleading `[OK]`) and tell the user what command to run manually.

**Architecture:** 4-file change. `command.rs` — extract `run_with_timeout()` private method from `run()` and add manual command hint to the timeout error. `progress.rs` — add `with_spinner_result()` that prints `[FAILED]` on error. `interactive.rs` + `main.rs` — swap `with_spinner` to `with_spinner_result` for clean operations (scan stays unchanged).

**Tech Stack:** Rust, anyhow, wait-timeout, indicatif

**Spec:** `docs/superpowers/specs/2026-05-22-command-timeout-hint-design.md`

**Actual commits:**

```
7b51a97 style: apply cargo fmt to new timeout hint and with_spinner_result callers
e3049e5 feat: switch clean operations to with_spinner_result for accurate [OK]/[FAILED]
63503b4 test(progress): rename with_spinner_result tests to reflect what they actually test
d2f66a1 feat(progress): add with_spinner_result for error-aware spinner output
5bdbfb3 fix(command): trim leading whitespace from timeout error continuation lines
6de20cf feat(command): add run_with_timeout with manual command hint on timeout
1d03c83 docs: add design spec for command timeout manual command hint
```

---

### Task 1: Add `run_with_timeout` and timeout error hint in `command.rs`

**Files:**
- Modify: `src/command.rs:1-108`

- [x] **Step 1: Read the current file**

Read `src/command.rs` — confirmed content matches expectations.

- [x] **Step 2: Add `run_with_timeout` private method to `SystemCommandRunner`**

Implemented `impl SystemCommandRunner { fn run_with_timeout(...) }` + changed `impl CommandRunner for SystemCommandRunner { fn run() { self.run_with_timeout(...) } }`.

**Deviation:** The error message had leading whitespace from Rust's `\` string line continuation (`\n\` followed by indented text). Fixed in a follow-up commit `5bdbfb3` by removing the leading whitespace on continuation lines.

Final correct code in the `None` branch:

```rust
None => {
    let _ = child.kill();
    let _ = child.wait();
    let cmd_str = if args.is_empty() {
        program.to_string()
    } else {
        format!("{} {}", program, args.join(" "))
    };
    anyhow::bail!(
        "command `{cmd_str}` did not complete within {}s and was killed.\n\
You can run this command manually in another terminal:\n  $ {cmd_str}",
        timeout.as_secs()
    );
}
```

Key differences from original `run()`:
- Takes `timeout: Duration` parameter instead of using `COMMAND_TIMEOUT` directly
- `cmd_str` is constructed from program + args
- Error message ends with `You can run this command manually...`
- `\n\` continuation lines start at column 0 (no leading whitespace)

- [x] **Step 3: Add the timeout error format test**

```rust
    #[test]
    fn timeout_error_includes_manual_command_hint() {
        let runner = SystemCommandRunner;
        let result = runner.run_with_timeout("sleep", &["60"], Duration::from_millis(10));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("You can run this command manually"), ...);
        assert!(err.contains("sleep 60"), ...);
        assert!(err.contains("did not complete within"), ...);
    }
```

- [x] **Step 4: Run the new test**

```text
$ cargo test timeout_error_includes_manual_command_hint -- --nocapture
test command::tests::timeout_error_includes_manual_command_hint ... ok
```

Result: **PASS** (completed in ~10ms, all 3 assertions passed)

- [x] **Step 5: Run all command.rs tests**

```text
$ cargo test -p sasurahime command::
test command::tests::run_long_command_respects_timeout ... ok
test command::tests::run_tool_not_found_returns_error ... ok
test command::tests::run_successful_command_returns_output ... ok
test command::tests::run_captures_stderr ... ok
test command::tests::timeout_error_includes_manual_command_hint ... ok
```

Result: **all 5 passed**

- [x] **Step 6: Commit**

```bash
git add src/command.rs
git commit -m "feat(command): add run_with_timeout with manual command hint on timeout"
```

Commit: `6de20cf`

**Follow-up fix commit:** `5bdbfb3 fix(command): trim leading whitespace from timeout error continuation lines`

---

### Task 2: Add `with_spinner_result` in `progress.rs`

**Files:**
- Modify: `src/progress.rs` — add function + tests

- [x] **Step 1: Add `with_spinner_result` function**

```rust
pub fn with_spinner_result<T, E: std::fmt::Display>(
    msg: &str,
    f: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let pb = ProgressBar::new_spinner();
    pb.set_style(spinner_style().clone());
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    let result = f();
    pb.finish_and_clear();
    if result.is_ok() {
        eprintln!("{msg} [OK]");
    } else {
        eprintln!("{msg} [FAILED]");
    }
    result
}
```

- [x] **Step 2: Add tests for `with_spinner_result`**

**Deviation:** Test names were originally `with_spinner_result_prints_ok_on_success` / `with_spinner_result_prints_failed_on_error`, but since they don't capture stderr to verify printing (only check return values), they were renamed to `with_spinner_result_returns_ok` / `with_spinner_result_returns_error` in commit `63503b4`.

Final test code:

```rust
    #[test]
    fn with_spinner_result_returns_ok() {
        let result: Result<i32, String> = with_spinner_result("test", || Ok(42));
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn with_spinner_result_returns_error() {
        let result: Result<i32, &str> = with_spinner_result("test", || Err("boom"));
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "boom");
    }
```

- [x] **Step 3: Run progress.rs tests**

```text
$ cargo test -p sasurahime progress::
19 tests passed (17 existing + 2 new)
```

Result: **all 19 passed**

- [x] **Step 4: Commit**

```bash
git add src/progress.rs
git commit -m "feat(progress): add with_spinner_result for error-aware spinner output"
```

Commit: `d2f66a1`

**Follow-up fix commit:** `63503b4 test(progress): rename with_spinner_result tests to reflect what they actually test`

---

### Task 3: Update callers — `interactive.rs` and `main.rs`

**Files:**
- Modify: `src/interactive.rs:62` and `src/interactive.rs:156`
- Modify: `src/main.rs:568` and `src/main.rs:679`

4 substitutions, all identical in nature: `with_spinner` → `with_spinner_result` for clean operations only. Scan operations (`detect()` calls) remain on `with_spinner`.

- [x] **Step 1: Update `interactive.rs` — `run_auto` clean call**

```rust
let result = crate::progress::with_spinner_result(&format!("Cleaning {}...", name), || {
    cleaners[i].clean(false, &reporter)
});
```

- [x] **Step 2: Update `interactive.rs` — `run_interactive` clean call**

```rust
let result = crate::progress::with_spinner_result(&format!("Cleaning {}...", name), || {
    cleaners[cleaner_idx].clean(false, &reporter)
});
```

- [x] **Step 3: Update `main.rs` — `run_clean_target`**

```rust
    crate::progress::with_spinner_result(&format!("Cleaning {label}..."), || {
        cleaner_fn(dry_run, reporter)
    })?
```

- [x] **Step 4: Update `main.rs` — caches loop**

```rust
                    match crate::progress::with_spinner_result(
                        &format!("Cleaning {}...", c.name()),
                        || c.clean(dry_run, reporter.as_ref()),
                    ) {
```

- [x] **Step 5: Build and run all tests**

```text
$ cargo build
Finished dev profile (no warnings)

$ cargo test
170 passed, 0 failed  (unit tests)
 + integration tests all passed
```

Result: **clean build, all tests pass**

- [x] **Step 6: Commit**

```bash
git add src/interactive.rs src/main.rs
git commit -m "feat: switch clean operations to with_spinner_result for accurate [OK]/[FAILED]"
```

Commit: `e3049e5`

---

### Task 4: Final verification

- [x] **Step 1: Run full test suite**

```text
$ cargo test
170 passed; 0 failed; 0 ignored
+ 16 integration test suites, all passed
```

Result: **all tests pass**

- [x] **Step 2: Run clippy**

```text
$ cargo clippy -- -D warnings
   Completed with zero warnings
```

Result: **no warnings**

- [x] **Step 3: Run format check**

Initial run found 2 formatting issues (multi-line assert arg in `command.rs`, indentation in `main.rs`). Fixed with `cargo fmt`.

```text
$ cargo fmt --check
(no output — clean)
```

Result: **clean**

- [x] **Step 4: Verify the changed files**

```bash
git diff --stat 1d03c83..HEAD
```

Actual output:

```
 src/command.rs     | 44 ++++++++++++++++++++++++++++++++++++++------
 src/interactive.rs |  4 ++--
 src/main.rs        |  4 ++--
 src/progress.rs    | 31 +++++++++++++++++++++++++++++++
 4 files changed, 73 insertions(+), 10 deletions(-)
```
