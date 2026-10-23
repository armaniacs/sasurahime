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
- [ ] 4クリーナー（library_caches, orphan_containers, app_support, mise）の削除ループで、safe_delete の Err を per-entry で捕えて警告 + 継続し、実行完了まで進む
- [ ] 失敗エントリは per-entry の stderr 文脈（クリーナー名 + 対象）付きで報告される
- [ ] シーム自体の分類（is_skippable_error → skip_reason、fatal → Err）は変更しない
- [ ] 既存のスキップ記録テストが green を維持する

## テスト戦略
- 統合: 1エントリが消えた状態での --permanent 実行、残りが削除されることの観測
- 単体: ループレベルの Err 捕捉のテスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
