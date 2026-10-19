# APFS snapshot 強化 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `apfs-snapshot` の誤検出と削除コマンドの誤りを直し、`--all` 一括削除、解放量の計測、権限不足時の sudo 案内を加える。

**Architecture:** `src/cleaners/apfs_snapshot.rs` に閉じた変更。sasurahime 自身は sudo を実行せず、失敗時に実行すべきコマンドを表示する。解放量は `fs2::available_space("/")` の削除前後の差で測る（snapshot 単体のサイズは `tmutil` / `diskutil apfs` から取得できないため）。

**Tech Stack:** Rust, `fs2`（既存依存）, `dialoguer`, `assert_cmd`。

## 調査で判明した既存不具合（本計画で修正する）

1. `tmutil listlocalsnapshots /` はスナップショットが無くても `Snapshots for disk /:` というヘッダ行を出す（実機確認済み）。現行の `parse_snapshot_names` は非空行を全て名前として扱うため、スナップショットが無くても `detect()` が `Pruneable` を返す。
2. 削除に `tmutil deletelocalsnapshot / <name>` を使っているが、正しい構文は `tmutil deletelocalsnapshots <date>`（date は `YYYY-MM-DD-HHMMSS`）で、root が必要。
3. 削除失敗は `log::error!` のみで、利用者に次の手が伝わらない。
4. `diskutil apfs list` は snapshot 単位のサイズを返さない。ゆえにサイズ計測は空き容量の差で代替する。

## File Structure

- Modify: `src/cleaners/apfs_snapshot.rs` — パース、削除、`--all`、空き容量計測
- Modify: `src/main.rs`（`CleanTarget::ApfsSnapshot` `:275`, `dry_run()` `:383`, dispatch `:882`）— `--all` の追加
- Modify: `tests/apfs_snapshot.rs` — E2E
- Modify: `docs/SUPPORTED.md` — 仕様の記載

---

### Task 1: ヘッダ行を除外し、date を取り出す

**Files:** Modify `src/cleaners/apfs_snapshot.rs`（`parse_snapshot_names` `:17-24`、テスト末尾）

- [ ] **Step 1: 失敗するテストを追加**（`mod tests` 内）

```rust
    #[test]
    fn parse_ignores_header_line() {
        let input = "Snapshots for disk /:\ncom.apple.TimeMachine.2026-05-10-120000.local\n";
        assert_eq!(
            parse_snapshot_names(input),
            vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()]
        );
    }

    #[test]
    fn parse_header_only_returns_empty() {
        assert!(parse_snapshot_names("Snapshots for disk /:\n").is_empty());
    }

    #[test]
    fn snapshot_date_extracts_timestamp() {
        assert_eq!(
            snapshot_date("com.apple.TimeMachine.2026-05-10-120000.local"),
            Some("2026-05-10-120000".to_string())
        );
        assert_eq!(snapshot_date("Snapshots for disk /:"), None);
        assert_eq!(snapshot_date("com.apple.os.update-ABCDEF"), None);
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p sasurahime apfs_snapshot -- --nocapture`
Expected: `snapshot_date` 未定義でコンパイルエラー。

- [ ] **Step 3: 実装**（`parse_snapshot_names` を置き換え）

```rust
const TM_PREFIX: &str = "com.apple.TimeMachine.";

/// Returns the `YYYY-MM-DD-HHMMSS` part of a Time Machine local snapshot name.
/// `tmutil deletelocalsnapshots` accepts only this date form.
pub fn snapshot_date(name: &str) -> Option<String> {
    let rest = name.trim().strip_prefix(TM_PREFIX)?;
    let date = rest.strip_suffix(".local").unwrap_or(rest);
    let valid = date.len() == 17
        && date
            .chars()
            .enumerate()
            .all(|(i, c)| if matches!(i, 4 | 7 | 10) { c == '-' } else { c.is_ascii_digit() });
    valid.then(|| date.to_string())
}

pub fn parse_snapshot_names(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| snapshot_date(l).is_some())
        .map(|l| l.to_string())
        .collect()
}
```

- [ ] **Step 4: 通過を確認**

Run: `cargo test -p sasurahime apfs_snapshot`
Expected: 既存テストを含め PASS（既存テストの名前は全て `com.apple.TimeMachine.<date>.local` 形式）。

- [ ] **Step 5: Commit**

```bash
git add src/cleaners/apfs_snapshot.rs
git commit -m "fix(apfs-snapshot): tmutil のヘッダ行を snapshot として扱わない"
```

### Task 2: 正しい削除コマンドと sudo 案内

**Files:** Modify `src/cleaners/apfs_snapshot.rs`（削除ループ `:128-136`）

方針: `deletelocalsnapshots <date>` を実行する。`Err` または非ゼロ終了なら失敗とし、実行すべき `sudo tmutil deletelocalsnapshots <date>` を stderr に出す。

- [ ] **Step 1: 失敗するテストを追加**

```rust
    use std::sync::Mutex;

    struct Recording {
        calls: Mutex<Vec<Vec<String>>>,
        delete_status: i32,
    }
    impl CommandRunner for Recording {
        fn run(&self, _: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|s| s.to_string()).collect());
            if args.contains(&"listlocalsnapshots") {
                return Ok(ok_output(
                    b"Snapshots for disk /:\ncom.apple.TimeMachine.2026-05-10-120000.local\n",
                ));
            }
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(self.delete_status << 8),
                stdout: vec![],
                stderr: b"must be root".to_vec(),
            })
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn delete_uses_date_and_reports_success() {
        let runner = Recording { calls: Mutex::new(vec![]), delete_status: 0 };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let failed = cleaner.delete_snapshots(&names);
        assert!(failed.is_empty());
    }

    #[test]
    fn delete_failure_returns_sudo_hint_list() {
        let runner = Recording { calls: Mutex::new(vec![]), delete_status: 1 };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
        let names = vec!["com.apple.TimeMachine.2026-05-10-120000.local".to_string()];
        let failed = cleaner.delete_snapshots(&names);
        assert_eq!(failed, vec!["2026-05-10-120000".to_string()]);
    }

    #[test]
    fn sudo_hint_lists_each_failed_date() {
        let hint = sudo_hint(&["2026-05-10-120000".to_string()]);
        assert!(hint.contains("sudo tmutil deletelocalsnapshots 2026-05-10-120000"));
    }
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test -p sasurahime apfs_snapshot`
Expected: `delete_snapshots` / `sudo_hint` 未定義でコンパイルエラー。

- [ ] **Step 3: 実装**（`impl ApfsSnapshotCleaner` に追加し、`clean` の削除ループを置き換え）

```rust
pub fn sudo_hint(failed_dates: &[String]) -> String {
    let mut s = String::from(
        "[apfs-snapshot] some snapshots could not be deleted (root privileges are required). Run:\n",
    );
    for d in failed_dates {
        s.push_str(&format!("  sudo tmutil deletelocalsnapshots {d}\n"));
    }
    s
}

impl ApfsSnapshotCleaner {
    /// Returns the dates that failed to delete.
    fn delete_snapshots(&self, names: &[String]) -> Vec<String> {
        let mut failed = vec![];
        for name in names {
            let Some(date) = snapshot_date(name) else { continue };
            match self.runner.run("tmutil", &["deletelocalsnapshots", &date]) {
                Ok(o) if o.status.success() => println!("[apfs-snapshot] deleted: {name}"),
                Ok(_) | Err(_) => failed.push(date),
            }
        }
        failed
    }
}
```

`clean` の `for name in &selected { ... }` ブロックを次に置き換える:

```rust
        let failed = self.delete_snapshots(&selected);
        if !failed.is_empty() {
            eprint!("{}", sudo_hint(&failed));
        }
```

- [ ] **Step 4: 通過を確認**

Run: `cargo test -p sasurahime apfs_snapshot`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
git add src/cleaners/apfs_snapshot.rs
git commit -m "fix(apfs-snapshot): deletelocalsnapshots <date> を使い、失敗時に sudo コマンドを案内"
```

### Task 3: `--all` による一括削除

**Files:** Modify `src/cleaners/apfs_snapshot.rs`（struct、`new`、`clean`）

`all=true` のときは対話選択を行わず、全件を削除する。非 TTY でも `--all` なら実行する（スクリプト利用のため）。`--dry-run` は従来どおり削除しない。

- [ ] **Step 1: 失敗するテストを追加**

```rust
    #[test]
    fn all_flag_deletes_without_terminal() {
        let cleaner = ApfsSnapshotCleaner::new(Box::new(TwoSnapshots)).with_all(true);
        let reporter = crate::progress::VerboseProgress::new();
        // Non-TTY under `cargo test`: must still run the deletion path, not skip.
        let result = cleaner.clean(false, &reporter).unwrap();
        assert_eq!(result.name, "apfs-snapshot");
    }

    #[test]
    fn all_flag_dry_run_never_calls_delete() {
        let runner = Recording { calls: Mutex::new(vec![]), delete_status: 0 };
        let cleaner = ApfsSnapshotCleaner::new(Box::new(runner)).with_all(true);
        let reporter = crate::progress::VerboseProgress::new();
        cleaner.clean(true, &reporter).unwrap();
    }
```

`all_flag_dry_run_never_calls_delete` で呼び出し履歴を検証するため、`Recording` を `Arc<Mutex<..>>` で共有する形にする（`calls: std::sync::Arc<Mutex<Vec<Vec<String>>>>`）。テスト末尾で `assert!(!calls.lock().unwrap().iter().any(|c| c.contains(&"deletelocalsnapshots".to_string())))` を確認する。

- [ ] **Step 2: 失敗を確認** — `with_all` 未定義でコンパイルエラー。

- [ ] **Step 3: 実装**

```rust
pub struct ApfsSnapshotCleaner {
    pub runner: Box<dyn CommandRunner>,
    all: bool,
}

impl ApfsSnapshotCleaner {
    pub fn new(runner: Box<dyn CommandRunner>) -> Self {
        Self { runner, all: false }
    }

    pub fn with_all(mut self, all: bool) -> Self {
        self.all = all;
        self
    }
}
```

`clean` 内の非 TTY 判定と選択部を次に変える:

```rust
        if !self.all && !std::io::stdin().is_terminal() {
            eprintln!(
                "[apfs-snapshot] not a terminal — skipping. Use `sasurahime clean apfs-snapshot` \
                 for interactive selection, or `--all` to delete everything."
            );
            return Ok(CleanResult { /* 既存と同じ空の結果 */ });
        }

        let selected = if self.all { names.clone() } else { self.interactive_select(&names)? };
```

- [ ] **Step 4: 通過を確認** — `cargo test -p sasurahime apfs_snapshot` で PASS。

- [ ] **Step 5: Commit**

```bash
git add src/cleaners/apfs_snapshot.rs
git commit -m "feat(apfs-snapshot): --all で全 snapshot を非対話で削除"
```

### Task 4: 空き容量の差で bytes_freed を計測

**Files:** Modify `src/cleaners/apfs_snapshot.rs`

- [ ] **Step 1: 失敗するテストを追加**

```rust
    #[test]
    fn freed_bytes_is_free_space_delta() {
        assert_eq!(freed_bytes(100, 350), 250);
        assert_eq!(freed_bytes(350, 100), 0);
    }
```

- [ ] **Step 2: 失敗を確認** — `freed_bytes` 未定義。

- [ ] **Step 3: 実装**

```rust
fn freed_bytes(before: u64, after: u64) -> u64 {
    after.saturating_sub(before)
}

fn root_free_space() -> u64 {
    fs2::available_space("/").unwrap_or(0)
}
```

`clean` の削除部を、削除前に `let before = root_free_space();`、削除後に `let after = root_free_space();` を取り、`bytes_freed: freed_bytes(before, after)` を返す形にする。「Snapshot size cannot be measured」のコメントは「purgeable 領域の反映が遅れ、値が実際より小さくなることがある」という WHY に置き換える。

- [ ] **Step 4: 通過を確認** — `cargo test -p sasurahime apfs_snapshot` で PASS。

- [ ] **Step 5: Commit**

```bash
git add src/cleaners/apfs_snapshot.rs
git commit -m "feat(apfs-snapshot): 空き容量の差で解放量を報告"
```

### Task 5: CLI の `--all` 配線と E2E

**Files:** Modify `src/main.rs`, `tests/apfs_snapshot.rs`

- [ ] **Step 1: 失敗する E2E を追加**（`tests/apfs_snapshot.rs`）

```rust
#[test]
fn clean_apfs_snapshot_all_dry_run_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = Command::cargo_bin("sasurahime")
        .unwrap()
        .env("HOME", tmp.path())
        .args(["clean", "apfs-snapshot", "--all", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
```

- [ ] **Step 2: 失敗を確認**

Run: `cargo test --test apfs_snapshot`
Expected: `--all` が未知の引数で FAIL。

- [ ] **Step 3: 実装**

`src/main.rs:275`:

```rust
    ApfsSnapshot {
        #[arg(long)]
        dry_run: bool,
        /// Delete every local snapshot without prompting
        #[arg(long)]
        all: bool,
    },
```

`:383` を `CleanTarget::IosBackup { dry_run } => *dry_run,` と `CleanTarget::ApfsSnapshot { dry_run, .. } => *dry_run,` に分ける。`:882` の分岐を:

```rust
                    CleanTarget::ApfsSnapshot { dry_run, all } => {
                        let cleaner = cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
                            SystemCommandRunner,
                        ))
                        .with_all(all);
```

- [ ] **Step 4: 全体確認**

Run: `cargo test && cargo clippy -- -D warnings && cargo fmt --check`
Expected: すべて成功。

- [ ] **Step 5: Commit**

```bash
git add src/main.rs tests/apfs_snapshot.rs
git commit -m "feat(apfs-snapshot): clean apfs-snapshot に --all を追加"
```

### Task 6: ドキュメント

**Files:** Modify `docs/SUPPORTED.md`（apfs-snapshot の節）

- [ ] **Step 1:** 最新仕様のみを記載する（経緯や移行履歴は書かない）。内容は次の4点: 削除は `tmutil deletelocalsnapshots <date>`、root が必要で失敗時は sudo コマンドを表示、`--all` は非対話で全件削除、解放量は空き容量の差で報告（反映遅延で小さく出ることがある）。
- [ ] **Step 2: Commit**

```bash
git add docs/SUPPORTED.md
git commit -m "docs(apfs-snapshot): --all と sudo 案内の仕様を追記"
```

## Self-Review

- 仕様の網羅: サイズ計測(Task 4、代替手段を明記)、`--all`(Task 3, 5)、sudo 案内(Task 2)。調査で見つけた既存不具合2件は Task 1, 2 で修正。
- 型の整合: `snapshot_date` / `sudo_hint` / `delete_snapshots` / `with_all` / `freed_bytes` を後続タスクの定義と一致させた。
- 注意点: `detect()` は snapshot の件数からサイズを出せないため `Pruneable(0)` のままになる。この点は今回変更しない。
- 実機の削除は root が必要なため自動テストでは行わず、fake runner で検証する。
