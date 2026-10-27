# colima 実解放修正 実装計画

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** colima の clean が検出した VM ディスク領域を実際に解放できるようにする(prune + fstrim の後続コマンド)。

**Architecture:** `CliCleaner` に後続コマンドのリストを追加し、メインコマンド実行後に実行する。colima はメインに `colima prune --all --force`、後続に `colima ssh -- sudo fstrim -av` を設定する。後続の失敗は非致命で、warning と remedy を表示して clean を続ける。解放量は既存の物理ブロック diff(`dir_size` = `blocks()*512`)がそのまま報告する。

**Tech Stack:** Rust、既存の `CommandRunner` モック(`crate::test_helpers::MockRunner`、テスト内ローカルモック)、assert_cmd + tempfile の E2E。

**スペック:** docs/superpowers/specs/2026-10-27-colima-reclaim-design.md

---

### Task 0: ブランチ作成

**Files:** なし(git 操作のみ)

- [ ] **Step 1: 現ブランチから修正ブランチを切る**

現ブランチ `feat/orphan-trash-ux` は READY(CI 緑、TTY 手元確認待ち)。hermetic PATH スイープを含むこのブランチから切り出すことで、E2E テストの既存パターン(`install_fake_tool`、hermetic PATH)をそのまま使える。

```bash
git checkout feat/orphan-trash-ux
git checkout -b fix/colima-reclaim
```

Expected: `Switched to a new branch 'fix/colima-reclaim'`

---

### Task 1: CliCleaner に後続コマンドを追加

**Files:**
- Modify: `src/cleaners/generic.rs:36-50`(構造体フィールド)
- Modify: `src/cleaners/generic.rs:68-85`(`new` の初期化)
- Modify: `src/cleaners/generic.rs:106-109`(ビルダー追加)
- Modify: `src/cleaners/generic.rs:609-624`(clean フロー、`Some(detect_dir)` 分岐のメインコマンド処理の後)
- Test: `src/cleaners/generic.rs`(末尾の `mod tests` 内)

- [ ] **Step 1: 失敗するテストを書く**

`src/cleaners/generic.rs` の `mod tests` 内(`MissingToolRunner` の定義より後)に追加:

```rust
    // ── PBI 44: CliCleaner extra commands (colima prune + fstrim) ──

    /// Succeeds on the first call, fails on the second (VM stopped).
    struct OkThenErr(std::sync::atomic::AtomicUsize);
    impl CommandRunner for OkThenErr {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                Ok(std::process::Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: vec![],
                    stderr: vec![],
                })
            } else {
                anyhow::bail!("failed to spawn `colima`: VM is not running")
            }
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    /// Succeeds and removes a canary file inside the detect dir, so the
    /// physical-size diff reports the effect of the follow-up command.
    struct ShrinkOnRun(PathBuf);
    impl CommandRunner for ShrinkOnRun {
        fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
            let _ = fs::remove_file(self.0.join("keep.txt"));
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(0),
                stdout: vec![],
                stderr: vec![],
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn clean_runs_extra_command_and_its_effect_counts_in_freed_bytes() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("keep.txt"), b"x").unwrap();
        let cleaner = CliCleaner::with_detect_dir(
            "test",
            "colima",
            &["prune", "--all", "--force"],
            dir.clone(),
            Box::new(ShrinkOnRun(dir)),
        )
        .with_extra_commands(vec![(
            "colima",
            &["ssh", "--", "sudo", "fstrim", "-av"],
            Some("start the VM to trim"),
        )]);
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter).unwrap();
        assert!(
            result.bytes_freed > 0,
            "the extra command's effect must count in the freed bytes"
        );
    }

    #[test]
    fn clean_extra_command_failure_is_non_fatal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = tmp.path().join("cache");
        fs::create_dir_all(&dir).unwrap();
        let cleaner = CliCleaner::with_detect_dir(
            "test",
            "colima",
            &["prune", "--all", "--force"],
            dir,
            Box::new(OkThenErr(std::sync::atomic::AtomicUsize::new(0))),
        )
        .with_extra_commands(vec![(
            "colima",
            &["ssh", "--", "sudo", "fstrim", "-av"],
            Some("start the VM to trim"),
        )]);
        let reporter = crate::progress::DeepSuppressReporter;
        let result = cleaner.clean(false, &reporter);
        assert!(result.is_ok(), "a failed extra command must not fail the clean");
    }
```

注: `Ordering` はファイル冒頭で既に `use std::sync::atomic::{AtomicBool, Ordering};` と import 済み。`PathBuf` も import 済み。

- [ ] **Step 2: テストが失敗することを確認**

```bash
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --lib clean_runs_extra_command 2>&1 | tail -5
```

Expected: FAIL(コンパイルエラー: `with_extra_commands` というメソッドはない)

- [ ] **Step 3: 最小実装を書く**

(1) 構造体にフィールドを追加(`src/cleaners/generic.rs:36-50`):

```rust
pub struct CliCleaner {
    display_name: &'static str,
    program: &'static str,
    args: &'static [&'static str],
    /// Commands run after the main one. Each entry is (program, args,
    /// failure remedy). A failure is non-fatal: it prints a warning and
    /// the remedy, then the clean continues.
    extra_commands: Vec<(&'static str, &'static [&'static str], Option<&'static str>)>,
    detect_dir: Option<PathBuf>,
    runner: Box<dyn CommandRunner>,
    /// Optional confirmation prompt shown before cleaning on an interactive TTY.
    confirm_message: Option<&'static str>,
    /// If true and the CLI tool is not found, fall back to deleting detect_dir directly.
    fallback_delete: bool,
    /// Sparse VM disk image that pruning inside the VM may not shrink.
    notice: Option<DiskImageNotice>,
    /// Risk classification (defaults to Moderate via `new`).
    risk: Risk,
}
```

(2) `new` の初期化子に `extra_commands: Vec::new(),` を追加:

```rust
        Self {
            display_name,
            program,
            args,
            extra_commands: Vec::new(),
            detect_dir: None,
            runner,
            confirm_message: None,
            fallback_delete: false,
            notice: None,
            risk: Risk::Moderate,
        }
```

(3) `with_risk` の後にビルダーを追加:

```rust
    /// Add commands run after the main one. Each entry is (program, args,
    /// failure remedy); the remedy prints when the command fails.
    fn with_extra_commands(
        mut self,
        cmds: Vec<(&'static str, &'static [&'static str], Option<&'static str>)>,
    ) -> Self {
        self.extra_commands.extend(cmds);
        self
    }
```

(4) clean フロー(`Some(detect_dir)` 分岐): メインコマンドの `self.runner.run(program, args)?` とその出力処理(`if !output.status.success() { ... }` ブロック)の直後、`let size_after = ...` の直前に挿入:

```rust
                for (extra_program, extra_args, remedy) in &self.extra_commands {
                    println!("$ {extra_program} {}", extra_args.join(" "));
                    match self.runner.run(extra_program, extra_args) {
                        Ok(output) if !output.status.success() => {
                            eprintln!(
                                "[{}] warning: `{extra_program} {}` exited with code {:?}",
                                self.display_name,
                                extra_program,
                                extra_args.join(" "),
                                output.status.code()
                            );
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            if !stderr.is_empty() {
                                eprintln!("[{}] stderr: {stderr}", self.display_name);
                            }
                            if let Some(remedy) = remedy {
                                eprintln!("[{}] {remedy}", self.display_name);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!(
                                "[{}] warning: `{extra_program} {}` failed: {e}",
                                self.display_name,
                                extra_program,
                                extra_args.join(" ")
                            );
                            if let Some(remedy) = remedy {
                                eprintln!("[{}] {remedy}", self.display_name);
                            }
                        }
                    }
                }
```

- [ ] **Step 4: テストが通ることを確認**

```bash
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --lib clean_runs_extra_command 2>&1 | grep "test result"
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --lib clean_extra_command_failure 2>&1 | grep "test result"
```

Expected: 両方 1 passed; 0 failed

- [ ] **Step 5: コミット**

```bash
git add src/cleaners/generic.rs
git commit -m "feat(generic): run extra commands after the main clean command

CliCleaner can now carry follow-up commands (program, args, failure
remedy). They run after the main command, their effect counts in the
freed-bytes diff, and a failure prints a warning plus the remedy
without failing the clean."
```

---

### Task 2: colima ビルダーの後続 fstrim + 確認メッセージ修正

**Files:**
- Modify: `src/cleaners/generic.rs:203-224`(`colima_prune`)
- Modify: `tests/generic.rs`(末尾に追加)
- Test: `src/cleaners/generic.rs`(`mod tests` 内)と `tests/generic.rs`

- [ ] **Step 1: 失敗するテストを書く(src 側)**

`mod tests` 内の `colima_advisory_reports_disk_images` の後に追加:

```rust
    #[test]
    fn colima_confirm_message_describes_the_actual_action() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = CliCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let msg = cleaner
            .confirm_message
            .expect("colima has a confirmation prompt");
        assert!(msg.contains("fstrim"), "{msg}");
        assert!(
            !msg.contains("delete ALL stopped"),
            "the prompt must not describe colima delete: {msg}"
        );
    }
```

- [ ] **Step 2: 失敗するテストを書く(E2E)**

`tests/generic.rs` の末尾に追加:

```rust
#[test]
fn clean_colima_runs_prune_then_fstrim() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    install_fake_tool(&bin_dir, "colima");

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["--permanent", "clean", "colima"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let calls = recorded_calls(&bin_dir, "colima");
    assert!(calls.contains("prune --all --force"), "{calls}");
    assert!(calls.contains("ssh -- sudo fstrim -av"), "{calls}");
}

#[test]
fn clean_colima_succeeds_when_fstrim_fails() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    // A colima that succeeds for prune but fails for ssh (VM stopped).
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"ssh\" ]; then\n  echo \"FATA[0000] colima is not running\" >&2\n  exit 1\nfi\necho \"$@\" >> \"{}\"\nexit 0\n",
        bin_dir.join("calls_colima.txt").display()
    );
    fs::write(bin_dir.join("colima"), &script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join("colima"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .args(["--permanent", "clean", "colima"])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "a failed fstrim must not fail the clean: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("fstrim -av"), "{stderr}");
}
```

- [ ] **Step 3: テストが失敗することを確認**

```bash
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --lib colima_confirm_message 2>&1 | tail -3
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --test generic clean_colima 2>&1 | grep "test result"
```

Expected: 両方 FAIL(confirm に fstrim がない、calls に ssh がない)

- [ ] **Step 4: colima ビルダーを修正**

`src/cleaners/generic.rs:203-224` を置換:

```rust
    pub fn colima_prune(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            confirm_message: Some(
                "Prunes cached Colima assets and asks the running VM to discard unused disk blocks (fstrim). No containers, images or volumes are deleted. Continue?",
            ),
            fallback_delete: true,
            ..Self::with_detect_dir(
                "colima",
                "colima",
                &["prune", "--all", "--force"],
                home.join(".colima"),
                runner,
            )
            // Prune only clears colima's downloaded-asset cache; the space
            // the detection measures lives in the VM's sparse disk images,
            // which shrink only when the VM discards unused blocks.
            // Non-fatal: with the VM stopped the remedy prints and the
            // prune-only freed bytes are still reported.
            .with_extra_commands(vec![(
                "colima",
                &["ssh", "--", "sudo", "fstrim", "-av"],
                Some(COLIMA_REMEDY),
            )])
            .with_risk(Risk::Risky)
        }
        .with_notice(DiskImageNotice::new(
            "colima",
            colima_candidates(home),
            COLIMA_REMEDY,
        ))
    }
```

- [ ] **Step 5: テストが通ることを確認**

```bash
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --lib colima 2>&1 | grep "test result"
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --test generic clean_colima 2>&1 | grep "test result"
```

Expected: 両方 PASS

- [ ] **Step 6: コミット**

```bash
git add src/cleaners/generic.rs tests/generic.rs
git commit -m "feat(colima): reclaim VM disk space with a follow-up fstrim

colima prune only clears downloaded-asset caches, so the clean freed
nothing even though the scan measured the VM disk. The clean now runs
colima ssh -- sudo fstrim -av after the prune, so the VM discards
unused blocks and the sparse image's physical size shrinks. The
confirmation prompt now describes this non-destructive action."
```

---

### Task 3: CHANGELOG と最終検証

**Files:**
- Modify: `CHANGELOG.md`([Unreleased] セクション)

- [ ] **Step 1: CHANGELOG に追記**

`CHANGELOG.md` の `## [Unreleased]` の下(既存の [Unreleased] エントリに続く位置)に追加:

```markdown
### Fixed

- colima: `clean` now reclaims what the scan measures. `colima prune --all --force` only clears downloaded-asset caches, so the run freed 0 B while the scan reported the VM disk. The clean now follows up with `colima ssh -- sudo fstrim -av`, which asks the running VM to discard unused blocks so the sparse disk image's physical size shrinks. The follow-up is non-fatal: with the VM stopped, the remedy is printed and the prune-only freed bytes are still reported. The confirmation prompt now describes this non-destructive action instead of `colima delete`.
```

- [ ] **Step 2: docs-charset 検証**

```bash
python3 /Users/yaar/.agents/skills/docs-charset/scripts/check_docs_chars.py CHANGELOG.md
```

Expected: `OK: no out-of-range characters`(exit 0)

- [ ] **Step 3: ローカル全体検証**

```bash
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo fmt --check && echo "fmt: CLEAN"
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo clippy --all-targets 2>&1 | grep -cE "^warning|^error"
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --tests 2>&1 | grep -E "test result: FAILED" 
CARGO_HOME=$TMPDIR/cargo-home-dsh cargo test --lib 2>&1 | tail -2
```

Expected:
- fmt: CLEAN
- clippy: 0
- `--tests`: test result: FAILED が lib 側の既知 TCC セットのみ(huggingface、library_logs、pre_commit、trash の 4 件)で、generic/colima 関連は含まれない
- `--lib`: 730+3 passed / 4 failed(既知 TCC セットのみ)

- [ ] **Step 4: コミットと push**

```bash
git add CHANGELOG.md
git commit -m "docs: note the colima fstrim follow-up in the changelog"
git push -u origin fix/colima-reclaim
```

- [ ] **Step 5: 手元での実機確認(ユーザー)**

VM が起動している状態で:

```bash
./target/release/sasurahime clean colima
```

Expected: `$ colima prune --all --force` と `$ colima ssh -- sudo fstrim -av` が実行され、`Freed:` に実際の解放量(物理割り当ての縮小分)が表示される。confirm プロンプトは新しい文言で表示される。

注: fstrim は非破壊(未使用ブロックの破棄のみ)だが、実行はユーザーの判断で行う。サンドボックス内では実行しない。
