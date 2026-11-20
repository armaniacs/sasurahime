出所: holistic-code-improvement round 10（継続ラウンド・差分スコープ）。なぜなぜ分析は本ラウンドでは簡略（ラウンド 9 の分析と code-review 指摘の継承）。
フェーズ0 確定値: 検証コマンド = `cargo check` / `cargo clippy --tests -- -D warnings` + `cargo fmt --check` / `cargo test` / `cargo build`。アーカイブ先 = `.plan/archived/`。INDEX = `.plan/00-INDEX.md`。
DoD 反映漏れ棚卸し: `pbi/` 直下に PBI ファイルなし → 対象 0 件。
トリガー発火: Cleaner trait 再評価（11 → 12 メソッド）、--yes 語彙（PBI 03 着地）。未発火: tests/ fixture 監査、`SCAN_CACHE.set` 脆弱点。

# 2026-11-21-00-backlog-holistic-round10 — 大局的コード改善 round 10 台帳

## RICE 採点表（全候補 1 表）

| # | 候補 | PBI | Reach | Impact | Confidence | Effort | RICE |
|---|---|---|---|---|---|---|---|
| 1 | BatchOptions 構造体化（部分適用）+ bounds debug_assert | 2026-11-21-01 | 4 | 1 | 1.0 | 1 SP | 4.0 |
| 2 | --yes skip 行への語彙付与 | 2026-11-21-02 | 4 | 0.5 | 1.0 | 1 SP | 2.0 |
| 3 | Cleaner trait capability split 裁定 | 2026-11-21-03 | 3 | 0.5 | 0.8 | 1 SP | 1.2 |

## 実行順と逸脱理由

| バッチ | 内容 | PBI | 担当ファイル |
|---|---|---|---|
| Batch 1（並列 2 件） | BatchOptions 部分適用 ∥ trait 裁定 | 01 ∥ 03 | `src/batch.rs` + `src/main.rs` + `src/interactive.rs` ∥ `src/cleaner.rs` |
| Batch 2（直列） | --yes 語彙付与 | 02 | `src/interactive.rs` + `tests/scan.rs` |

- 純 RICE 順: 1 (4.0) > 2 (2.0) > 3 (1.2)。**逸脱**: 候補 3（3 位）を候補 2（2 位）より先に実行 — 候補 2 は候補 1 への同一ファイル依存（interactive.rs）で Batch 2 待ちが確定しており、Batch 1 の空きスロットに候補 3（cleaner.rs は無関係・並列可）を置くことで壁時計を浪費しない。
- **スコープ裁定（ユーザー確認済み・部分適用）**: `run_batch` 本体の引数リスト変更は round 7 の記録契約（`src/batch.rs:82-90`「argument-list change takes the form enum plus stage 3 in the same PR」）に抵触するため、BatchOptions 化は round 9 の新シーム `run_selected_batch` とその呼び出し面（無人 3 経路）に限定し、`run_batch` 本体は据え置く。

## 台帳送り（再検討トリガー付き）

| 候補 | 出所 | 再検討トリガー |
|---|---|---|
| `run_batch` 本体の引数リストの BatchOptions 全面移行 | 本ラウンドの部分適用裁定 | **stage 3（single/batch 表示統合）統合 PR と同時に実施する**（round 7 契約の要件）。form enum + BatchOptions + stage 3 を 1 PR に束ねる |
| exit-path 列挙 pin（フロー内 `std::process::exit` が末尾 flush を迂回する旨の集約 doc） | code-review round 10 指摘 | フロー関数が 15 を超えたら |
| RFC3339 形状 helper の統合（tests/history.rs と src/history.rs tests の同型ロジック） | code-review round 10 指摘 | 同型形状 pin が 3 箇所に増えたら |
| tests/ 42 ファイルの fixture 重複全数監査 | round 9 から引継 | explore 系タスクの際に併走 |
| `let _ = SCAN_CACHE.set(...)` テストリセット脆弱点 | round 9 から引継 | scan_cache テスト基盤に触る機会 |

## 積み残しセクション

- クリーナー内部実装（uv/brew/mise 等の clean 本体）の個別監査 — round 1〜8 は seams/計上視点で、clean 本体のアルゴリズム詳細は未踏破。次ラウンド予約なし（トリガー: 特定クリーナーへの変更依頼時に併走）
- `installed_apps` / `disk_image` / `vm_swap` / `apfs_snapshot` / `orphan_containers` の小モジュール詳細監査。次ラウンド予約なし
- graphify-out が 2026-10-30 生成で陳腐化 — 次ラウンドで全体把握に使うなら再生成を推奨

## 経過記録

- 2026-11-21: フェーズA〜C 完了。ユーザー確認 3 回（候補リスト → RICE 承認 → run_batch 契約発見による部分適用裁定）。
- 2026-11-21: フェーズD〜F 完了。3 PBI 実装・検証・アーカイブ済み（01 → `3bc8a6f`、03 → `c9afc3e`、02 → `0796f45`。バッチ構成 2 バッチ・Batch 1 は 2 体並列）。Batch 2 で統合側が --yes のラッパー行の理由二重表現を発見し、header から理由を外して per-target 行に所有者を一本化（1 行追加調整）。最終ゲート: workspace 1289 passed 0 failed、clippy 0、fmt clean、cargo build ok。未完了 0。
