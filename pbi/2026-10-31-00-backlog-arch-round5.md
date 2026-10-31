# バックログ索引: アーキテクチャ改善ラウンド 5（2026-10-31）

出典: 持ち越し 6 項目のなぜなぜ分析（3 立場 × 32 段 × 3 グループ = 連鎖合計 288 段、全文は .plan/whywhy-round5-*.md 3 ファイル、gitignored）

## RICE 索引

| PBI | タイトル | R | I | C | E（人週） | スコア | 判定 |
|---|---|---|---|---|---|---|---|
| 92 | BatchSummary に failures と results の運搬型を載せる（段 3 統合の準備） | 2 | 2 | 95% | 0.2 | 19.0 | 実装（準備 PBI、R1 実害ゼロだが段 3 の攻撃面を構造的に消す予防投資） |
| 94 | hint 管轄パスを scanner と候補画面で可視化する | 3 | 2 | 85% | 0.5 | 11.9 | 実装（式どおりは 10.2、出典値 11.9 を主値） |
| 93 | app-support と library-caches の freed を削除時の実測に統一する | 2 | 1.5 | 90% | 0.25 | 10.8 | 実装（library-caches の 1 時間窓が実害） |

小粒タスク（PBI 化不要、動作変更ゼロ、contracts worktree で実装済み）:
- docs/HOWTO-USE.md:88/:672 のプロンプト文言例示を実際の出力に同期（PBI 91 の漏れ残務）
- src/hint/apps.rs への vetted 契約コメント追記（gitignored な PBI 文書にしかなかった契約の一次情報をコードに収束）
- library_caches.rs の単一 suffix 不変条件の doc 明文化 + 構造テスト 1 本（複数 suffix の Caches エントリを書くと fail）
- README への部分成功 exit 0 契約の明記（R2 残差分の緩和）+ pbi/AGENTS.md への所有者表更新ルール 1 行

## 分析の裁定（実装しない項目とその理由）

- **Option A 段 3（run_clean_target 置換）**: 発火条件まで待つ（条件: batch.rs doc の「エンジンの意味論を触る変更」or 明示的実装指示。前段 PBI-92 に依存）。R2 exit code 差は意図的契約、R3/R6 は報告 vs 進行出力の概念差（所有者表に文書化済み）、R5 は現行維持、R7 は段 1+2 で実体が満たされる。
- **KNOWN_ENTRIES 条件付き中間解**: 条件付き休止を維持（需要 3 件以上で発火、前置きの 3 照合統一が未解決で Box::leak の単位も確定不可）。
- **known_entry_for の contains 化**: 不採用（集約 threshold の per-dir 誤適用で悪化、Unknown 分類の現在の実害はゼロ）。

## バッチ計画（closer 実行用）

| バッチ | 対象 | worktree | マージ順 | 根拠 |
|---|---|---|---|---|
| 1 | contracts（小粒 5 項目） | .worktrees/contracts（実装済み） | 先行 | PBI と独立、docs/doc コメント/テスト 1 本 |
| 2 | 92 / 93 / 94（並列 3 体） | .worktrees/pbi-92 〜 pbi-94 | 93 → 94 → 92 | 93 と 94 は app_support.rs・library_caches.rs を共有（93 は apply_selection/delete_entries、94 は format_candidate_label/render）だが別関数領域。92 は batch.rs・interactive.rs で独立 |

## 次ラウンドの持ち越し候補

1. Option A 段 3（発火条件付き、PBI-92 前段に依存、受け入れ基準は分析の実装順 (3) を参照）
2. KNOWN_ENTRIES 条件付き中間解（需要 3 件以上で発火、draft PBI としての決定記録）
3. TUI "Will free approximately" の交差差し引き（app-support の sub_targets 公開 = UX shape 変更を伴う別 PBI）
4. docs/HOWTO-USE.md に「hints」の記載ゼロ（clean hints が HOWTO の clean <target> テーブルに載っていない）
5. 運用ルール「gate 変更 PBI は所有者表の該当行を同じ PR で更新」（pbi/AGENTS.md に追記済み、遵守のみ）
