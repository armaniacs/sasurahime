# Command Timeout Manual Command Hint

Date: 2026-05-22

## Background

`colima prune --all` など時間のかかる外部コマンドが 30 秒のタイムアウトに達すると、現在の sasurahime は以下の問題を引き起こす:

1. **`[OK]` が誤表示される**: `with_spinner()` がクロージャの成否を確認せず常に `[OK]` を印字するため、タイムアウト失敗にもかかわらず `Cleaning colima... [OK]` と表示される
2. **エラーメッセージが不親切**: `command colima did not complete within 30s and was killed` とだけ表示され、ユーザーが次に何をすべきか分からない
3. **手動コマンドの案内がない**: ユーザーは別ターミナルで `colima prune --all` を再実行すれば良いことに気づきにくい

ユーザーが自分で別ターミナルで実行できるコマンドがあれば、それを案内するように改善する。

---

## 変更箇所

### 1. `src/command.rs` — タイムアウトエラーに手動コマンドヒントを追加

`SystemCommandRunner` に `run_with_timeout()` プライベートメソッドを追加し、既存の `run()` はそれに委譲する。

**`run_with_timeout()` のタイムアウト分岐:**

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

**`run()` の委譲:**

```rust
impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output> {
        self.run_with_timeout(program, args, COMMAND_TIMEOUT)
    }
}
```

`SystemCommandRunner` はユニット構造体のまま（フィールド追加なし）。`run_with_timeout` は `pub` ではなく同一モジュール内からのみアクセス可能。

**テスト:**

```rust
#[test]
fn timeout_error_includes_manual_command_hint() {
    let runner = SystemCommandRunner;
    // 短いタイムアウト(10ms)で即座にタイムアウトさせる
    let result = runner.run_with_timeout("sleep", &["60"], Duration::from_millis(10));
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("You can run this command manually"));
    assert!(err.contains("sleep 60"));
    assert!(err.contains("did not complete within"));
}
```

### 2. `src/progress.rs` — `with_spinner_result` を追加

`with_spinner` は Result を意識しない汎用関数として残し、Result を返す処理向けに `with_spinner_result` を新設する。

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

- `Ok` → `{msg} [OK]` を stderr に印字、`Result` をそのまま返す
- `Err` → `{msg} [FAILED]` を stderr に印字、`Err` を呼び出し元に伝播（エラー詳細は呼び出し元が表示）

**`with_spinner` を使い続ける箇所（常に `[OK]` で問題ない）:**

| ファイル | 用途 |
|---------|------|
| `scanner.rs` | `detect()` — エラーを返さない |
| `interactive.rs` スキャン | `detect()` — エラーを返さない |
| `progress.rs` テスト | `with_spinner_returns_value` |

**`with_spinner_result` に切り替える箇所:**

| ファイル | 行 | 現在 |
|---------|----|------|
| `interactive.rs` | 62, 156 | `with_spinner` + `clean()` |
| `main.rs` | 568 | `with_spinner` + `cleaner_fn` |
| `main.rs` | 679 | `with_spinner` + `clean()` |

変更はすべて関数名の置き換えのみ（`with_spinner` → `with_spinner_result`）。引数・戻り値の型は互換性がある。

### 3. テスト

| テスト | ファイル | 内容 |
|-------|---------|------|
| `timeout_error_includes_manual_command_hint` | `command.rs` | 10ms タイムアウトでエラーメッセージに手動コマンドヒントが含まれることを確認 |
| `with_spinner_result_prints_ok_on_success` | `progress.rs` | 成功時に `[OK]` が印字され値が返ることを確認 |
| `with_spinner_result_prints_failed_on_error` | `progress.rs` | 失敗時に `[FAILED]` が印字されエラーが伝播することを確認 |

既存の E2E/統合テストは fake コマンドが即座に返るため変更不要。

---

## 出力イメージ

### タイムアウト発生時（対話モード）

```
Select caches to clean  [space to toggle, enter to confirm]: colima               9.3 GB

Will free approximately 9.3 GB.
Proceed? [y/N] y
Cleaning colima... [FAILED]
Error: command `colima prune --all` did not complete within 30s and was killed.
You can run this command manually in another terminal:
  $ colima prune --all

Total freed: 0 B
```

### 成功時（従来通り）

```
Cleaning colima... [OK]
Freed: 9.3 GB

Total freed: 9.3 GB
```

### `--yes` モード（run_auto）

```
Cleaning colima... [FAILED]
Error cleaning colima: command `colima prune --all` did not complete within 30s and was killed.
You can run this command manually in another terminal:
  $ colima prune --all

Total freed: 0 B
```

---

## 対象となるクリーナー

`CommandRunner::run()` を使用する全クリーナー（約 20 種）に一律適用:

colima, docker, orbstack, brew, uv, bun, go, pip, npm, yarn, pnpm, pipx, deno, cocoapods, conda, poetry, flutter(dart), simulator(xcrun), rustup, gradle, maven(mvn), act など

各クリーナーはタイムアウトエラーメッセージに自身のコマンドラインを自動的に含むため、個別の設定は不要。
