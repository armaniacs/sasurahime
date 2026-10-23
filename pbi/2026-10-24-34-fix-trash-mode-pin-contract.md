# PBI: trash_mode pin 契約の一貫性（CliCleaner / ProjectArtifacts）

## ユーザーストーリー
CleanOptions.trash_mode を pin する開発者として、全クリーナーが pin を尊重してほしい、なぜなら cleaner.rs:124-127 の契約は「Some(v) pins the mode so uses_trash と実際の削除が決して食い違わない」と述べるが、CliCleaner（generic.rs:529-533、clean_with_opts オーバーライドなしで fallback が Global 直書き）と ProjectArtifactsCleaner（project_artifacts.rs:213-217 付近、result はグローバル再読み）が pin を無視するから。

## 優先度
- 順位: 7 / 10
- RICEスコア: 24（Reach=15 / Impact=1 / Confidence=80% / Effort=0.5人日）
- 根拠: latent。到達可能な呼び出しサイトは現状 pin しないが、32 の uses_trash 統一と同じファイル群のため直列で続けて実施。

## BDD受け入れシナリオ
Scenario: pin 済みモードが CliCleaner でも効く
  Given opts.trash_mode = Some(Permanent) を持つ CleanOptions
  When  CliCleaner の fallback 削除を clean_with_opts で実行する
  Then  削除は permanent で実行され、uses_trash は false を返す

Scenario: pin 済みモードが ProjectArtifacts でも効く
  Given opts.trash_mode = Some(Trash) を持つ CleanOptions
  When  ProjectArtifactsCleaner の clean_with_opts を実行する
  Then  削除は Trash で実行され、uses_trash は true を返す

## 受け入れ基準
- [ ] CliCleaner が clean_with_opts の opts.trash_mode を fallback 削除に反映する（None は Global）
- [ ] ProjectArtifactsCleaner の clean_with_opts が opts.trash_mode を反映し、result の uses_trash がシーム outcome から返る
- [ ] pin 契約のテスト（generic.rs:1002-1045 の PathCleaner pin テストと同型）が CliCleaner / ProjectArtifacts に追加される
- [ ] 既存の削除統合テストが green を維持する

## テスト戦略
- 単体: pin 済みモードの反映テスト（Some(Permanent) / Some(Trash) / None の3形）
- 統合: 既存の削除統合テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
