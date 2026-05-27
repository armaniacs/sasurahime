# PBI-G: sasurahime stats（削除履歴ログ + 統計コマンド）

## ユーザーストーリー

macOS 開発者として、過去に sasurahime が解放したディスク容量の履歴を振り返りたい。なぜなら、「累計で何GB解放したか」が見えることでツールへの信頼感が増し、定期実行のモチベーションになるから。

## ビジネス価値

- ツールの貢献度が可視化され継続利用を促進する
- 過去の削除履歴で「いつ何を消したか」が把握できる
- トラブル時（「あのキャッシュ消したっけ？」）の参照先になる

## 実装ステータス

**全機能: ✅ 完了**

- `sasurahime stats` サブコマンド — ✅ 実装済み
- `src/history.rs` モジュール — ✅ HistoryEntry, append_history (atomic), load_history, compute_stats, format_stats
- `~/.local/share/sasurahime/history.json` の自動記録 — ✅ run_clean_target, run_auto, run_interactive, Caches ハンドラ
- E2E/統合/単体テスト — ✅ 15 テスト（6 E2E + 9 unit）

### 依存関係（解決済み）

PBI-G の前提条件となる PBI は全て完了:

| PBI | 状態 | 依存理由 |
|-----|------|---------|
| PBI-C (Trash warning) | ✅ 完了 | stats でゴミ箱モードの割合表示に利用可能 |
| PBI-E (config.toml) | ✅ 完了 | stats の出力形式設定等に利用可能 |
| PBI-F (--yes flag) | ✅ 完了 | stats の履歴が --yes 実行でも記録される必要あり |

## BDD 受け入れシナリオ

```gherkin
Scenario: clean 実行後に履歴が記録される
  Given ~/.local/share/sasurahime/history.json が存在しない
  When sasurahime clean uv を実行し 500MB を解放する
  Then history.json が作成される
  And 実行日時・クリーナー名・解放バイト数が記録される

Scenario: sasurahime stats で統計が表示される
  Given 3回の clean が実行された履歴が存在する
  When sasurahime stats を実行する
  Then 累計解放容量（例: "Total freed: 12.5 GB"）が表示される
  And 実行回数（例: "Runs: 3"）が表示される
  And 直近の実行一覧（日付・クリーナー・解放量）がテーブル表示される

Scenario: 履歴が存在しない場合は空の統計を表示する
  Given history.json が存在しない
  When sasurahime stats を実行する
  Then "No history yet. Run 'sasurahime clean' to get started." が表示される
  And 終了コードは 0 である

Scenario: 履歴ファイルが壊れていても graceful に動作する
  Given history.json の JSON が壊れている
  When sasurahime stats を実行する
  Then "Warning: history file corrupted, starting fresh." と表示される
  And 終了コードは 0 である（クラッシュしない）

Scenario: sasurahime stats --last N で直近 N 件に絞れる
  Given 20件の履歴が存在する
  When sasurahime stats --last 5 を実行する
  Then 直近 5 件のみ表示される
```

## 受け入れ基準

- [x] `clean` / `clean --yes` 実行後に `~/.local/share/sasurahime/history.json` へ履歴を追記する
- [x] 記録内容: `{ timestamp, cleaner, freed_bytes, skipped_count }`
- [x] `sasurahime stats` で累計解放量・実行回数・直近一覧を表示する
- [x] `sasurahime stats --last N` で直近 N 件に絞れる
- [x] 履歴なし時は案内メッセージを表示（終了コード 0）
- [x] 履歴ファイル破損時は警告を出して正常終了（クラッシュしない）
- [x] `--dry-run` 時は履歴を記録しない

## t_wada スタイル テスト戦略

```
E2Eテスト:
- clean 実行後に history.json が作成されていることを確認
- history.json の内容を JSON パースして freed_bytes を検証
- stats コマンドの stdout に "Total freed:" が含まれることを確認

統合テスト:
- HistoryWriter::append(entry: &HistoryEntry) のテスト（tempdir 使用）
- HistoryReader::load(path: &Path) のパース・集計テスト
- 破損ファイルの graceful 処理テスト

単体テスト:
- format_stats(entries: &[HistoryEntry]) -> String の純関数テスト
- format_duration_ago(timestamp) の表示テスト（"2 days ago" など）
- HistoryEntry の JSON シリアライズ/デシリアライズテスト
```

## 実装アプローチ

- **Outside-In**: `clean` 後に `history.json` が存在する E2E テストから開始
- **Subagent-Driven Development**: 1 タスクで実装 → 2 段階レビュー（spec → quality）
- **Red-Green-Refactor**: TDD サイクル
  1. Red: 履歴未記録の E2E テスト
  2. Green: `history.rs` モジュール実装 + main.rs/interactive.rs への hook
  3. Refactor: `default_history_dir()` 抽出、アトミック書き込みパターン
- **データ形式**:
  ```json
  [
    {
      "timestamp": "2026-05-25T10:30:00+09:00",
      "cleaner": "uv",
      "freed_bytes": 524288000,
      "skipped_count": 0
    }
  ]
  ```

## 技術的考慮事項

- **依存関係**: `serde_json = "1"`, `chrono = { version = "0.4", features = ["serde"] }` — 新規追加
- **アトミック書き込み**: `history.json.tmp` に書き込んでから `fs::rename` で置き換え。同一ファイルシステム上のリネームはアトミック
- **履歴ディレクトリ**: `~/.local/share/sasurahime/` — 初回書き込み時に `create_dir_all` で自動作成
- **stats --last N**: デフォルト 10件。`format_stats_last(summary, last)` で N 件に絞って表示
- **hook 箇所**: `run_clean_target()` (main.rs), `run_auto()` (interactive.rs), `run_interactive()` (interactive.rs), `CleanTarget::Caches` ループ (main.rs) — 全 4 箇所
- **dry-run ガード**: 全 hook 箇所で `!dry_run && result.bytes_freed > 0` を確認。`write_history_entry` 側でも `freed_bytes == 0` をガード（二重防御）
- **サイレントフェイル**: `let _ = append_history(...)` — 履歴書き込みの失敗が clean 操作に影響しない
- **破損ファイル耐性**: `serde_json::from_reader` のエラーをキャッチし `eprintln!` 警告 + 空 Vec で続行
- **履歴出力形式**: ボックス描画ヘッダー + アラインメントテーブル。既存の `format_bytes()` を利用

## 変更履歴（PBI 更新）

| 日付 | 変更内容 |
|------|---------|
| 2026-05-26 | PBI-G 全機能実装完了。history.rs + stats サブコマンド + clean フロー hook。15 テスト (6 E2E + 9 unit) |
| 2026-05-25 | PBI-G ドキュメント作成。PBI-E/F 完了に伴い依存関係解決済み。実装は未着手 |

## 見積もり

**3 SP**

## Definition of Done

- [x] 全受け入れシナリオが通る（6 E2E tests）
- [x] `cargo test` 全パス（433 tests, 0 failures）
- [x] `cargo clippy -- -D warnings` クリーン
- [x] `cargo fmt --check` クリーン
- [x] コードレビュー完了（spec + quality, 修正対応済み）
- [x] `docs/HOWTO-USE.md` に stats コマンドの使い方を追記
