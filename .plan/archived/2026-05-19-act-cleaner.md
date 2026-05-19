# PBI: act Cache Cleaner

## ユーザーストーリー
GitHub Actions をローカルで実行する開発者として、act のアクションキャッシュを sasurahime で確認・削除したい、なぜなら `~/.cache/act/` にダウンロードされた GitHub Actions のスナップショットが蓄積し、数百MB になることがあるから。

## ビジネス価値
CI/CD ワークフローをローカル実行する用途で act は広く使われており、`actions/setup-node` や `actions/cache` など頻繁に使うアクションが繰り返し蓄積される。手動で削除する機会が少ないため sasurahime が有効。

## BDD受け入れシナリオ

```gherkin
Scenario: act キャッシュのサイズを表示する
  Given ~/.cache/act/ にキャッシュが存在する
  When  sasurahime scan を実行する
  Then  act の項目にキャッシュサイズが表示される

Scenario: act キャッシュを削除する
  Given ~/.cache/act/ にキャッシュが存在する
  When  sasurahime clean act を実行する
  Then  ~/.cache/act/ 以下のキャッシュエントリが削除される
  And   解放サイズが報告される

Scenario: dry-run では削除されない
  Given ~/.cache/act/ にキャッシュが存在する
  When  sasurahime clean act --dry-run を実行する
  Then  ファイルは削除されない
  And   削除予定のサイズが表示される

Scenario: キャッシュが存在しない場合はスキップされる
  Given ~/.cache/act/ が存在しない
  When  sasurahime scan を実行する
  Then  act の項目は 0 B または NotFound と表示される
```

## テスト戦略

### E2Eテスト
- ダミーの `actions-setup-node@v4/` ディレクトリを作成して scan が認識する
- `--dry-run` で削除が実行されないことを確認

### 単体テスト
- `$ACT_CACHE_DIR` 環境変数によるパス上書き
- キャッシュエントリ列挙（`<action-name>@<ref>/` 形式）

## 実装アプローチ
- パス: `$ACT_CACHE_DIR` → `~/.cache/act/`
- act 自体に cache purge コマンドはないため、ディレクトリ直接削除
- `~/.cache/act/` 以下を全削除（act は次回実行時に再ダウンロードする）
- `GenericCleaner` パターンを踏襲（外部 CLI なし、シンプルなディレクトリ削除）
