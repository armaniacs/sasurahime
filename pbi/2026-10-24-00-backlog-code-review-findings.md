# Backlog: コードレビュー発見事項の PBI 索引（2026-10-24）

`09bba4f..0f7a9e9` のコードレビュー（security / correctness / new-module design の3観点、サブエージェント裏取り済み）で発見した全項目の PBI 化と優先度付け。

## RICE 順位表

| 順位 | PBI | RICE | 根拠 |
|---|---|---|---|
| 1 | `2026-10-24-28-fix-safe-delete-scope-tripwire.md` | 320 | 全削除シンクの defense-in-depth。依存元なしの土台 |
| 2 | `2026-10-24-29-fix-global-dry-run-flag.md` | 300 | 実ならデータ損失クラス。verify-first で安価（pre-existing） |
| 3 | `2026-10-24-30-fix-deletion-regression-gate.md` | 192 | ゲート欠落の是正（uv 実削除の直接カバー） |
| 4 | `2026-10-24-31-fix-permanent-continue-on-fatal.md` | 128 | 多エントリ実行の堅牢性 |
| 5 | `2026-10-24-32-fix-uses-trash-seam-truth.md` | 56 | チェック済み基準と食い違う記録の是正 |
| 6 | `2026-10-24-33-fix-scanner-robustness.md` | 48 | 唯一の detect 経路の panic 分離 |
| 7 | `2026-10-24-34-fix-trash-mode-pin-contract.md` | 24 | latent。32 に依存 |
| 8 | `2026-10-24-35-fix-history-guard-consolidation.md` | 16 | 監査証跡の一貫性 |
| 9 | `2026-10-24-36-fix-hint-cleaner-polish.md` | 8 | latent injection（リスク軽減タイブレーク勝ち） |
| 10 | `2026-10-24-37-fix-test-mode-guard-hygiene.md` | 8 | テストのみ。31 に依存 |

## バッチ計画

- バッチ1（並列、ファイル非重複）: 28（trash.rs + generic.rs）, 29（main.rs）, 30（tests/trash.rs + 新テスト）, 35（batch.rs + history.rs）, 36（hint/cleaner.rs）
- バッチ2（直列、同一ファイル群）: 31 + 37（library_caches / orphan_containers / app_support / mise / library_logs）
- バッチ3（並列）: 32（gradle / generic / project_artifacts / ループサイト + アーカイブ記録訂正）, 33（scanner.rs + interactive.rs）
- バッチ4（32 に依存）: 34（generic.rs CliCleaner + project_artifacts）

## 実装外の参照候補（deferred）

レビューで指摘されたが PBI 化を見送った軽微な項目:

- batch.rs:54-60 同名クリーナーの dedup（到達条件は未検証の [custom] 名衝突、pre-existing config gap）
- batch.rs:31-41 animated 分岐で reporter 引数が dead（内部 API の整理）
- generic.rs:837-859 の env-override scope trust（`PUB_CACHE=$HOME/Documents` が削除対象になる、env 出所は起動シェルの scope-trust トレードオフ）
- hint/apps.rs の path_suffixes 形状の強制（現行 22 件はすべて非空・非トラバーサル、コンパイル時定数）
