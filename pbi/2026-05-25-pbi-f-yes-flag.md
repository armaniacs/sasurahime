# PBI-F: --yes フラグ（非インタラクティブモード）

## ユーザーストーリー

macOS 開発者として、確認プロンプトなしで全キャッシュを一括削除したい。なぜなら、`launchd` や cron で定期実行したいとき、インタラクティブな操作が必要だとスクリプト化できないから。

## ビジネス価値

- cron / launchd / CI での完全自動化が可能になる
- `config.toml` の `exclude` と組み合わせて「安全な全削除」を実現できる
- スクリプト組み込みユースケースを正式サポートする

## BDD受け入れシナリオ

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

- [ ] `sasurahime --yes` で全クリーナーを確認なしで実行する
- [ ] `sasurahime clean <target> --yes` で特定クリーナーを確認なしで実行する
- [ ] stdin が TTY でなくても（`/dev/null` リダイレクトでも）動作する
- [ ] `config.toml` の `exclude` を尊重する
- [ ] ゴミ箱警告（PBI-C）は `--yes` 時も表示する
- [ ] エラーハンドリング（PBI-B）は `--yes` 時も有効

## t_wada スタイル テスト戦略

```
E2Eテスト:
- sasurahime --yes を stdin=/dev/null で実行し終了コード 0 を確認
- 削除後のディレクトリが存在しないことを確認
- config.toml の exclude と組み合わせたテスト

統合テスト:
- Context::is_yes() / Args::yes フィールドが正しく伝播することをテスト
- Cleaner::clean() が yes=true 時に確認をスキップすることをテスト

単体テスト:
- コマンドライン引数パース: `sasurahime --yes` で yes=true になることをテスト
- `sasurahime clean brew --yes` で target="brew", yes=true になることをテスト
```

## 実装アプローチ

- **Outside-In**: `/dev/null` stdin で `--yes` の E2E テストから開始
- **Red-Green-Refactor**:
  1. Red: stdin がない状態で通常実行するとハング or エラー
  2. Green: `Args` に `yes: bool` を追加し `Context` に伝播
  3. Refactor: `Prompter` trait を導入し `YesPrompter` / `InteractivePrompter` を切り替え
- **既存コードへの影響**: `dialoguer` の確認プロンプトを `Prompter` 経由にする

## 技術的考慮事項

- 依存関係: 追加なし（`clap` の `action = ArgAction::SetTrue`）
- `--yes` と `--dry-run` の同時指定: `--dry-run` が優先（削除しない）
- テスタビリティ: `Prompter` trait でモック可能にする
  ```rust
  trait Prompter: Send + Sync {
      fn confirm(&self, msg: &str) -> bool;
  }
  ```

## 見積もり

**2 SP**（PBI-E の config.toml 実装後）

## Definition of Done

- [ ] 受け入れシナリオが全て通る
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` クリーン
- [ ] `cargo fmt --check` クリーン
- [ ] コードレビュー完了
- [ ] `docs/HOWTO-USE.md` に `--yes` の使い方と注意事項を追記
