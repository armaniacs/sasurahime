# PBI: 削除回帰ゲートの再構築（TCC 許容テストが回帰を隠す問題）

## ユーザーストーリー
削除経路の回帰を監視する開発者として、uv の実削除がテストで直接カバーされてほしい、なぜなら tests/trash.rs の TCC 許容版書き換え（09bba4f..0f7a9e9 で実施）が「old cache must be removed」の無条件 assert を環境判別子に変え、`simple-v16` が残存した場合に要求されるのは exit 1 + `Freed: 0 B`（スキップ可能な削除失敗すべてが同じ結果）で、実回帰と TCC スキップが区別できなくなり、この E2E が uv 実削除の唯一のゲートだったから。

## 優先度
- 順位: 3 / 10
- RICEスコア: 192（Reach=60 / Impact=2 / Confidence=80% / Effort=0.5人日）
- 根拠: ゲート欠落の是正。依存元はないが、safe_delete（28）に触れないため並列可。

## BDD受け入れシナリオ
Scenario: TCC が動く環境での実削除ゲート
  Given tempdir HOME に uv の古い simple-vN ディレクトリを作る
  When  uv cleaner の clean を trash バックエンドが動く環境で実行する
  Then  ディレクトリが消え、その削除が観測される（バックエンドが動かない環境ではスキップ分岐であることが明示される）

Scenario: テスト名と本体の一致
  Given 現行の trash_mode_with_permanent_cleaner_does_not_claim_trash
  When  テストを読む
  Then  名前と本体が一致し、コードが決して出力しないリテラルへの空弁論 assert が存在しない

## 受け入れ基準
- [x] uv の clean() 実削除を直接カバーする TCC セーフなテストが存在する（RecordingRunner か mock バックエンドで trash 成功経路を観測、またはスキップ分岐であることを明示する分岐設計）
- [x] tests/trash.rs:94-143 のテスト名と本体が一致する（in-Trash 出力を主張する分岐を持つなら名前もそれを述べる）
- [x] コードが決して出力しないリテラル（"moved to Trash"）への空弁論 assert が削除される（tests/trash.rs:139-142、同パターンの 177/210/283 は pre-existing として報告に記録、diff 内のものは修正）
- [x] 既存の他テストは追加編集なしに green を維持する

## テスト戦略
- 統合: uv clean の実削除観測テスト（trash バックエンドの成否を環境判別子として明示）
- 単体: テスト名と本体の一致は実装者確認

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 07182ef（tests/trash.rs のみ）。
- TCC フリーの削除ゲート: 新テスト permanent_mode_uv_clean_removes_old_indexes_unconditionally がビルド済みバイナリで --permanent clean uv を実行（子プロセスのグローバル trash モードが off に pin され、削除は fs::remove_dir_all、Finder/TCC 不関与）。simple-v16 が無条件に消えること（スキップ分岐で回帰を吸収しない）、simple-v21 が残存、Freed: 表示、Logical freed: 不在（本番コードが trash モードで出力するリテラルなので実主張）を assert。
- Red 証拠（break + revert）: safe_delete に誤ったパスを一時的に渡すと新ゲートは FAILED、旧 trash-mode E2E はスキップ分岐で PASSED — 空虚性の主張を実証。break は revert、最終差分は tests/trash.rs のみ。
- テスト名修正: trash_mode_with_permanent_cleaner_does_not_claim_trash → uv_trash_mode_reports_logical_freed_when_trash_succeeds（本体が Logical freed: を主張するため旧名は虚偽）。両分岐維持。
- 空弁論削除: `!stdout.contains("moved to Trash")`（コードが決して出力しないリテラル）を削除。同パターンの既存3件（~177/210/283）は diff 対象外のため未触、報告に記録。
- 重要な発見（PBI 前提の逸脱、記録）: permanent モードの削除ゲートは既に存在した（tests/uv.rs:56-89、a09bafc で TCC 許容書き換え以前に追加）。「uv の実削除をカバーするテストが存在しない」は事実として古かった。新テストは tests/uv.rs に欠けていた no-Logical-freed の permanent-truth assert を追加。
- 検証: lib 650 passed / 4 failed（既知 TCC）、統合 36 ターゲット green（trash 9件含む）、clippy 0、fmt クリーン。
