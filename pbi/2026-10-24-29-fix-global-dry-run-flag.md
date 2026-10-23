# PBI: グローバル --dry-run がサブコマンドで無視される問題の検証・修正

## ユーザーストーリー
`sasurahime --dry-run clean auto` を実行するスクリプト作成者として、どのサブコマンドでも `--dry-run` が確実に効いてほしい、なぜならレビューで「グローバル `--dry-run`（main.rs:352 `global = true`）がローカル再宣言と同じ id を持つサブコマンド（Auto/Caches/All/Logs）で沈黙裏に飲まれる可能性」が指摘され、実なら flag 無視の実削除（データ損失クラス）だから。

## 優先度
- 順位: 2 / 10
- RICEスコア: 300（Reach=50 / Impact=3 / Confidence=50% / Effort=0.25人日）
- 根拠: 実ならデータ損失クラスだが未検証（clap 4 の受付とローカル help 表示は実測、値流は clap セマンティクスからの推論）。verify-first で安価に確定できる。

## BDD受け入れシナリオ
Scenario: グローバル --dry-run がサブコマンドでも効く
  Given tempdir HOME にクリーン対象のキャッシュを作る
  When  `sasurahime --dry-run clean auto` を実行する
  Then  何も削除されず、dry-run の表示が出る

Scenario: サブコマンドローカルの dry_run とグローバルの整合
  Given tempdir HOME
  When  `sasurahime clean --dry-run xcode`（ローカル宣言）と `sasurahime --dry-run clean auto`（グローバル）をそれぞれ実行する
  Then  どちらも削除が走らない

## 受け入れ基準
- [ ] clap 4 での実際の値流が実測で確定される（エンドツーエンド、破壊的確認は tempdir で）
- [ ] 問題が実在する場合、グローバルとローカルの干渉が解消される（ローカル宣言の削除、または明示的な統合）
- [ ] 問題が実在しない場合、その判定根拠が検証記録として残る
- [ ] 全サブコマンドで --dry-run が効くことの統合テストが存在する

## テスト戦略
- E2E: tempdir HOME で `--dry-run` 付きサブコマンド実行、exit code と stdout、削除されないことの観測
- 統合: ローカル宣言との整合

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
