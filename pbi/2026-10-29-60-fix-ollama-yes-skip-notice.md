# PBI: ollama `--yes` で skip されたことを run_auto の通知に出す

## ユーザーストーリー
`--yes` でスクリプトから sasurahime を実行するユーザーとして、ollama モデルが対話ゲードで skip されたことを実行結果の通知で知りたい。なぜなら、現在は stderr の 1 行（gate のメッセージ）だけで run_auto の「skip されたターゲット」一覧に出ず、ollama が掃除されなかったことに気づかないから。

## 優先度
- 順位: 60 / 全 9 候補
- RICE スコア: 16.0（Reach=2 / Impact=1 / Confidence=80% / Effort=0.1 人週）
- 根拠: ollama は `Risk::Safe`（src/cleaners/ollama.rs:81-83）なので `--yes` の auto_clean_indices に選ばれるが、clean() が `gate_interactive("ollama")`（ollama.rs:146）で非 TTY を skip する。`--yes` は定義上非 TTY なので、ollama モデルは無人実行では決して掃除されず、run_auto の「Skipped target(s) that need an explicit selection」通知（src/interactive.rs:74-86）も Safe 分類のため対象外。skip 自体は PBI 52 修正の意図的挙動（モデル削除を対話なしに進めない）だが、通知の欠落は UX の穴
- 判断: skip 挙動は維持する（安全側）。通知の追加のみ

## ビジネス価値
- `--yes` の実行結果で ollama が掃除されなかったことが 1 か所から分かる
- 「正常終了したのに何も減っていない」混乱が消える

## BDD 受け入れシナリオ
Scenario: `--yes` で ollama が skip されたことが通知に出る
  Given 非 TTY 環境（stdin がパイプ）で ollama がインストールされている
  When  `sasurahime --yes` を実行する
  Then  実行は成功し、ollama が対話ゲードで skip されたことが run_auto の通知に含まれる

Scenario: TTY では従来どおり選択プロンプトが出る
  Given TTY が接続されている
  When  ollama の clean を実行する
  Then  通知は出ず、従来どおり選択プロンプトが表示される

## 受け入れ基準
- [ ] run_auto が非 TTY で gate によって skip されたターゲットを通知に含める
- [ ] ollama の clean が非 TTY で skip される挙動は変わらない（PBI 52 修正の維持）
- [ ] 非 TTY での通知を固定するテストが 1 本追加されている
- [ ] 全テストが緑

## テスト戦略
- E2E: tests/ollama.rs の非 TTY E2E に通知の主張を追加
- 単体: run_auto の通知構築（skip されたターゲット一覧）のテスト

## 実装アプローチ
- 実装順: (1) run_auto の通知に gate-skip されたターゲットを集める仕組みを確認、(2) ollama（Risk::Safe だが gate で skip される）を通知対象に含める、(3) E2E で固定
- 判断の根拠: ollama を Risk::ManualOnly に変える選択肢は採らない。モデル削除はユーザーが選択すべきで、skip + 通知が最小の変更

## 見積もり
0.1 人週

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] CHANGELOG の [Unreleased] に追記
- [ ] コードレビュー完了
