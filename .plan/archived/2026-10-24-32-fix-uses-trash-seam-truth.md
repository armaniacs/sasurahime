# PBI: uses_trash の seam truth 統一と PBI 記録の訂正

## ユーザーストーリー
Trash 表示の正確さを維持する開発者として、全クリーナーが uses_trash をシーム outcome から返してほしい、なぜなら gradle.rs（118,154,252,288）/ generic.rs（524）/ project_artifacts.rs（236）の6サイトが依然 `uses_trash: crate::trash::is_trash_mode()` を読み、チェック済みの PBI 20 受け入れ基準とアーカイブ済み検証記録の「既知の全サイトが OR-in に置換」の主張に矛盾するから。

## 優先度
- 順位: 5 / 10
- RICEスコア: 56（Reach=35 / Impact=1 / Confidence=80% / Effort=0.5人日）
- 根拠: 記録の正確さと将来の pin 拡張への備え。今日ランタイムで真実（全サイト Global + 実行ごとのグローバル pin）だが、記録とコードの不一致は必ず是正する。

## BDD受け入れシナリオ
Scenario: シーム outcome からの uses_trash
  Given 6サイトの削除経路
  When  クリーンを実行する
  Then  uses_trash は is_trash_mode() の直接読みでなく、削除 outcome の uses_trash の OR-in から返る

Scenario: OR-in 配置の統一
  Given xcode.rs:238 / uv.rs:111 / cargo.rs:109,140 / browser.rs:151 / custom.rs:96 / log.rs:194 が skip チェック前に OR-in している
  When  全削除がスキップされる
  Then  uses_trash は立たない（何も削除されていないため）

## 受け入れ基準
- [x] 6サイト（gradle.rs x4, generic.rs:524, project_artifacts.rs:236）が outcome.uses_trash の OR-in に置換される
- [x] skip チェック前に OR-in している6サイト（xcode, uv, cargo x2, browser, custom, log）が skip チェック後に統一される
- [x] gradle.rs:321-340 の gradle_clean_uses_trash_true がシーム truth を主張する形に更新される（グローバル pin でハードコードを通すのではなく）
- [x] アーカイブ済み .plan/archived/2026-10-23-20-fix-safe-deleter-seam.md の検証記録の該当主張が実際に合わせて訂正される（6サイトの残存を明記）
- [x] 既存の削除統合テストが green を維持する

## テスト戦略
- 単体: 全削除スキップ時に uses_trash が立たないことのテスト
- 統合: 既存の uses_trash 関連テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 2513ba7。
- 6ハードコードサイト除去: gradle.rs x4 / generic.rs:524 / project_artifacts.rs が outcome.uses_trash の OR-in に置換。grep is_trash_mode() src/cleaners/ は 0 件。
- OR-in 配置統一: xcode / uv / cargo x2 / browser / custom / log の skip チェック前 OR-in 6件が skip チェック後に移動（スキップされた Trash 削除は uses_trash を立てない）。
- generic.rs:524: 対象外許可の注記があったが false に置換 — サイトは削除しない（safe_delete 不通過）ため zero-deletion の真実値であり、free clean_cli_or_fallback の unsafe-path return と PBI 受け入れ基準の記載に整合。
- gradle テスト修正: gradle_clean_uses_trash_true / jetbrains_clean_uses_trash_true がシーム truth を主張（uses_trash == skipped.is_empty() の条件形、TCC-deterministic）。red 4件 → green。
- 新テスト: zero-deletion / all-skipped で !uses_trash（all-skipped は読めるが書けない親 chmod 0o555 で skip を決定論化、権限は TempDir クリーンアップ前に復元）。
- アーカイブ記録訂正: .plan/archived/2026-10-23-20-fix-safe-deleter-seam.md 検証記録に【訂正 2026-10-24 (PBI 32)】の1行を追加（履歴書き換えなし）— アーカイブ時点で6サイトが依然グローバルを読んでいたことを明記。
- 検証: lib 666 passed / 4 failed（既知 TCC のみ、5番目フレークも green）、統合 37 ターゲット green、clippy 0、fmt クリーン。
