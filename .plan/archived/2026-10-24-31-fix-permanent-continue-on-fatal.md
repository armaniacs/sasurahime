# PBI: --permanent モードで continue-on-fatal を復元

## ユーザーストーリー
`sasurahime --permanent clean caches` を実行するユーザーとして、scan と delete の間で消えた1つのパス（TOCTOU）が残りのエントリを中断しないでほしい、なぜなら library_caches.rs / orphan_containers.rs / app_support.rs / mise.rs の4クリーナーは旧実装では全エラーを SkippedEntry に記録して継続していたが、新実装の `safe_delete(...)?` は NotFound（permanent で by-design fatal）が複数エントリ実行の全体を中断するから。

## 優先度
- 順位: 4 / 10
- RICEスコア: 128（Reach=40 / Impact=2 / Confidence=80% / Effort=0.5人日）
- 根拠: 多エントリ実行の堅牢性。シームの分類（fatal→Err）は変えず、ループレベルで per-entry の継続を実現する。

## BDD受け入れシナリオ
Scenario: TOCTOU で消えたパスが残りを中断しない
  Given 複数エントリのうち1つが scan 後に消えている
  When  --permanent でクリーンする
  Then  そのエントリは警告付きでスキップされ、残りのエントリは削除される

Scenario: 致命的でないエラーの扱い
  Given 削除が1エントリで失敗する
  When  --permanent でクリーンする
  Then  失敗は per-entry の stderr 文脈付きで報告され、実行は完了する

## 受け入れ基準
- [x] 4クリーナー（library_caches, orphan_containers, app_support, mise）の削除ループで、safe_delete の Err を per-entry で捕えて警告 + 継続し、実行完了まで進む
- [x] 失敗エントリは per-entry の stderr 文脈（クリーナー名 + 対象）付きで報告される
- [x] シーム自体の分類（is_skippable_error → skip_reason、fatal → Err）は変更しない
- [x] 既存のスキップ記録テストが green を維持する

## テスト戦略
- 統合: 1エントリが消えた状態での --permanent 実行、残りが削除されることの観測
- 単体: ループレベルの Err 捕捉のテスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 7a3e28b。
- 4クリーナーの削除ループ（library_caches delete_entries / orphan_containers delete_selected / app_support apply_selection / mise removal loop）で safe_delete の Err を per-entry で捕え、per-entry 文脈（各ファイルの既存プレフィックス形状）付き eprintln + 既存チャネルへの記録 + continue に変更。シーム自体の分類（is_skippable_error → skip_reason、fatal → Err）は未変更。
- library_logs パリティ: skip 理由が結果から落ちていた（eprintln のみ）問題を修正 — SkippedEntry チャネルに記録し、fatal Err arm も記録 + 継続。
- TDD: red（delete_entries_continues_past_fatal_permanent_delete が unwrap on Err でループ中断を証明）→ green（生存エントリは削除され、skipped.len()==1 が消滅パスと理由を運ぶ）。
- 検証: lib 662 passed / 4 failed（既知 TCC のみ、+1 新規）、統合 37 ターゲット green、clippy 0、fmt クリーン。
- 備考: 5番目の既知フレーク cleaners::xcode::tests::clean_selected_subcategory_only_deletes_that_one はこのサンドボックスで pass/fail が反転する既知の pre-existing フレーク（git stash で pristine tree でも失敗を証明済み、グローバル pin の下で実 trash バックエンドを経由、TEST_LOCK なし）。
