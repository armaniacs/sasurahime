# PBI-009: npm / yarn / pnpm キャッシュクリーナー

## ユーザーストーリー
**Node.js プロジェクトで npm / yarn / pnpm を使う開発者**として、各パッケージマネージャーのキャッシュを個別に、または一括で削除したい、なぜなら複数のパッケージマネージャーが共存する環境では合計が数GBになりやすいから。

## ビジネス価値
- `~/.npm`・`~/.yarn/cache`・pnpm store は気づかないうちに肥大化する
- ツールごとのキャッシュコマンドを覚えなくてよくなる

## BDD 受け入れシナリオ

```gherkin
Scenario: npm キャッシュを削除する
  Given npm がインストールされている
  And ~/.npm にキャッシュが存在する
  When `sasurahime clean npm` を実行する
  Then `npm cache clean --force` が実行される
  And 回収サイズが表示される

Scenario: yarn キャッシュを削除する
  Given yarn がインストールされている
  When `sasurahime clean yarn` を実行する
  Then `yarn cache clean` が実行される

Scenario: pnpm store を prune する
  Given pnpm がインストールされている
  When `sasurahime clean pnpm` を実行する
  Then `pnpm store prune` が実行される

Scenario: 複数が共存する環境で clean caches を実行する
  Given npm と pnpm がインストールされ yarn はインストールされていない
  When `sasurahime clean caches` を実行する
  Then npm と pnpm のキャッシュが削除される
  And yarn は "not found, skipped" と表示される

Scenario: いずれもインストールされていない
  Given npm / yarn / pnpm がすべて PATH に存在しない
  When `sasurahime clean npm` を実行する
  Then "npm not found, skipping" と表示して正常終了する
```

## 受け入れ基準
- [ ] `sasurahime clean npm` / `clean yarn` / `clean pnpm` が個別に動作する
- [ ] `sasurahime clean caches` の実行対象グループに含まれる
- [ ] 各ツール未インストール時はスキップ
- [ ] `--dry-run` でコマンドを表示するのみ（実行しない）
- [ ] 回収サイズを表示する（可能な範囲で）

## t_wada スタイル テスト戦略
```
E2Eテスト:
- npm / yarn / pnpm コマンドをモックし、呼び出し引数を検証
- 未インストールツールがスキップされることを確認

統合テスト:
- NpmCleaner::is_available() がコマンド存在チェックを正しく行うこと

単体テスト:
- 各ツールの dry_run_message() が期待する文字列を返すこと
```

## 実装アプローチ
- PBI-006 と同じ `Cleaner` trait を実装
- `CommandRunner` 経由で各コマンドを実行

## 見積もり
3 ストーリーポイント

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] `clean caches` グループに統合されている
- [ ] `cargo clippy` 警告ゼロ
