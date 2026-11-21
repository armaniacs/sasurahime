出所: pbi-backlog-prioritizer（2026-11-21）で起票後、pbi-create-bdd-workspace（ryuzee × BDD × t_wada 統合）テンプレートへ書き直し（2026-11-21）。実測アンカーは main @ d206d0c。

# PBI 2026-11-21-05: stage 3 統合（form enum + BatchOptions 全面移行 + report_single_target を同一 PR で）

## ユーザーストーリー

保守担当者として、single-target と batch の行家族表示が 1 つの表示エンジン（report_single_target）に統合される機能がほしい、なぜなら新しい行家族や gate 変更が 2 箇所ではなく 1 箇所に落ち、2 エンジンの静かな分岐という構造リスクを解消できるから。

## ビジネス価値

- **価値**: 6 行家族（status / cancel / note / freed / skip / exit code）の変更が 1 所有者に届く状態。stage-3 carry（src/batch.rs:31-36）が意図的に準備してきた統合を実現し、「次の gate 変更で 2 エンジンが静かに分岐する」リスクを消す
- **測定方法**: 両エンジンの全行家族出力 pin（E2E + 単体）が無修正で green、行家族表（batch.rs doc）の所有者列が 1 エンジンに統一、以後の行家族変更の diff が 1 箇所に現れる

## 優先度

- 順位: 2 位 / 全 7 候補（pbi-backlog-prioritizer 2026-11-21）
- RICE スコア: 2.13（Reach=4 / Impact=2 / Confidence=0.8 / Effort=3 SP）
- 根拠: round 7 から意図的に延期されてきた統合で、Confidence 0.8 は report_single_target 形状の設計判断が残るため。PBI 04（RICE 4.0）が 1 位、本 PBI はファイル排他で並列可
- 依存: なし（round 7 契約の 1 PR 束ね要件: form enum + BatchOptions 全面移行 + stage 3 を**同一 PR** で実施する）

## BDD 受け入れシナリオ

```gherkin
Scenario: 両エンジンの byte 等価（ハッピーパス）
  Given 現行の single / batch の全出力 pin（E2E 群）
  When report_single_target への統合を実施する
  Then 全行家族の出力・終了コード・履歴記録が現行と byte 等価

Scenario: ゲートの非対称の維持（エラー/境界ケース）
  Given batch の with_notes=false 経路（run_caches_batch）と single の show_spinner() 分岐
  When 統合後の report_single_target が両者を処理する
  Then note 行の gate（--deep-suppress で非表示、default と --suppress で表示）と batch always exit 0 / single exit 1 の契約が現行どおり保たれる

Scenario: stage-3 trap の遵守（境界ケース）
  Given 統合後の report_single_target が成功した cleaner の CleanResult を持つ
  When 表示・履歴ラベルを構成する
  Then ラベルは batch ループの cleaner 名から取り、results[0].name は使われない
```

## 深掘り裁定（deep-dig 2026-11-21・実装の前提）

1. **統合機構の形状は実装判断（round 7 契約を目的に修正）**: 契約文の「form enum plus stage 3」は手段まで指定していたが、目的（行家族の 1 所有者化）と手段を分離し、「**統合機構**（form enum / 引数構造体 / 小関数群のいずれでもよい）plus stage 3」に文言を修正する。実装時に batch.rs:82-90 の doc を修正し、変更理由（3 種 spinner mechanics の実測・byte 等価検証の単位）を記録する。1 PR 束ね要件（BatchOptions 全面移行 + stage 3 同時）は維持する
2. **行家族表 #6（exit code）は統合対象外**: single = CLI エラー慣習（exit 1）、batch = report-only（always 0）は本質的に異なるセマンティクス（PBI 78 fence 2 で pin）。表示 5 家族のみ統合し、#6 は行家族表に「意図的な非対称」として記録する
3. （PBI 04 で確定: scan-cache リセットは lock なし swap・グローバルを触るテスト 2 本目で lock 再判断 — 本 PBI とは無関係）

## 受け入れ基準

- [ ] **統合機構**（form enum / 引数構造体 / 小関数群のいずれか — 形状は実装判断、truth table pin で 1 所有者を検証）を導入し、2 エンジンの 6 行家族のうち**表示 5 家族**（status / cancel / note / freed / skip）を report_single_target（stage 3）に統合する。行家族表（src/batch.rs:59-66）を統合後の所有者で更新する
- [ ] **round 7 契約文言の修正**: src/batch.rs:82-90 の「form enum plus stage 3」を「統合機構 plus stage 3」に修正し、変更理由（deep-dig 裁定: 目的と手段の分離、3 種 spinner mechanics の実測）を doc に記録する。1 PR 束ね要件は維持
- [ ] **#6（exit code）を統合対象外として行家族表に明記**: batch always exit 0 / single exit 1 は「意図的な非対称」（PBI 78 fence 2）として記録する
- [ ] `run_batch` 本体の引数リストを BatchOptions 化する（修正後契約が本 PR 内での実施を要求）し、残る直接呼び出し 2 箇所（run_caches_batch: main.rs:1018、run_interactive whole-target: interactive.rs:520 付近）も named 化する
- [ ] stage-3 trap を遵守: 表示・履歴ラベルは batch ループの cleaner 名から取り、`results[0].name` を使わない。この非不変条件の pin（意図的な source pin または doc）を残す
- [ ] 統合の**前に**両エンジンの出力 pin を棚卸しし、不足分（single 側の [SKIPPED] 行、quiet reporter の [FAILED] 抑制等）を先に足す
- [ ] 96 セル matrix pin（src/unattended.rs）、selection matrix、既存 E2E 全て無修正で green
- [ ] `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean + `cargo build` green

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 既存 pin の無修正 green が byte 等価の主証拠（auto_policy / clean_all / scan / trash / vm_swap / xcode / interactive）
- 棚卸しで不足した single 側の行家族 pin（[SKIPPED] 行、quiet reporter の [FAILED] 抑制、note gate）を統合**前**に補完

### 統合テスト
- batch.rs tests の run_batch / run_selected_batch 群に report_single_target の unit pin（**統合機構の truth table** — 表示 5 家族 × gate 組み合わせを網羅。BatchSummary の carry 消費）
- 履歴記録の契約（write_history_entry_if_needed の 1 回ガード）が両経路で維持される pin

### 単体テスト
- report_single_target の gate 組み合わせ truth table（show_spinner × animated × with_notes × suppress のうち行家族に効く面。#6 exit code は統合外のため対象としない）
- CleanCancelled の無トークン契約（CancelClassified 経由）の境界値 pin

## 実装アプローチ

- **Outside-In**: 行家族表（batch.rs:59-66）が契約 → 両エンジンの出力 pin 棚卸し・補完（失敗なし、現行 green を pin）→ 統合機構設計（truth table から導出、形状は実装判断）→ report_single_target 実装 → batch / single 呼び出し側の切替 → green → 行家族表の所有者列と round 7 契約文言を更新
- **Red-Green-Refactor**: report_single_target の truth table pin を先に Red で書き、統合で Green。byte 等価 pin は常に green に保ち、red になったら統合の誤りとして扱う
- **リファクタリング**: Green 後に 2 エンジンの旧表示コード（main.rs show_spinner 分岐のインライン表示、batch.rs の分岐）を削除し、行家族表と契約文言を更新

## 見積もり

3 SP（要チームでの見積もり）

## 技術的考慮事項

- 依存関係: なし（PBI 04 と同時実装する場合はファイル排他成立・並列可）。PBI 2026-11-21-01（BatchOptions 部分適用、済）が前提の足場
- テスタビリティ: reporter は既に注入可能（ProgressReporter の dyn シーム）。行家族表の gate 列を truth table に写像する
- 非機能要件: 性能影響なし（表示経路の統合のみ）。セキュリティ影響なし（削除シーム safety.rs / trash.rs には触れない）

## 実装者向け注記

### 現状コードの確認
（着手前に必ず実行すること — 2026-11-21 に実施済み・未実装を確認）
```bash
rg -n 'report_single_target|FormKind|ReportForm' src/
rg -n 'form enum' src/batch.rs
sasurahime targets
```
- **既実装確認の結果**: `report_single_target` は doc コメント内のみ（batch.rs:11/71/76）で関数は未実装、form enum（`FormKind` / `ReportForm`）は 0 件 → **未実装**（BatchSummary.results / failures の stage-3 carry が準備済み）。コマンド面は 51 ターゲットで、本 PBI がコマンドを追加しないことも確認済み

### 実装手順
1. pin 棚卸し: batch.rs:59-66 の行家族表を行ごとに single / batch の現行出力で pin し、不足分（single の [SKIPPED]、quiet の [FAILED] 抑制）を tests に先に足す
2. round 7 契約文言の修正（batch.rs:82-90）: 「form enum plus stage 3」→「統合機構 plus stage 3」に変更し、変更理由（deep-dig 裁定）を doc に記録
3. 統合機構の設計: 行家族表の「Gate」列が切り替え条件（animated / show_spinner / with_notes / suppress / dry_run）— truth table から表示 5 家族の出力責務を report_single_target に集約する形を導出（enum / 引数構造体 / 小関数群の選択はこの時点で裁定し、理由を doc に残す）
4. report_single_target を src/batch.rs に新設: stage-3 carry（results / failures / skipped / cancelled）を消費し、single 契約（Err 再構築・exit_code()==1 の生存）を再構築する
5. run_batch 本体の BatchOptions 化 + run_caches_batch / run_interactive whole-target の named 化（修正後契約により本 PR 内で実施）
6. run_clean_target の表示部分を report_single_target へ寄せ、main.rs のインライン表示を削除
7. stage-3 trap の pin: `CleanResult.name` と `c.name()` の非同値性が pin されていない旨の source pin（または doc）を batch.rs に残す
8. 行家族表（batch.rs:59-66）の所有者列を統合後の形に更新（#6 は「意図的な非対称」記載）

### 落とし穴
- **stage-3 trap**: `results[0].name` を表示・履歴に使うと、name 非不変条件の下でラベルが静かに壊れる（batch.rs:76-80）。ラベルは必ず batch ループの `c.name()` から
- **animated は caller フラグ**: `reporter.show_spinner()` から導出してはならない（no-tick-while-prompting 契約、batch.rs:44-53）。form enum に animated を独立バリアントとして残す
- **exit code 契約**: batch always exit 0（PBI 78 scope fence 2）を統合で壊さない。run_and_exit が exit(1) を担う single 側との非対称は行家族表の #6 が契約
- **pin を壊す統合は byte 等価の違反**: 既存 E2E pin が red になったら「統合の誤り」と扱い、pin を修正するのは出力意図が変わる場合のみ
- **run_selected_batch との整合**: round 10 で部分適用済みの BatchOptions に形を揃える（2 種の options 型を残さない）

## Definition of Done

- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] cargo test green（既知 TCC 依存 4 失敗を除く許容）+ clippy 0 + fmt clean + build green
- [ ] コードレビュー完了（統合側によるバッチ境界検証。PR 運用時は approve 必須）
- [ ] リファクタリング完了（グリーン後: 旧 2 エンジン表示コードの削除と行家族表の更新）
- [ ] ロールバック手段: byte 等価リファクタリングのため `git revert`（1 PR）で完全に切り戻せる。挙動変更・閾値変更は含まない
- [ ] ドキュメント更新済み（行家族表の更新が doc 更新を兼ねる。README / docs には触れない）

## scope 外（やらないこと）

- run_selected_batch / SkipAdvice の再設計（round 9・10 で着地済み、統合の下位互換として維持）
- 新しい行家族の追加（別 PBI）
- `CleanOptions` と BatchOptions の統合検討（別概念）
- safety.rs / trash.rs の削除シーム変更（本 PBI は表示統合のみ）
