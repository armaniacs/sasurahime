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
- [x] library_caches.rs:672 と 692 が TrashModeGuard を使用する
- [x] orphan_containers.rs:1222 と 1236 の同一値 set/restore（実質 no-op）が TrashModeGuard または明示的な記述に統一される
- [x] 既存のテストが green を維持する

## テスト戦略
- 単体: 既存テストのガード採用（挙動不変）

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 2ba7aad。
- 許可ファイル内の全 raw set/restore サイトを panic-safe TrashModeGuard::set(...) に変換（各テストは pin の意図を述べるコメントを保持、TEST_LOCK は既存のまま、trailing restore は削除）:
  - library_caches.rs: delete_entries_removes_and_sums_bytes（set(false)+trailing set(true) → set(false)）、delete_entries_records_failures_as_skipped（同値 set(true)/set(true) の no-op → set(true)）。
  - orphan_containers.rs: delete_selected_records_failures_as_skipped（同値 no-op → set(true)）。
  - app_support.rs: 8 テスト（apply_* 系、各 set(false)+trailing set(true) → set(false)）。
  - mise.rs: テストモジュールに set_trash_mode サイトゼロ（grep 証明）→ 採用対象なし。
- 許可5ファイルに raw set_trash_mode はゼロを確認。
- 挙動中立リファクタ: 全テストの pin 値は変化なし、全テスト green。
- 検証: lib 662 passed / 4 failed（既知 TCC のみ）、統合 37 ターゲット green、clippy 0、fmt クリーン。
