# バックログ索引: アーキテクチャ改善ラウンド 4（2026-10-31）

出典: 持ち越し 4 項目のなぜなぜ分析（3 立場 × 32〜36 段 × 4 項目 = 連鎖合計 395 段、全文は .plan/whywhy-round4-*.md 4 ファイル、gitignored）

## RICE 索引

| PBI | タイトル | R | I | C | E（人週） | スコア | 判定 |
|---|---|---|---|---|---|---|---|
| 90 | バッチ経路の note 出力を reporter 条件で統一 | 2 | 2 | 95% | 0.1 | 38.0 | 実装 |
| 89 | symlink スコープ検査を全削除経路に統一 | 3 | 3 | 90% | 0.5 | 16.2 | 実装 |
| 91 | hint の選択プロンプトと dry-run に削除先パスを表示 | 2 | 2 | 85% | 0.25 | 13.6 | 実装 |
| 88 | cache-dir resolution を constructor 時の 1 回に移す（依存注入） | 3 | 3 | 95% | 1.5 | 5.7 | 実装（guard 追加の対案を上回る構造的解として優先） |

補足: PBI 88 の素の RICE は対案（guard 6 本追加、E 0.5）より低く出るが、対案は「欠落が必ず再発する完了しないパッチ + 実データ削除ハザードを防げない」ため構造的解を上位に置く（各 PBI ファイルの判断節を参照）。

## 分析の裁定（実装しない項目とその理由）

- **KNOWN_ENTRIES の config 化**: 見送り。3 立場が合致（攻撃面の二重締め崩壊、`&'static` 構造の波掀、需要小 + 既存の安い手段 [[custom]]/Issue/plugin）。条件付き: 需要 3 件以上が材料化したら「config は追加のみ・既定 22 件の上書き不可」の中間解 PBI を起動。動作変更ゼロの小タスクとして hint/apps.rs への vetted 契約コメント追記が残る。
- **hint と cleaner の役割分担の統合**: 維持。hint は proactive・quit-aware、cleaner は scan-driven という design-it-twice は正当。重複のうち解消すべきは symlink ガード非対称（PBI 89）、二重計上と freed 3 種（Phase 2/3 として残す）。
- **Option A 完全統合**: 先送り維持。ただし発火条件を修正: 「次に新 entry point」→「次にエンジンの意味論（行家族の追加/gate の移動/gate 条件の変更）を触る変更」。前段 3 段階: failures を BatchFailure{name, anyhow::Error} に → BatchSummary に results 追加 → run_clean_target を run_batch(1 要素) + report_single_target レイヤーに置換。
- **R5 単発 cancel 中立行**: 判断待ち（現行維持も成立）。

## バッチ計画（closer 実行用）

| バッチ | PBI | worktree | マージ順 | 根拠 |
|---|---|---|---|---|
| 1 | 88 / 89 / 90 / 91（並列 4 体） | .worktrees/pbi-88 〜 pbi-91、ブランチ pbi-88 〜 pbi-91 | 88 → 89 → 90 → 91 | 88 と 89 は src/safety.rs を共有（88 は doc :83-98、89 は新設ヘルパ）だが別領域、89 と 91 は src/hint/cleaner.rs を共有（89 は auto_clean_hint :111-164、91 は prompt/dry-run :298-394）だが別関数。順次マージで非交差 |

## 次ラウンドの持ち越し候補

1. Option A 前段 3 段階（新発火条件付き、上記裁定参照）
2. KNOWN_ENTRIES 条件付き中間解（需要 3 件以上で発火）+ hint/apps.rs vetted 契約コメント（動作変更ゼロ）
3. hint 重複 Phase 2/3: hint_owned_paths() を KNOWN_ENTRIES から機械導出し scanner.rs の Total 計算で hints 別枠表示、app_support/library_caches の候補 render に [managed by hints] タグ、freed 計測の削除時再計測統一
4. R1/R2/R3/R6/R7 残差分: failures が String でエラー本体を運ばない、単発 exit 1 vs バッチ exit 0、freed 文言の 2 軸差、skip 行の名前有無、戻り値型
5. library_caches.rs known_entry_for の構造一致が複数 suffix エントリで Unknown 分類になる（KNOWN_ENTRIES 分析の副産物）
6. docs/HOWTO-USE.md:88/:672 のプロンプト文言例示（PBI 91 実装時に更新要否を判定）
