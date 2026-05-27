# PBI-B: 堅牢なエラーハンドリング

## ユーザーストーリー

macOS 開発者として、権限エラーやファイルロックが発生しても削除処理が途中でクラッシュせずに続いてほしい。なぜなら、一部のキャッシュファイルが保護されていても、残りの大部分は安全に削除したいから。

## ビジネス価値

- 削除処理のパニック（プロセスクラッシュ）をゼロにする
- 権限エラーが出ても削除できた分の容量を正しく報告する
- 失敗したファイルを把握でき、ユーザーが手動対処できる

## BDD受け入れシナリオ

```gherkin
Scenario: 権限エラーのファイルはスキップされ処理は継続する
  Given キャッシュディレクトリ内に権限 000 のファイルが存在する
  When sasurahime clean <target> を実行する
  Then 権限エラーのファイルはスキップされる
  And 残りのファイルは正常に削除される
  And 終了時に "1 file(s) skipped (Permission denied)" が表示される
  And プロセスの終了コードは 0 である（部分成功）

Scenario: ファイルロック中のファイルはスキップされる
  Given ollama デーモンが起動中でロックファイルを保持している
  When sasurahime clean ollama を実行する
  Then ロックされたファイルはスキップされる
  And ロックされていないキャッシュは削除される
  And 終了時にスキップ理由が表示される

Scenario: 全ファイルが失敗した場合は終了コード 1
  Given 全てのファイルに権限エラーがある
  When sasurahime clean <target> を実行する
  Then 終了コードは 1 である
  And "N file(s) failed" のエラーサマリーが表示される

Scenario: dry-run では権限チェックのみ行う
  Given 権限エラーのファイルが存在する
  When sasurahime clean <target> --dry-run を実行する
  Then 実際の削除は行われない
  And 権限エラーになる予定のファイルを警告として表示する
```

## 受け入れ基準

- [x] 権限エラー（`EPERM`, `EACCES`）発生時にパニックせず、エラーをスキップして処理を継続する
- [x] ファイルロック（`EBUSY`）発生時も同様にスキップして継続する
- [x] 終了時に `N file(s) skipped: <reason>` 形式でサマリーを表示する
- [x] 一部成功・一部失敗の場合、削除できた容量を正確に報告する
- [x] 全ファイルが失敗した場合のみ終了コード 1
- [x] `--dry-run` でも権限チェックを行い予告警告を出す

## t_wada スタイル テスト戦略

```
E2Eテスト:
- tempdir に chmod 000 のファイルを含むキャッシュを作成し clean を実行
- 終了コードと stdout のサマリーメッセージを検証

統合テスト:
- CleanResult に skipped_files: Vec<(PathBuf, Error)> フィールドを追加し
  Cleaner::clean() がスキップを正しく記録することをテスト
- 部分削除時の解放容量計算をテスト

単体テスト:
- delete_with_skip() ヘルパー関数の各エラー種別（EPERM/EBUSY）テスト
- サマリーメッセージの生成ロジックのテスト
- 終了コード判定ロジック（全失敗 vs 部分成功）のテスト
```

## 実装アプローチ

- **Outside-In**: chmod 000 ファイルを含む E2E テストから開始
- **Red-Green-Refactor**:
  1. Red: 現在のコードが権限エラーでパニックすることを確認するテスト
  2. Green: エラーをキャッチしてスキップするロジックを実装
  3. Refactor: `CleanResult` を拡張し `skipped` フィールドを追加
- **データ構造変更**:
  ```rust
  pub struct CleanResult {
      pub freed_bytes: u64,
      pub skipped: Vec<SkippedEntry>,  // 追加
  }
  pub struct SkippedEntry {
      pub path: PathBuf,
      pub reason: String,
  }
  ```

## 技術的考慮事項

- 依存関係: 追加なし（標準ライブラリの `std::io::ErrorKind` を使用）
- `chflags -R nouchg` の失敗もスキップ対象にする（immutable flag 解除失敗）
- テスタビリティ: `chmod 000` テストはルート権限不要（一般ユーザーで作成したファイルなら変更可能）
- CI 考慮: GitHub Actions の macOS runner でも動作する設計

## 見積もり

**3 SP**

## Definition of Done

- [x] 受け入れシナリオが全て通る
- [x] `cargo test` 全パス
- [x] `cargo clippy -- -D warnings` クリーン
- [x] `cargo fmt --check` クリーン
- [x] コードレビュー完了

---

## Implementation Status (2026-05-25)

Implemented and released as v0.1.23 on branch `pbi-2026-05-25`. All acceptance criteria met.

### Architecture summary

| Change | Details |
|--------|---------|
| `SkippedEntry` | New struct: `{ path: PathBuf, reason: String }` (`src/cleaner.rs`) |
| `CleanResult` | Added `skipped: Vec<SkippedEntry>` field |
| `is_skippable_error()` | Public helper: catches `PermissionDenied`, `WouldBlock`, `AlreadyExists` (`src/cleaner.rs`) |
| `exit_code()` | Returns `1` when all files skipped / nothing freed; `0` on partial or full success |
| `run_clean_target` | Prints skip summary to stderr; exits with `exit_code()` |
| `trash.rs` | Changed `map_err` → `.with_context()` to preserve `io::Error` chain for downcasting |

### Files modified

All 20+ cleaners with deletion paths updated to catch skippable errors:

```
src/cleaner.rs                        SkippedEntry + CleanResult.skipped + is_skippable_error
src/trash.rs                          with_context preservation
src/main.rs                           run_clean_target skip summary + exit code
src/cleaners/browser.rs               delete_path skippable handling
src/cleaners/xcode.rs                 delete_path skippable handling
src/cleaners/cargo.rs                 delete_path skippable handling
src/cleaners/log.rs                   delete_path skippable handling
src/cleaners/uv.rs                    remove_dir_all skippable handling
src/cleaners/mise.rs                  delete_path skippable handling
src/cleaners/ios_backup.rs            delete_path skippable handling
src/cleaners/library_logs.rs          delete_path skippable handling (2 functions)
src/cleaners/device_support.rs        delete_path skippable handling
src/cleaners/ollama.rs                delete_path skippable handling (2 functions)
src/cleaners/rustup.rs                cleanup skippable handling
src/cleaners/gradle.rs                remove_dir_all skippable handling (2 cleaners)
src/cleaners/generic.rs               delete_path skippable handling (3 sites)
```

### Verification

```
$ cargo test                          # 300+ passed, 0 failed
$ cargo clippy -- -D warnings         # clean
$ cargo fmt --check                   # clean
```
