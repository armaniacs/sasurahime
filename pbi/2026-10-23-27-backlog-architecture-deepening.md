# PBI: アーキテクチャレビュー deepening 候補の RICE 索引（2026-10-23）

improve-codebase-architecture による探索（コアフロー + cleaners/ 全 21k 行、file:line 実測検証）で発見された deepening 候補 8 件の採点と着手順。Reach = 影響を受けるクリーン経路の割合（%）。

## RICE 採点と最終順位

| 順 | 候補 | Reach | Impact | Conf | Effort(人日) | スコア | PBI | 根拠 |
|---|---|---|---|---|---|---|---|---|
| 1 | SafeDeleter 削除シーム | 100 | 3 | 100% | 1.5 | 200 | [20](<2026-10-23-20-fix-safe-deleter-seam.md>) | chflags 15 サイト × 3 セマンティクス、uses_trash 誤報告 16 クリーナー。**依存優先で先頭**（DeleteOutcome が他候補の土台シーム） |
| 2 | resolve_env_dir | 40 | 3 | 100% | 0.5 | 240 | [21](<2026-10-23-21-fix-env-cache-dir-resolver.md>) | スコア1位だが土台シーム待ち。PBI 19 をクラスとして閉じる |
| 3 | detect_all 統一 | 90 | 1 | 100% | 0.5 | 180 | [22](<2026-10-23-22-fix-detect-all-dedup.md>) | rayon ブロックの 3 重複、2 クローンは未テスト |
| 4 | run_batch | 80 | 2 | 80% | 1.5 | 85 | [23](<2026-10-23-23-feat-clean-batch-module.md>) | 履歴ガード 6 サイト、sub-target の履歴欠落 |
| 5 | TargetSpec | 60 | 2 | 80% | 2.0 | 48 | [24](<2026-10-23-24-fix-special-target-registry.md>) | 5 sync points + keep=2 乖離。構築統一に絞る |
| 6 | globals cleanup | 50 | 1 | 80% | 0.5 | 80 | [25](<2026-10-23-25-fix-dead-globals-cleanup.md>) | 削除テストで消失する純粋削除 2 件 |
| 7 | dedup bundle | 30 | 1 | 80% | 1.0 | 24 | [26](<2026-10-23-26-fix-cleaner-dedup-bundle.md>) | ~200 行重複 + gradle symlink ギャップ |
| 8 | uv/brew/hf/pre_commit テーブル化 | 20 | 2 | 50% | 2.0 | 10 | なし（後日候補） | 値/工数が最低。PBI 20 の DeleteOutcome 落地後に再評価 |

## 依存による順序調整

スコア順では resolve_env_dir(240) > SafeDeleter(200) だが、SafeDeleter の `DeleteOutcome` が候補 2/6/8 の土台シームであり、generic.rs 等のファイル重複があるため **依存はスコアより優先** で実装順 = ファイル名順（20 → 21 → 22 → 23 → 24/25 並列 → 26）とした。

## 実装バッチ計画（autonomous-task-closer 用）

- **バッチ1（直列・土台）**: PBI 20（trash.rs / cleaner.rs / 全 cleaner + hint）
- **バッチ2（並列・ファイル非重複）**: PBI 21（generic env ブロック + huggingface + pre_commit）∥ PBI 22（scanner + interactive）
- **バッチ3（並列）**: PBI 23（main.rs + interactive.rs）∥ PBI 26（gradle / library_logs / app_support / orphan / hint/detector）
- **バッチ4（並列）**: PBI 24（main.rs + cleaner.rs）∥ PBI 25（context / progress / scanner / explorer / hint/cleaner）

既存の PBI 19（HF_HOME ルートパス）は PBI 21 の実装で閉じる。

## 後日候補（PBI 化せず、再評価待ち）

- uv/brew/huggingface/pre_commit の CleanerSpec テーブル化（候補 8）― PBI 20 落地後に再評価
- filter_support() の trait データ化、TUI/explorer の自己再実行の in-process 化、HOME 3 系統解決の統一、サイズパーサ 3 実装の統一、共有 interactive スケルトン

## 再評価 (2026-10-24)

file:line 実測の再評価（読み取り専用エージェント）で、後日候補 6 件を判定した:

| 項目 | 判定 | 根拠 |
|---|---|---|
| 候補 8: uv/brew/hf/pre_commit テーブル化 | RESOLVED | 4 つとも define_cleaners! 表の単一行（main.rs:100-104, 181-185）。表外の特殊 dispatch は 0 件。hf/pre_commit は CliFallbackConfig データアダプタ済み。残存は約 55 行のアダプタ boilerplate のみ（スコア約 4、PBI 不値） |
| filter_support() の trait データ化 | RESOLVED | main.rs:634-647 が trait メソッド per_cleaner_filters()（cleaner.rs:239-240、override generic.rs:386/646, log.rs:132）を照会済み |
| TUI/explorer 自己再実行の in-process 化 | STILL REAL | interactive.rs:256-258（bytes_freed: 0 捏造で Total freed 過小表示）+ explorer.rs:245+278-290。サブターゲット掃除の in-process シームなし。スコア約 11 |
| HOME 3 系統解決の統一 | PARTIAL | 4 サイト 2 セマンティクス + generic.rs:851-855 の stale コメント（PBI 47 の修正で main.rs:482 は空 HOME をフィルタするようになり、コメントの主張が偽になった）。スコア 64 |
| サイズパーサ 3 実装の統一 | STILL REAL | brew.rs:23-50（堅牢）と ollama.rs:60-74（弱いコピー、ゴミ入力で黙って 0）を統一。apfs_snapshot.rs:64-95 は tmutil 規約の 10 進版として別実装に保つ。スコア約 5 |
| 共有 interactive スケルトン | PARTIAL | batch シームと純粋ヘルパー抽出は済み。残りは y/N 確認・set_skip_confirm ブラケット・バッチ報告印刷の 2 重複（約 40-50 行）。スコア 32 |

### 採用とバッチ計画（2026-10-24）

- PBI 化（RICE 順）: [48](<2026-10-24-48-fix-home-resolution-unify.md>)(64) > [49](<2026-10-24-49-fix-interactive-skeleton-shared-helpers.md>)(32) > [50](<2026-10-24-50-fix-tui-explorer-in-process-clean.md>)(約 11) > [51](<2026-10-24-51-fix-size-parser-unify.md>)(約 5)
- バッチ A（並列・ファイル非重複）: PBI 48（main.rs / generic.rs / config.rs / history.rs）と PBI 49（interactive.rs）と PBI 51（brew / ollama / apfs_snapshot / format）
- バッチ B（直列・A の main.rs / interactive.rs マージ後）: PBI 50（interactive.rs / explorer.rs / main.rs）
- 候補 8 と filter_support は PBI 化せず RESOLVED として閉じる（残存 boilerplate は将来の cleanup に繰り入れ）

## 報告書

HTML レポート: `$TMPDIR/architecture-review-20261023-214446.html`（リポジトリ外、Tailwind + Mermaid）
