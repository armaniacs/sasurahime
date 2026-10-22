# PBI: 不明サイズの解消とscan --json

## ユーザーストーリー
スクリプト利用者として、サイズ不明の `Pruneable(0)` ではなく推定値と機械可読出力を得たい、なぜなら合計の見積もりとCI判定に使えないから

## ビジネス価値
`docker/npm` 等のCLI委譲系が初めて見積もり可能になり、スクリプト・レポート連携ができる。01の契約強化の出口になる。

## 優先度
- 順位: 06 / 7
- RICEスコア: 427（Reach=400 / Impact=2 / Confidence=80% / Effort=1.5人週）
- 根拠: 01の後にしか正しく設計できないため01の後。単独スコアは中位。

## BDD受け入れシナリオ
Scenario: 不明が不明として表示される
  Given `docker` が存在するがサイズ計測できない環境がある
  When `sasurahime scan` を実行する
  Then `docker` 行は `0 B` ではなく `unknown` と表示され合計に含まれない

Scenario: JSON出力が使える
  Given 複数ターゲットがある状態がある
  When `sasurahime scan --json` を実行する
  Then 名前・Byte・状態・推定可否を含むJSONが出て `jq` で集計できる

## 受け入れ基準
- [ ] `Pruneable(0)` の乱用がなくなり `Unknown/Estimate` の区別がつく
- [ ] `scan --json` が追加され既存の表出力がデフォルトのまま保たれる
- [ ] `explore` との数値突合テストが追加される
- [ ] ロケール違いでも出力パースが壊れない（外部コマンド出力の厳密化）

## テスト戦略
- E2E: `scan --json` のスキーマテスト（`jq -e` で検証）
- 統合: CLI委譲系の `detect` が `Unknown` を返すことのテスト
- 単体: 推定値計算・JSON整形のテスト

## 見積もり
5 SP（要チームでの見積もり）

## 実装アプローチ
1. 01の `Unavailable` とは別に `Unknown/Estimated(u64)` を導入
2. `scanner.rs` に `--json` 出力を追加（表は維持）
3. `GenericCleaner::Command` 系の `detect` を `Unknown` に切替

## 技術的考慮事項
- JSONスキーマは後方互換のため追加フィールドのみで拡張する
- 外部コマンドのパースは本PBIでは変えず表示側のみ

## 実装者向け注記
- 現状確認: `grep -rn "Pruneable(0)" src/ | head`、`grep -rn "run_scan\|format_bytes" src/scanner.rs | head`
- 着手前に `cargo test` でグリーン確認。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（`targets`/`scan --json` の使い方を追記）
