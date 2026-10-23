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
- [x] clap 4 での実際の値流が実測で確定される（エンドツーエンド、破壊的確認は tempdir で）
- [x] 問題が実在する場合、グローバルとローカルの干渉が解消される（ローカル宣言の削除、または明示的な統合）
- [x] 問題が実在しない場合、その判定根拠が検証記録として残る
- [x] 全サブコマンドで --dry-run が効くことの統合テストが存在する

## テスト戦略
- E2E: tempdir HOME で `--dry-run` 付きサブコマンド実行、exit code と stdout、削除されないことの観測
- 統合: ローカル宣言との整合

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 結論: 問題は実在しない（グローバル --dry-run はローカル再宣言のあるサブコマンドにも到達する）。本番コードの変更なし。
- 実測（ビルド済みバイナリ、tempdir HOME、偽キャッシュ、--permanent コントロール付き）: `--dry-run clean caches` / `clean --dry-run caches` / `clean --dry-run node-gyp`（標準ターゲット）/ `clean --dry-run xcode` / `--dry-run auto`（+policy）のすべてで exit 0、[dry-run] would remove 表示、対象ディレクトリ残存。コントロール（--permanent clean node-gyp / xcode）は実削除を確認 → dry-run 主張は非空虚。
- 根本原因（レビュー誤検出の解明）: clap_builder-4.6.0 arg_matcher.rs fill_in_global_values はマッチ済みグローバル値を同じ id を再宣言するサブコマンドレベルを含む全レベルに挿入するため、グローバル値は CleanTarget::Caches{dry_run} 等に流れ込む。両位置指定時は内側が勝つ（同一 ValueSource）ためローカルフラグも機能する。なお `--dry-run clean auto` は有効な CLI 形式ではない（clean の external_subcommand arm が "no plugin named 'auto'" を報告、有意な形式は `--dry-run auto`）。
- テスト: tests/dry_run.rs を新設（8 E2E、--permanent コントロール付きで非空虚な主張）。
- 検証: lib 650 passed / 4 failed（既知 TCC）、統合 37 ターゲット green（新 dry_run 含む）、clippy 0、fmt クリーン。
- 備考: 受け入れ基準 2 は「問題が実在する場合」の条件付きのため該当なし（実在しないことが実測で確定）。
