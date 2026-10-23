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
- [x] CliCleaner が clean_with_opts の opts.trash_mode を fallback 削除に反映する（None は Global）
- [x] ProjectArtifactsCleaner の clean_with_opts が opts.trash_mode を反映し、result の uses_trash がシーム outcome から返る
- [x] pin 契約のテスト（generic.rs:1002-1045 の PathCleaner pin テストと同型）が CliCleaner / ProjectArtifacts に追加される
- [x] 既存の削除統合テストが green を維持する

## テスト戦略
- 単体: pin 済みモードの反映テスト（Some(Permanent) / Some(Trash) / None の3形）
- 統合: 既存の削除統合テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 6887842。
- CliCleaner: clean() は Global の1行 compat 委譲に（PathCleaner::clean と同じパターン）、新 clean_with_opts が opts.trash_mode.unwrap_or(Global) を解決して inherent clean_with_mode(dry_run, _reporter, mode) にスレッド。fallback safe_delete サイトは mode を渡す。ツール実行経路（当該コードベースの削除 seam なし）は uses_trash: false を維持。impl 分割は既存の複数 impl ブロックスタイルに一致。
- ProjectArtifactsCleaner: clean() → clean_with_mode(Global)、clean_with_opts が filter-merged effective を構築して effective.clean_with_mode(dry_run, opts.trash_mode.unwrap_or(Global)) を呼ぶ。PBI 32 の OR-in（skipped outcomes は非 OR）は確認済みで、pinned モード下で result の uses_trash は自動的に seam outcome を反映。
- pin テスト: 3形/cleaner（Some(Permanent) with trash global / Some(Trash) with permanent global / None with permanent global）、TEST_LOCK + panic-safe TrashModeGuard。CliCleaner の Some(Trash) は uses_trash==true 直接主張（fallback skip 分岐が seam のモード主張を返すため gradle 条件形が成立しない、逸脱を記録）、ProjectArtifacts は gradel 形の seam-truth 条件。
- TDD: red 4件（pin 無視を証明、Some(Permanent) でディレクトリ残存、Some(Trash) で permanent rm + uses_trash 不一致）→ green。
- 検証: lib 672 passed / 4 failed（既知 TCC のみ、+6 新規）、統合 37 ターゲット green、clippy 0、fmt クリーン。
