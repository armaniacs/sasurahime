# PBI: Hugging Face Model Cache Cleaner

## ユーザーストーリー
AI/ML開発者として、Hugging Face のモデルキャッシュを sasurahime で確認・削除したい、なぜなら `~/.cache/huggingface/hub/` にダウンロードしたモデルが溜まり、数GB〜数十GBに膨れ上がることがあるから。

## ビジネス価値
ローカルLLM・埋め込みモデルの利用が急増しており、AI/ML開発者のマシンでは 1GB 以上のモデルキャッシュが蓄積されやすい。`huggingface-cli delete-cache` が使えない環境では手動削除が必要で、sasurahime が代替手段になる。

## BDD受け入れシナリオ

```gherkin
Scenario: Hugging Face キャッシュのサイズを表示する
  Given ~/.cache/huggingface/hub/ にモデルキャッシュが存在する
  When  sasurahime scan を実行する
  Then  huggingface の項目にキャッシュサイズが表示される

Scenario: huggingface-cli が存在する場合は CLI で削除する
  Given huggingface-cli が PATH に存在する
  And   ~/.cache/huggingface/hub/ にキャッシュが存在する
  When  sasurahime clean huggingface を実行する
  Then  huggingface-cli delete-cache --yes が実行される
  And   解放サイズが報告される

Scenario: huggingface-cli が存在しない場合はディレクトリを直接削除する
  Given huggingface-cli が PATH に存在しない
  And   ~/.cache/huggingface/hub/ にキャッシュが存在する
  When  sasurahime clean huggingface を実行する
  Then  hub/ 以下のキャッシュディレクトリが直接削除される
  And   解放サイズが報告される

Scenario: キャッシュが存在しない場合はスキップされる
  Given ~/.cache/huggingface/hub/ が存在しない
  When  sasurahime scan を実行する
  Then  huggingface の項目は 0 B または NotFound と表示される
```

## テスト戦略

### E2Eテスト
- ダミーの hub/ ディレクトリ（`models--org--name/snapshots/abc123/`）を作成して scan が認識する
- `--dry-run` で削除が実行されないことを確認

### 単体テスト
- `huggingface-cli` の有無による分岐（`CommandRunner` モック）
- キャッシュパス解決（`$HF_HOME` 環境変数があればそちらを優先）

## 実装アプローチ
- パス: `$HF_HOME` → `~/.cache/huggingface/hub/`（環境変数を優先）
- CLI 優先: `huggingface-cli delete-cache --yes` を試み、NotFound なら直接削除
- 直接削除: `hub/` 以下の `models--*` `datasets--*` ディレクトリを削除（`hub/` 自体は残す）
- `GenericCleaner` パターンを踏襲
