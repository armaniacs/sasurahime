# PBI: Trash & Downloads Reporter

## ユーザーストーリー
macOSユーザーとして、ゴミ箱とダウンロードフォルダの使用量を sasurahime scan で確認したい、なぜならこれらのフォルダも定期的に掃除したいが、対応するアプリがなく放置しがちだから。

## ビジネス価値
実装が極めて簡単（`dir_size` を呼ぶだけ）。
ただし「削除」は危険なため、scan での報告のみとし、clean は確認を必須とする。

## BDD受け入れシナリオ

```gherkin
Scenario: ゴミ箱のサイズを表示する
  Given ~/.Trash/ にファイルが存在する
  When  sasurahime scan を実行する
  Then  trash の項目にゴミ箱のサイズが表示される

Scenario: ダウンロードフォルダのサイズを表示する
  Given ~/Downloads/ にファイルが存在する
  When  sasurahime scan を実行する
  Then  downloads の項目にダウンロードフォルダのサイズが表示される

Scenario: ダウンロードフォルダの削除には確認が必要
  Given ~/Downloads/ に古いファイルが存在する
  When  sasurahime clean downloads を実行する
  Then  削除前に確認が表示される
```

## テスト戦略

### E2Eテスト
- ダミーの Trash / Downloads ディレクトリを作成して scan が認識することを確認
- Downloads の clean は `--dry-run` のみテスト（実際の削除は確認必須のため）

### 単体テスト
- 古いファイルの日付フィルタ（logs の `is_older_than` を流用）

## 実装アプローチ
- Trash: `dir_size("~/.Trash")` でサイズ報告のみ。clean は実装しない（危険）。
- Downloads: `dir_size("~/Downloads")` + 30日より古いファイルのみ削除候補（logs の `keep_days` と同様）。デフォルトは scan のみ、clean は `--force` フラグがなければ確認を求める。
- 確認プロンプトが必要な初めてのケース→Xcode の `is_xcode_running` と同様のパターンで実装
