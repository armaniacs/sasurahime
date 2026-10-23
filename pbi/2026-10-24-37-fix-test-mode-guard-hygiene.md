# PBI: テストモードガードの衛生（panic-safe TrashModeGuard 採用）

## ユーザーストーリー
並列テストスイートを保守する開発者として、グローバル trash モードの pin が panic-safe であってほしい、なぜなら library_caches.rs:672 と 692 が生の `set_trash_mode(false)` + trailing `set_trash_mode(true)` を使い（アサート失敗時に permanent モードが並列テストへ漏出）、既存の panic-safe `TrashModeGuard`（test_helpers.rs:121-137、gradle は使用済み）を使っていないから。

## 優先度
- 順位: 10 / 10
- RICEスコア: 8（Reach=5 / Impact=0.5 / Confidence=80% / Effort=0.25人日）
- 根拠: テストのみ。31（同一ファイル群）の後に実施。

## BDD受け入れシナリオ
Scenario: panic してもモードが復元される
  Given TrashModeGuard で pin されたテスト
  When  アサートが失敗して panic する
  Then  ガードの Drop でグローバルモードが復元され、並列テストへの漏出が起きない

## 受け入れ基準
- [ ] library_caches.rs:672 と 692 が TrashModeGuard を使用する
- [ ] orphan_containers.rs:1222 と 1236 の同一値 set/restore（実質 no-op）が TrashModeGuard または明示的な記述に統一される
- [ ] 既存のテストが green を維持する

## テスト戦略
- 単体: 既存テストのガード採用（挙動不変）

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
