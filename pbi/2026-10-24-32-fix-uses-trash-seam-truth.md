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
- [ ] 6サイト（gradle.rs x4, generic.rs:524, project_artifacts.rs:236）が outcome.uses_trash の OR-in に置換される
- [ ] skip チェック前に OR-in している6サイト（xcode, uv, cargo x2, browser, custom, log）が skip チェック後に統一される
- [ ] gradle.rs:321-340 の gradle_clean_uses_trash_true がシーム truth を主張する形に更新される（グローバル pin でハードコードを通すのではなく）
- [ ] アーカイブ済み .plan/archived/2026-10-23-20-fix-safe-deleter-seam.md の検証記録の該当主張が実際に合わせて訂正される（6サイトの残存を明記）
- [ ] 既存の削除統合テストが green を維持する

## テスト戦略
- 単体: 全削除スキップ時に uses_trash が立たないことのテスト
- 統合: 既存の uses_trash 関連テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
