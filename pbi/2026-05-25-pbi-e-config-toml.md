# PBI-E: config.toml 統合設定ファイル

## ユーザーストーリー

macOS 開発者として、設定ファイルでカスタムパスの追加・クリーナーの除外・フィルタ条件を一元管理したい。なぜなら、コマンドラインフラグだけでは設定を毎回指定しなければならず、cron や自動化での再利用が難しいから。

## ビジネス価値

- 一度設定すれば毎回フラグを書かなくて済む
- ホワイトリストにより「消してはいけないキャッシュ」の保護を永続化できる
- カスタムパスで対応していないツールも sasurahime で管理できる

## BDD受け入れシナリオ

```gherkin
Scenario: ホワイトリストで指定したクリーナーが除外される
  Given ~/.config/sasurahime/config.toml に exclude = ["huggingface"] が設定されている
  When sasurahime scan を実行する
  Then huggingface クリーナーはスキャン対象から除外される
  And TUI でも huggingface はリストに表示されない

Scenario: カスタムパスがスキャン対象に追加される
  Given config.toml に [[custom]] name = "my-cache" path = "~/work/.cache" が設定されている
  When sasurahime scan を実行する
  Then "my-cache" がスキャン結果テーブルに表示される
  And そのサイズが正しく計算される

Scenario: per-cleaner フィルタで古いキャッシュのみ対象にする
  Given config.toml の [cleaner.uv] に older_than_days = 30 が設定されている
  When sasurahime clean uv を実行する
  Then 30日以内に使用された uv キャッシュは削除されない
  And 30日より古いキャッシュのみ削除される

Scenario: larger_than_mb フィルタで大きいキャッシュのみ対象にする
  Given config.toml の [cleaner.brew] に larger_than_mb = 500 が設定されている
  When sasurahime clean brew を実行する
  Then 500MB 未満のキャッシュは削除されない

Scenario: config.toml が存在しない場合はデフォルト動作する
  Given ~/.config/sasurahime/config.toml が存在しない
  When sasurahime scan を実行する
  Then エラーなく通常のスキャンが実行される

Scenario: --config フラグで設定ファイルパスを上書きできる
  Given /tmp/custom-config.toml に設定が書かれている
  When sasurahime scan --config /tmp/custom-config.toml を実行する
  Then そのファイルの設定が適用される
```

## 受け入れ基準

- [ ] `~/.config/sasurahime/config.toml` を自動ロードする
- [ ] `exclude = ["cleaner-name", ...]` でクリーナーをスキャン・TUI から除外できる
- [ ] `[[custom]]` セクションで任意ディレクトリをスキャン対象に追加できる
- [ ] `[cleaner.<name>]` セクションで `older_than_days` / `larger_than_mb` を設定できる
- [ ] config.toml が存在しない場合はデフォルト動作（エラーなし）
- [ ] `--config <path>` フラグで設定ファイルパスを上書きできる
- [ ] TOML パースエラー時はわかりやすいエラーメッセージを表示して終了コード 1

## t_wada スタイル テスト戦略

```
E2Eテスト:
- tempdir に config.toml を配置し scan を実行、exclude が適用されることを検証
- カスタムパスのスキャン結果に custom cleaner が含まれることを検証
- older_than_days フィルタで新旧ファイルの削除差を検証

統合テスト:
- Config::load(path: &Path) -> Result<Config> のパースをテスト
- 各フィルタ（older_than_days / larger_than_mb）の適用ロジックをテスト
- exclude リストの適用をテスト

単体テスト:
- Config のデフォルト値テスト（ファイルなし時）
- TOML パースエラーのエラーメッセージフォーマットテスト
- older_than_days 判定: is_older_than(path, days) 純関数テスト
- larger_than_mb 判定: is_larger_than(path, mb) 純関数テスト
```

## 実装アプローチ

- **Outside-In**: `exclude` の E2E テストから開始
- **Red-Green-Refactor**:
  1. Red: config.toml の exclude が無視される E2E テストが落ちる
  2. Green: `Config` 構造体を実装し `Scanner` に渡す
  3. Refactor: `CleanerFilter` trait に分離してテスタビリティを高める
- **Config 構造体**:
  ```toml
  # ~/.config/sasurahime/config.toml
  exclude = ["huggingface", "ollama"]

  [[custom]]
  name = "my-cache"
  path = "~/work/.cache"

  [cleaner.uv]
  older_than_days = 30

  [cleaner.brew]
  larger_than_mb = 500
  ```

## 技術的考慮事項

- 依存関係: `toml` クレート + `serde` を追加（`serde_derive` feature）
- パス展開: `~` を `$HOME` に展開する処理が必要（`shellexpand` クレートまたは手動実装）
- `older_than_days` は `fs::metadata().accessed()` または `modified()` を使用
  - `atime` はファイルシステム設定に依存するため `mtime` を使う方が安全
- カスタムクリーナーは `GenericPathCleaner` として実装し `Cleaner` trait を実装

## 見積もり

**5 SP**

## Definition of Done

- [ ] 受け入れシナリオが全て通る
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` クリーン
- [ ] `cargo fmt --check` クリーン
- [ ] コードレビュー完了
- [ ] `docs/HOWTO-USE.md` に config.toml の使い方を追記
