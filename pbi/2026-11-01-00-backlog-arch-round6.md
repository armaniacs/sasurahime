# バックログ索引: アーキテクチャ改善ラウンド 6（2026-11-01）

出典: /improve-codebase-architecture 探索（2 subagent、候補 12 件）+ なぜなぜ分析（3 立場 × 30-34 段 × 3 グループ = 連鎖合計 300 段超、全文は .plan/whywhy-round6-*.md 3 ファイル、gitignored）

## RICE 索引

| PBI | タイトル | R | I | C | E（人週） | スコア | 判定 |
|---|---|---|---|---|---|---|---|
| 95 | ユーザーキャンセルがバッチエンジンで [FAILED] と印字される矛盾を閉じる | 1 | 3 | 90% | 0.1 | 27.0 | 実装（round 6 最上位、実在経路 clean all --include-manual × colima、未 test 経路を閉じる） |
| 96 | scan 総計の計上決定を measured_bytes に集約する | 2 | 1 | 85% | 0.15 | 11.3 | 実装（98 より先に着地: 計上決定と名前重複関係の「共有の居場所」を先に作る） |
| 97 | library-logs と orphan-containers の freed を削除時の実測に統一する | 2 | 1.5 | 90% | 0.25 | 10.8 | 実装（PBI 93 の scope 漏れを完成、library-logs の窓は人間待ちで無上限） |
| 98 | TUI の "Will free approximately" の重複計上を差し引く（tier 1） | 2 | 1.5 | 85% | 0.25 | 10.2 | 実装（除外ではなく重複除去、tier 2 は別 PBI に分離） |
| 99 | interactive-select の儀式を multi_select_or_skip に集約する | 2 | 1.5 | 85% | 0.3 | 8.5 | 実装（動作変更ゼロ、orphan_containers は不移行、display_name 一意性 pin を同時追加） |
| 100 | clean hints の subs 筋を KNOWN_ENTRIES の静的 lookup に寄せる | 2 | 2 | 80% | 0.5 | 6.4 | 実装（silent no-op 源 2 種の解消、docs 未掲載も同時解消） |

direct commit タスク（PBI 化不要、contracts worktree で実装済み、rationale branch で着地済み）:
- B6: 5 cleaner の auto_clean_allowed override の WHY コメントを risk() 側に移設（`09860bb`、override 削除なし、risk 再分類時の tripwire として維持）
- B4: tests/AGENTS.md に source-pin 運用規約の新節（`498f622`、検索起点は定義行、自己充足回避、behavior test 優先）

## 分析の裁定（実装しない項目とその理由）

- **A4（録画 adapter 導入）**: 不採用（仮説は反証された ― pin が見ているのは indicatif 内部と eprintln 出力で ProgressReporter のメソッド面に現れない。観測可能なものは RecordingReporter + 振る舞いテストが既に担う）
- **A5（run_batch 3 チャネル interface）**: 現行維持、Option A 段 3 発火時に必ず同梱（3 bool → 形態 enum）
- **CO1（Option A 段 3）**: 発火させない（round 6 の 5 finding はいずれも batch.rs:80-83 の発火条件に非該当。A2 = emission 分類、A1 = scanner、A6 = TUI 面、A5 = 温存）
- **CO2（KNOWN_ENTRIES 条件付き中間解）**: 条件付き休止を維持（需要 3 件以上で発火）
- **B6 の 5 override 削除**: 不採用（tripwire として妥当、理由移設のみ）
- **B5 の表統合・ID 化**: 不採用（display_name 一意性 pin のみ追加）

## バッチ計画（closer 実行用）

| バッチ | 対象 | worktree | マージ順 | 根拠 |
|---|---|---|---|---|
| 1 | rationale（B6+B4、direct） | .worktrees/rationale（実装済み） | 先行 | PBI と独立、コメント移設 + docs |
| 2 | 95 / 96 / 97 / 98 / 99 / 100（並列 6 体） | .worktrees/pbi-95 〜 pbi-100 | 95 → 96 → 98 → 97 → 99 → 100 | 96 と 98 は scanner.rs を共有（98 は 96 の決定関数を消費）。97 と 99 は library_logs.rs・orphan_containers.rs を共有（別関数領域）。95 は progress.rs・cleaner.rs・batch.rs、100 は hint/cleaner.rs・detector.rs・docs で独立 |

## 次ラウンドの持ち越し候補

1. Option A 段 3（発火条件付き、A2 修正済み helper と A1 の単一決定が土台になる、A5 を同梱）
2. TUI 見積もり tier 2（hint_owned_paths を使った path 単位の和集合、selection_mapping の path 運搬拡張が前提）
3. KNOWN_ENTRIES 条件付き中間解（需要 3 件以上で発火、B5 の join 方向と同時検討）
4. docs/HOWTO-USE.md の clean <target> テーブルに clean hints を載せる（PBI 100 で着地予定）
5. 運用ルール「gate を触る PBI は src/batch.rs の行家族所有者表の該当行を同じ PR で更新」（pbi/AGENTS.md に追記済み、遵守のみ）
