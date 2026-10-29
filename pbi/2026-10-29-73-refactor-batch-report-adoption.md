# PBI: バッチ skip レポートの既存関数採用統合

## ユーザーストーリー
sasurahime の保守担当者として、failures / skipped / Total freed のレポートが 1 組の既存関数に収束することがほしい、なぜなら同じレポートが main.rs 3 arm で手書きされ、文言と "Total freed" の先頭空行が漂っているから。

## 優先度
- 順位: 5 / 8
- RICE スコア: 2.13（Reach=2 / Impact=1 / Confidence=80% / Effort=0.75 人週）
- 根拠: 機械的な統合で PBI-49 で既存の report_batch_failures(ReportShape) + report_total_freed を採用するだけ。新設は不要（深掘りで確定）。PBI 74（UnattendedPolicy）が同じ main.rs arm を触るため先に着手。

## 発見の詳細（深掘り結果）
- レポート実装は 5 つ: interactive.rs:184-209 report_batch_failures（batch.rs 形 3 箇所とバイト同一）+ main.rs:789-797（単一ターゲット経路、CleanResult 形、cleaner 名なし行 = 5 つ目）
- main.rs 3 arm（1029-1040 / 1184-1195 / 1389-1400）は手書き。"Total freed" は 2 形: main.rs 3 箇所（先頭空行なし）vs interactive.rs report_total_freed（"\nTotal freed"）
- 文言差は variant 固有: auto = stderr + "Run sasurahime clean <target>" ガイダンス、all = stdout に per-target + stderr に summary の混在、yes = stdout comma-joined
- main.rs:789-797 は入力型が異なるため対象外と明記

## BDD受け入れシナリオ
Scenario: 3 arm が既存 report 関数を採用する
  Given auto / caches / all の 3 arm
  When  バッチ掃除が完了する
  Then  failures と skipped は report_batch_failures(Named)、Total freed は report_total_freed から出る
  And   手書き報告行（main.rs 3 箇所）が消える

Scenario: 文言と出力先が不変
  Given 既存 E2E テスト
  When  全テストを実行する
  Then  auto / clean_all / interactive の E2E が無修正で green

## 受け入れ基準
- [ ] main.rs 3 arm が report_batch_failures(Named) + report_total_freed を使用し手書き報告行が消える
- [ ] "Total freed" の先頭空行が全 batch arm で統一される（変更は記録済みの visible change）
- [ ] main.rs:789-797 は対象外と明記（入力型が異なる）
- [ ] 既存 E2E（auto_policy.rs / clean_all.rs / interactive.rs）が無修正で green
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
- E2E: 既存 3 本が無修正で green すること（文言・出力先の不変 pin）
- 統合: 変更後の "Total freed" 出力を 1 箇所の assert で pin

## 見積もり
0.75 人週

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（CHANGELOG の visible change 記載）
