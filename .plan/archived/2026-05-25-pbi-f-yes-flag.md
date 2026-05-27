# PBI-F: --yes フラグ（非インタラクティブモード）

## ユーザーストーリー

macOS 開発者として、確認プロンプトなしで全キャッシュを一括削除したい。なぜなら、`launchd` や cron で定期実行したいとき、インタラクティブな操作が必要だとスクリプト化できないから。

## ビジネス価値

- cron / launchd / CI での完全自動化が可能になる
- `config.toml` の `exclude` と組み合わせて「安全な全削除」を実現できる
- スクリプト組み込みユースケースを正式サポートする

## 実装ステータス

**全機能: ✅ 完了**

- `sasurahime --yes` — 確認プロンプトなしで全クリーナーを一括実行 ✅
- `sasurahime clean <target> --yes` — 特定ターゲットを確認なしで実行 ✅
- 非 TTY 環境でも動作（stdin が `/dev/null` でも OK） ✅
- PBI-E `exclude` との統合 ✅
- PBI-C ゴミ箱警告の表示 ✅
- PBI-B エラーハンドリングの継承 ✅
- Progress bar 表示 ✅
- `--permanent` + 確認プロンプトとの連携 ✅
- `--dry-run` との共存 ✅

## BDD 受け入れシナリオ

```gherkin
Scenario: --yes フラグで全クリーナーを確認なしで実行する
  Given キャッシュが複数のクリーナーに存在する
  When sasurahime --yes を実行する
  Then 全クリーナーの clean() が確認プロンプトなしで実行される
  And 削除結果のサマリーが stdout に出力される
  And 終了コードは 0 である

Scenario: --yes は stdin を読まない
  Given sasurahime --yes が実行される
  When stdin が /dev/null にリダイレクトされている
  Then エラーなく完了する
  And TTY がなくても動作する

Scenario: config.toml の exclude と --yes を組み合わせる
  Given config.toml に exclude = ["huggingface"] が設定されている
  When sasurahime --yes を実行する
  Then huggingface は削除されない
  And 残りの全クリーナーは削除される

Scenario: --yes でもゴミ箱警告は表示される
  Given ゴミ箱移動を行うクリーナーが存在する
  When sasurahime --yes を実行する
  Then ゴミ箱移動後の警告メッセージは stdout に出力される

Scenario: 特定クリーナーだけ --yes で実行する
  Given sasurahime clean brew --yes を実行する
  Then brew のみを確認なしで削除する
```

## 受け入れ基準

- [x] `sasurahime --yes` で全クリーナーを確認なしで実行する
- [x] `sasurahime clean <target> --yes` で特定クリーナーを確認なしで実行する
- [x] stdin が TTY でなくても（`/dev/null` リダイレクトでも）動作する
- [x] `config.toml` の `exclude` を尊重する
- [x] ゴミ箱警告（PBI-C）は `--yes` 時も表示する
- [x] エラーハンドリング（PBI-B）は `--yes` 時も有効

## t_wada スタイル テスト戦略

```
E2Eテスト（実装済み: tests/interactive.rs + tests/trash.rs に 9 テスト）:
- yes_flag_exits_zero_and_skips_tui: --yes が TUI をスキップする
- yes_flag_nothing_pruneable_exits_zero: 削除対象がなくてもエラーにならない
- startup_version_display_yes: --yes 起動時にバージョン表示
- yes_flag_cleans_xcode_without_interactive_prompt: Xcode 実行中の警告表示
- yes_flag_shows_progress_spinner: プログレスバー表示
- yes_flag_shows_detect_progress: スキャンプログレス表示
- no_args_without_tty_exits_with_hint: TTY なしで --yes のヒント表示
- yes_with_empty_dir_exits_cleanly: 空ディレクトリで正常終了
- yes_permanent_requires_confirmation: --permanent と --yes の組み合わせ
```

## 実装アプローチ

- **実装方法**: `Cli` struct に `#[arg(long)] yes: bool` を追加
- **分岐**: `main()` で `if cli.yes { interactive::run_auto(&cleaners) } else { interactive::run_interactive(&cleaners) }`
- **run_auto の動作**:
  1. 並列スキャン（`with_parallel_scan`）
  2. Pruneable なクリーナーに対して順次 `clean()` を実行
  3. 各クリーン結果の freed bytes を合計し、総解放量を表示
- **確認プロンプトの抑制**: 非 TTY 環境では `GenericCleaner` の `confirm_message` が自動スキップ。TUI でも `set_skip_confirm(true)` で二重確認を防止
- **Xcode の特別処理**: `--yes` 時は Xcode 実行中の警告を表示して続行（`cli.yes && xcode_cleaner.is_xcode_running()`）
- **依存関係**: `clap` の `ArgAction::SetTrue`（追加依存なし）

## 技術的考慮事項

- `--yes` と `--dry-run` の同時指定: `--dry-run` が優先（削除しない）
- `--yes --permanent`: 確認プロンプトが表示される（完全削除の安全性確認）
- `exclude` との関係: PBI-E の exclude は `all_cleaners()` でフィルタ済み。`run_auto()` は既にフィルタされたリストを受け取る
- TTY 検出: `std::io::IsTerminal` — `interactive.rs` の `run_interactive()` で TTY チェック。`run_auto()` はチェックしない

## 変更履歴（PBI 更新）

| 日付 | 変更内容 |
|------|---------|
| 2026-05-25 | PBI-F 全機能実装完了。PBI-E の exclude 統合も含め全シナリオ対応 |

## 見積もり

**2 SP**（PBI-E の config.toml 実装に依存）

## Definition of Done

- [x] 全受け入れシナリオが通る
- [x] `cargo test` 全パス（418 tests, 0 failures）
- [x] `cargo clippy -- -D warnings` クリーン
- [x] `cargo fmt --check` クリーン
- [x] コードレビュー完了（PBI-D/E の統合レビューに含む）
- [x] PBI-E `exclude` との統合確認
- [x] PBI-C ゴミ箱警告との統合確認
- [x] PBI-B エラーハンドリングとの統合確認
