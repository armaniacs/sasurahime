# PBI-011: 設定ファイルサポート

## ユーザーストーリー
**sasurahime を日常的に使う開発者**として、ログ保持日数や追加のログターゲットを設定ファイルで管理したい、なぜなら毎回 `--keep-days` を指定するのが手間で、独自ツールのログも整理したいから。

> **設定ファイル形式**: `~/.config/sasurahime/config.toml` (TOML)

## ビジネス価値
- ハードコードされた設定値をユーザーが上書きできる
- PBI-007 のログクリーナーを任意のツールに拡張できる
- 一度設定すれば引数なし実行でも意図通りに動く

## BDD 受け入れシナリオ

```gherkin
Scenario: 設定ファイルのログ保持日数が使われる
  Given ~/.config/sasurahime/config.toml に keep_days = 30 が設定されている
  When `sasurahime clean logs` を実行する
  Then 30 日より古いログが削除される
  And フラグなしでも設定値が適用される

Scenario: CLI フラグが設定ファイルを上書きする
  Given ~/.config/sasurahime/config.toml に keep_days = 30 が設定されている
  When `sasurahime clean logs --keep-days 7` を実行する
  Then 7 日より古いログが削除される（フラグが優先）

Scenario: ユーザーが追加したログターゲットが処理される
  Given ~/.config/sasurahime/config.toml に my-tool のログパスが定義されている
  When `sasurahime clean logs` を実行する
  Then my-tool のログも整理対象になる

Scenario: 設定ファイルが存在しない場合はデフォルト値で動作する
  Given ~/.config/sasurahime/config.toml が存在しない
  When `sasurahime clean logs` を実行する
  Then デフォルト値（keep_days = 7 等）で動作する
  And エラーにならない

Scenario: 設定ファイルに構文エラーがある場合
  Given ~/.config/sasurahime/config.toml が不正な TOML である
  When sasurahime を実行する
  Then わかりやすいエラーメッセージを表示して終了する
```

## 設定ファイル仕様

```toml
# ~/.config/sasurahime/config.toml

[logs]
keep_days = 7  # デフォルト: 7

[[logs.targets]]
name = "kilo"
path = "~/.local/share/kilo/log"
exclude = ["dev.log"]

# ユーザー追加ターゲットの例
[[logs.targets]]
name = "my-tool"
path = "~/.local/share/my-tool/logs"
```

- ファイルが存在しない場合は全項目がデフォルト値
- 部分的な記述も有効（記述のない項目はデフォルト値）
- `~` はホームディレクトリに展開する

## 受け入れ基準
- [ ] `~/.config/sasurahime/config.toml` を読み込む
- [ ] ファイル不在時はデフォルト値で動作する（エラーにしない）
- [ ] CLI フラグは設定ファイルの値を上書きする
- [ ] `[[logs.targets]]` でユーザーが追加したターゲットが `clean logs` に反映される
- [ ] 構文エラー時にわかりやすいエラーメッセージを表示する
- [ ] `~` をホームディレクトリに展開する

## t_wada スタイル テスト戦略
```
E2Eテスト:
- tmpdir に config.toml (keep_days = 30) を配置し、clean logs が 30 日基準で動作することを確認
- config.toml なしで clean logs がデフォルト値で動作することを確認

統合テスト:
- Config::load(path) が正しく設定を読み込むこと
- Config::load(非存在パス) がデフォルト値の Config を返すこと
- CLI フラグが Config の値を上書きすること

単体テスト:
- expand_tilde("~/.local/share/kilo/log") が正しいパスを返すこと
- Config::default() が期待するデフォルト値を持つこと
```

## 実装アプローチ
- **Outside-In**: config ありと config なしの E2E テストから開始
- TOML パース: `toml` クレート
- 設定と CLI フラグのマージ: clap の値を優先して Config を上書き

## 見積もり
3 ストーリーポイント

## 技術的考慮事項
- 依存: `toml`, `serde`, `dirs`（ホームディレクトリ取得）
- 将来拡張: 他のクリーナーの設定項目（除外パス等）も追加できる設計にする

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] config なしでも全クリーナーが正常動作することを確認
- [ ] `cargo clippy` 警告ゼロ
