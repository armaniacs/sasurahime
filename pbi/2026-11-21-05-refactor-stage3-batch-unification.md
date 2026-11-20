出所: pbi-backlog-prioritizer（2026-11-21）における round 10 台帳送り「run_batch 引数リスト全面移行」の PBI 化。着手順 2 位。実測アンカーは main @ d206d0c。

# PBI 2026-11-21-05: stage 3 統合（form enum + BatchOptions 全面移行 + report_single_target を同一 PR で）

- 種別: refactor（挙動不変・両エンジンの出力 byte 等価）
- 規模: M
- 優先度: 2 位 / 全 7 候補（RICE 2.13 = Reach 4 × Impact 2 × Confidence 0.8 / Effort 3 SP）
- 根拠: round 7 から意図的に延期されてきた 2 エンジン表示分岐の解消。BatchSummary の `results` / `failures` carry（src/batch.rs:31-36）は統合のための意図的な準備であり、round 10 の BatchOptions 部分適用で全面移行の要件（round 7 契約）が実施可能な形状になった
- 依存: なし（ただし PBI 2026-11-21-04 と同時実装する場合はファイル排他で並列可）

## 背景（実測済みの事実。推測で書き足さない）

- **round 7 記録契約**（src/batch.rs:82-90）: 「a `run_batch` argument-list change takes the form enum plus stage 3 in the same PR」— 本 PBI はこの契約が求める束ね（form enum + BatchOptions + stage 3）を 1 PR で実施する
- **stage-3 carry**（src/batch.rs:31-36）: `failures` は anyhow::Error のチェーン保持、`results` は成功 CleanResult の順序保持 — どちらも「single/batch 統合が single 契約を再構築する」ための意図的な準備で、今日の表示読み手は report_batch_failures のみ
- **2 エンジンの行家族表**（src/batch.rs:59-66）: status line / cancel line / note line / freed line / skip lines / exit code の 6 家族が single（run_clean_target、main.rs:894-986 付近）と batch（run_batch、src/batch.rs:91）で別所有者。gate の非対称（animated は caller フラグ、with_notes && show_spinner、batch always exit 0 vs single exit 1）は表で契約化済み
- **stage-3 trap**（src/batch.rs:76-80）: report_single_target が `results[0].name` を表示・履歴に使ってはならない（CleanResult.name と batch ループの c.name() の同値性は pin されていない非不変条件）— 統合時の最重要の落とし穴
- round 10 で BatchOptions は run_selected_batch 側に部分適用済み（.plan/archived/2026-11-21-01）。本 PBI で `run_batch` 本体（src/batch.rs:91-98）も BatchOptions 化し、残る直接呼び出し 2 箇所（run_caches_batch: main.rs:1018、run_interactive whole-target: interactive.rs:520 付近）も named 化する
- 単一表示経路の現状: run_clean_target の [OK]/[FAILED]/note/freed は main.rs の show_spinner() 分岐内にインライン実装

## ユーザーストーリー

As a 保守担当者, I want one display engine for both single-target and batch runs, so that a new line family or gate change lands in one place instead of silently diverging between the two paths.

## BDD 受け入れシナリオ

Scenario: 両エンジンの byte 等価（ハッピーパス）
  Given 現行の single / batch の全出力 pin（E2E 群）
  When report_single_target への統合を実施する
  Then 全行家族の出力・終了コード・履歴記録が現行と byte 等価（既存 pin 無修正で green）

Scenario: ゲートの非対称の維持（エラー/境界ケース）
  Given batch の with_notes=false 経路（run_caches_batch）と single の show_spinner() 分岐
  When 統合後の report_single_target が両者を処理する
  Then note 行の gate（`--deep-suppress` で非表示、default と `--suppress` で表示）と batch always exit 0 / single exit 1 の契約（PBI 78 scope fence 2）が現行どおり保たれる

## 受け入れ基準

- [ ] `form enum` を導入し、2 エンジンの行家族出力を report_single_target（stage 3）に統合する。行家族表（batch.rs:59-66）を統合後の所有者で更新する
- [ ] `run_batch` 本体の引数リストを BatchOptions 化する（round 7 契約の許可する形で本 PR 内で実施）し、run_caches_batch（main.rs:1018）と run_interactive whole-target 分岐（interactive.rs:520 付近）も named 化する
- [ ] stage-3 trap を遵守: report_single_target の表示・履歴ラベルは batch ループの cleaner 名から取り、`results[0].name` を使わない。この非不変条件についての pin（意図的な source pin または doc）を残す
- [ ] 統合の前に両エンジンの出力 pin を棚卸しし、不足分を先に足す（parity pin 規律: pin してから統合）
- [ ] 96 セル matrix pin（src/unattended.rs）、selection matrix、既存 E2E 全て無修正で green
- [ ] `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean + `cargo build` green
- [ ] 触れないこと: src/unattended.rs の select / matrix（読むだけ）、safety.rs / trash.rs の削除シーム

## テスト戦略

- E2E: 既存 pin の無修正 green が byte 等価の主証拠。棚卸しで不足した single 側の行家族 pin（例: 単一ターゲットの [SKIPPED] 行、quiet reporter の [FAILED] 抑制）を統合前に補完
- 統合: batch.rs tests の run_batch / run_selected_batch 群に report_single_target の unit pin（form enum の全バリアント網羅）を追加
- 単体: report_single_target の gate 組み合わせ（show_spinner × animated × with_notes × suppress）の truth table pin

## 見積もり

3 SP（form enum 設計 + report_single_target 新設 + BatchOptions 全面移行 + pin 棚卸し・補完。要チームでの見積もり）

## Definition of Done

- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] cargo test green（既知 TCC 依存 4 失敗を除く許容）+ clippy 0 + fmt clean + build green
- [ ] コードレビュー完了（統合側がバッチ境界で検証）
- [ ] 行家族表（batch.rs doc）が統合後の所有者に更新済み
- [ ] 本 PBI のアーカイブ（統合側が実施）

## scope 外（やらないこと）

- run_selected_batch / SkipAdvice の再設計（round 9・10 で着地済み、統合の下位互換として維持）
- Option A に含まれない新機能（新しい行家族の追加は別 PBI）
- `CleanOptions` と BatchOptions の統合検討（別概念）
- safety.rs / trash.rs の削除シーム変更（本 PBI は表示統合のみ）
