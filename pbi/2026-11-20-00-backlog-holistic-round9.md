# 2026-11-20-00-backlog-holistic-round9 — 大局的コード改善 round 9 台帳

出所: holistic-code-improvement スキルによる大局的レビュー（holistic-code-review の流儀に準拠した報告書、2026-11-20）。
実測裏取り: 全指摘に file:line 実測。なぜなぜ分析: `.plan/whywhy-round9-{scan-cache-growth,entry-dispatch,small-dup}.md`（3 立場 × 30 段超 × 3 グループ、gitignored）。
フェーズ0 確定値: 検証コマンド = `cargo check` / `cargo clippy --tests -- -D warnings` + `cargo fmt --check` / `cargo test` / `cargo build`。アーカイブ先 = `.plan/archived/`。INDEX = `.plan/00-INDEX.md`（git 追跡外・ローカル参照用）。
DoD 反映漏れ棚卸し: `pbi/` 直下にチェックボックス付き PBI なし → 対象 0 件。
継続ラウンド: `pbi/` 直下に進行中 PBI なし → 新規ラウンド。

## RICE 採点表（全候補 1 表）

| # | 候補 | PBI | Reach | Impact | Confidence | Effort | RICE |
|---|---|---|---|---|---|---|---|
| 1 | scan-cache flush 時の期限切れ破棄 + README/Privacy 同期 | 2026-11-20-01 | 3 | 1 | 1.0 | 1 SP | 3.0 |
| 3 | 無人バッチ骨格の 1 所有者化 + skip レンダラ 1 箇所化 | 2026-11-20-03 | 5 | 1 | 0.8 | 2 SP | 2.0 |
| 2 | main() の 11 フロー抽出 + flush 1 箇所化 + dry-run 解決 1 箇所化 | 2026-11-20-02 | 6 | 1 | 0.8 | 3 SP | 1.6 |
| 4 | expand_tilde 統一 + "~/" 接頭辞 source pin | 2026-11-20-04 | 3 | 0.5 | 1.0 | 1 SP | 1.5 |

- Reach: 今後 1 年の保守作業での関与頻度（6 = main.rs は全ラウンド最頻接触、5 = 無人フロー変更時に 3 サイト、3 = 対象ファイルは四半期頻度）
- Impact: 3 = 実害解消 / 2 = 大きい / 1 = 中（重複削減・規範化・構造的リソース規範）/ 0.5 = 小（混乱削減・将来防止）
- Confidence: 1.0 = コードで確定 / 0.8 = 設計判断が残る（seam 配置・opts 渡し形状）
- Effort: ストーリーポイント

## 実行順と逸脱理由

純 RICE 順: 1 (3.0) > 3 (2.0) > 2 (1.6) > 4 (1.5)。
**逸脱**: 候補 3 を候補 2 の後に実行する。理由: 候補 3 は main.rs の auto/all arm という同一領域を触るため候補 2 への順序依存が成立し（skill の依存優先ルール）、候補 2 で抽出した `fn run_<flow>` が候補 3 の共有化の当て先になる（diff 最小）。

| バッチ | 内容 | PBI | 担当ファイル |
|---|---|---|---|
| Batch 1（並列 2 件） | scan-cache prune ∥ expand_tilde 統一 | 01 ∥ 04 | `src/scan_cache.rs` + README ∥ `src/explorer.rs` + tests |
| Batch 2（直列） | main() フロー抽出 | 02 | `src/main.rs` |
| Batch 3（直列） | 無人バッチ骨格共有化 | 03 | `src/main.rs` + `src/batch.rs` + `src/interactive.rs` |

最終ゲート: `cargo test`（フルスイート）→ `cargo build`。バッチ境界は `cargo check` → clippy → 対象テスト（軽量）。

## 依存マップ

```
01 (scan_cache.rs, README) ── 独立
04 (explorer.rs, tests)   ── 独立（01 と並列可）
02 (main.rs)              ── 独立だが 03 と同一ファイルのため 03 の先行
03 (main.rs, batch.rs, interactive.rs) ── 02 に依存（同一領域）
```

## 台帳送り（再検討トリガー付き）

| 候補 | 出所 | 再検討トリガー |
|---|---|---|
| Cleaner trait の capability split 検討（cleaner.rs:328-419、required 3 + default 8、41 impl） | フェーズA 積み残し | default method が 11 を超えたら / 次ラウンドで再評価 |
| tests/ 42 ファイルの fixture 重複の全数棚卸し | フェーズA 積み残し | 次ラウンドで explore 系タスクの際に併せて監査 |
| `let _ = SCAN_CACHE.set(...)` のテストリセット脆弱点（scan_cache.rs:273、OnceLock 沈黙） | なぜなぜ round 9（1.b 24-31） | scan_cache のテスト基盤に触る機会が来たとき、context.rs の `with_test_lock()` 型ガードへ寄せる判断をする |
| --yes の skip 汎用 1 行への語彙追加（出力変更） | なぜなぜ round 9（2.a 7-13、2.c 17） | 候補 03 の着地後、共有レンダラの足場を使って出力変更 PBI として判断 |

## 積み残しセクション（フェーズA 出力制限由来）

- Cleaner trait 肥大化: 上記台帳送りに記録。次ラウンド予約は未確定（トリガー条件付き）。
- tests/ fixture 重複監査: 同上。
本ラウンドで TOP 4 テーマに集中し、積み残しは次ラウンドの判断事項とする（RICE 対象外）。

## 品質規律（本ラウンド適用）

- byte-identical: 候補 02/03/04 は全て挙動不変。移設前に parity pin（出力 byte 等価）の棚卸し。
- SSOT 登録: 該当なし（新規ファイルなし・命名規約リストなし）。
- PBI 毎コミット、`git add` は対象ファイル個別、Conventional Commits・日本語。
- サブエージェント: git 操作・INDEX 編集・pbi/ 編集は禁止（統合側が行う）。

## 経過記録

- 2026-11-20: フェーズA〜C 完了。ユーザー確認 2 回（候補リスト → なぜなぜ分析 30 段の指示 → round 9 分析実施 → 採点表承認）。
- 2026-11-20: フェーズD〜F 完了。4 PBI 実装・検証・アーカイブ済み（01 → `634b00b`、04 → `d5d967a`、02 → `c9924ad`、03 → `2270865`。バッチ構成 3 バッチ・Batch 1 は 2 体並列）。最終ゲート: lib 903 passed（既知 TCC 依存 1 件のみ・隔離再実行で green）、統合テスト 43 ターゲット全 ok、clippy 0、fmt clean、cargo build ok。バッチ内境界は cargo check + clippy + 対象テスト（軽量）で通過。個別 PBI は `.plan/archived/` を参照。未完了 0。
