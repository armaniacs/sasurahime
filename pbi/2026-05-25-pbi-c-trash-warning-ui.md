# PBI-C: ゴミ箱移動の警告UI

## ユーザーストーリー

macOS 開発者として、キャッシュをゴミ箱に移動した後に「なぜディスクの空き容量が増えないのか」を理解したい。なぜなら、ゴミ箱を空にするまでストレージは解放されないことを知らないと、ツールが機能していないと誤解するから。

## ビジネス価値

- ユーザーの混乱（「掃除したのに空き容量が増えない」）をゼロにする
- ゴミ箱操作後のサポート問い合わせ・Issueを削減する
- 特に大容量ファイル（XcodeデータやAIモデル）移動後の誤解を防ぐ

## BDD受け入れシナリオ

```gherkin
Scenario: ゴミ箱移動後に警告メッセージが表示される
  Given sasurahime が trash クレートを使ってファイルをゴミ箱へ移動する
  When ゴミ箱移動が完了する
  Then "Note: Files moved to Trash. Run 'Empty Trash' to free disk space." が表示される
  And 警告は削除後・合計容量表示の直後に出力される

Scenario: 1GB以上の大容量移動では強調警告が出る
  Given 合計 1GB 以上のファイルをゴミ箱に移動しようとしている
  When 確認プロンプトを表示する
  Then "⚠ Large files will be moved to Trash (not immediately freed)." の警告が確認前に出る
  And 通常メッセージよりも目立つ形式（stderr または色付き）で表示される

Scenario: dry-run では警告は表示されない
  Given --dry-run で実行する
  When ゴミ箱移動のシミュレーションが完了する
  Then ゴミ箱警告メッセージは表示されない
  And "Would move N files to Trash" のような dry-run 固有メッセージのみ出る
```

## 受け入れ基準

- [x] ゴミ箱移動が完了した後、必ず警告メッセージを表示する
- [x] 合計移動サイズが 1GB 以上の場合、確認プロンプトの前に強調警告を出す
- [x] `--dry-run` 時は警告を出さない
- [x] `--yes`（非インタラクティブ）時も警告は表示する（stdout に出力）
- [x] 警告は英語で表示する（他メッセージとの統一）

## t_wada スタイル テスト戦略

```
E2Eテスト:
- tempdir を使い clean 実行後の stdout に警告文字列が含まれることを assert
- 1GB 未満と 1GB 以上で表示される警告の違いをテスト

統合テスト:
- CleanResult に uses_trash: bool フィールドを持たせ、
  main の出力ロジックが警告を挿入することをテスト

単体テスト:
- format_trash_warning(freed_bytes: u64) -> Option<String> の純関数テスト
- 閾値（1GB）前後の境界値テスト
```

## 実装アプローチ

- **Outside-In**: stdout に警告文字列が含まれる E2E テストから開始
- **Red-Green-Refactor**:
  1. Red: 現在は警告が出ないため E2E テストが落ちる
  2. Green: `main.rs` の出力ロジックに警告挿入を追加
  3. Refactor: `format_trash_warning()` ヘルパーに抽出
- **閾値定数**: `const LARGE_TRASH_THRESHOLD_BYTES: u64 = 1024 * 1024 * 1024;`

## 技術的考慮事項

- 依存関係: 追加なし
- `trash` クレートを使う全クリーナーに適用（`CleanResult::uses_trash` で判定）
- テスタビリティ: 純関数 `format_trash_warning` で容易にテスト可能
- macOS 固有: `trash` クレートは macOS の Finder ゴミ箱 API を使用しているため Linux では不要

## 見積もり

**1 SP**

## Definition of Done

- [x] 受け入れシナリオが全て通る
- [x] `cargo test` 全パス
- [x] `cargo clippy -- -D warnings` クリーン
- [x] `cargo fmt --check` クリーン
- [x] コードレビュー完了

---

## Implementation Status (2026-05-25)

Implemented on branch `pbi-2026-05-25`. All acceptance criteria met.

### Architecture summary

| Change | Details |
|--------|---------|
| `CleanResult.uses_trash` | New `bool` field — set `true` when cleaner uses `delete_path()` (macOS Trash) |
| `format_trash_warning()` | Returns `"Note: Moved X to Trash. Run 'Empty Trash'..."` when applicable |
| `format_large_trash_warning()` | Returns `"⚠ Large files will be moved to Trash (not immediately freed)."` for ≥1GB |
| `LARGE_TRASH_THRESHOLD_BYTES` | Constant: 1 GiB |
| `run_clean_target` pre-clean | Shows trash notice before start when `is_trash_mode() && !dry_run` |
| `run_clean_target` post-clean | Shows size-specific trash note after clean result |

### Trash-warning cleaners

Cleaners that set `uses_trash: true`:
- browser, xcode, cargo, log, mise, ios_backup, library_logs, device_support, ollama (fallback), generic (DeleteDirs + fallback)

Cleaners that set `uses_trash: false` (CLI-only or `remove_dir_all`):
- uv, brew, apfs_snapshot, rustup, gradle

### Test coverage

- **7 unit tests** in `src/cleaner.rs`: both warning functions tested with None cases (no bytes, not trash, dry-run) and return cases (normal size, large threshold)
- **3 E2E tests** in `tests/trash.rs`: trash note on clean, suppressed with `--permanent`, suppressed with `--dry-run`

### Verification

```
$ cargo test                          # 350+ passed, 0 failed
$ cargo clippy -- -D warnings         # clean
$ cargo fmt --check                   # clean
```
