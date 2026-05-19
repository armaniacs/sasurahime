# PBI: pre-commit Cache Cleaner

## ユーザーストーリー
Python/多言語開発者として、pre-commit のフック環境キャッシュを sasurahime で確認・削除したい、なぜなら `~/.cache/pre-commit/` にリポジトリごとのフック環境が蓄積され、数百MB 〜 1GB になることがあるから。

## ビジネス価値
pre-commit は Python プロジェクト以外でも Rust・Go・JS プロジェクトで広く使われており、対象ユーザーが広い。フック環境は古いコミット参照のものが残り続けるため、定期的な清掃が有効。`pre-commit clean` という公式コマンドがあり実装が簡単。

## BDD受け入れシナリオ

```gherkin
Scenario: pre-commit キャッシュのサイズを表示する
  Given ~/.cache/pre-commit/ にキャッシュが存在する
  When  sasurahime scan を実行する
  Then  pre-commit の項目にキャッシュサイズが表示される

Scenario: pre-commit が存在する場合は CLI で削除する
  Given pre-commit が PATH に存在する
  And   ~/.cache/pre-commit/ にキャッシュが存在する
  When  sasurahime clean pre-commit を実行する
  Then  pre-commit clean が実行される
  And   解放サイズが報告される

Scenario: pre-commit が存在しない場合はディレクトリを直接削除する
  Given pre-commit が PATH に存在しない
  And   ~/.cache/pre-commit/ にキャッシュが存在する
  When  sasurahime clean pre-commit を実行する
  Then  ~/.cache/pre-commit/ が直接削除される
  And   解放サイズが報告される

Scenario: キャッシュが存在しない場合はスキップされる
  Given ~/.cache/pre-commit/ が存在しない
  When  sasurahime scan を実行する
  Then  pre-commit の項目は 0 B または NotFound と表示される
```

## テスト戦略

### E2Eテスト
- ダミーの `repo*/` ディレクトリを作成して scan が認識する
- `--dry-run` で削除が実行されないことを確認

### 単体テスト
- `pre-commit` コマンドの有無による分岐（`CommandRunner` モック）
- `$PRE_COMMIT_HOME` 環境変数によるパス上書き

## 実装アプローチ
- パス: `$PRE_COMMIT_HOME` → `$XDG_CACHE_HOME/pre-commit` → `~/.cache/pre-commit/`（環境変数を優先）
- CLI 優先: `pre-commit clean` を試み、NotFound なら直接削除
- ターゲット名: `pre-commit`（ハイフン含む。CLI オプションと合わせる）
- `GenericCleaner` パターンを踏襲
