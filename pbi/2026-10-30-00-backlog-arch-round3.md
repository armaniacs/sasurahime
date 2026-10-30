# バックログ - アーキテクチャ改善ラウンド 3（2026-10-30）

ラウンド 2（PBI 77-83）の scope fence に「意図的に先送りした」と記録された持ち越し候補 4 件を PBI 化した。Option A（単発 = run_batch 1 要素の完全統合）は発火条件「次に新 entry point を新設する時」まで待つため、本ラウンドの対象外。

## RICE 索引

| 順位 | PBI | スコア | Reach / Impact / Confidence / Effort | 内容 |
|------|-----|--------|--------------------------------------|------|
| 1 | [PBI-84](2026-10-30-84-fix-precommit-test-hygiene.md) | 11.4 | 3 / 2 / 95% / 0.5 人週 | pre_commit の `detect_returns_not_found_when_dir_missing` だけ TEST_LOCK + EnvGuard がなく並列実行で兄弟テストの tempdir キャッシュを拾い得る flake 対策 |
| 2 | [PBI-85](2026-10-30-85-fix-uv-symlink-filter.md) | 9.6 | 2 / 3 / 80% / 0.5 人週 | uv の GAP-006 ガードが symlink のみ除外し simple-vN の通常ファイルが削除候補（最高世代を装ったファイルが keep-highest を壊す）になる潜在バグ修正 + gradle keep_highest の借用 |
| 3 | [PBI-86](2026-10-30-86-fix-filter-support-plugin-warning.md) | 5.3 | 2 / 1 / 80% / 0.3 人週 | filter_support に plugin 名を載せて `[cleaner.<plugin>]` の unknown cleaner name 誤警告をなくす |
| 4 | [PBI-87](2026-10-30-87-fix-unattended-hints-exclusion.md) | 3.5 | 2 / 2 / 70% / 0.8 人週 | 無人実行（auto / clean all --include-manual）が quit 系クリーナー（hints）を選ばないようにする選別軸の追加 |

## バッチ計画

| バッチ | PBI | 実行 | 触るファイル |
|--------|-----|------|--------------|
| 1 | 84 / 85 / 86 / 87 | 並列（worktree 4 体） | 84: pre_commit.rs tests のみ / 85: uv.rs + gradle.rs + tests/uv.rs / 86: main.rs（filter_support 領域 + tests モジュール）+ tests/config_warnings.rs + tests/plugin.rs / 87: cleaner.rs + hint/cleaner.rs + unattended.rs + main.rs（all 腕 rendering）+ tests/auto_policy.rs |
| - | マージ | 直列 | 84 - 85 - 86 - 87 の順で `git merge --no-edit`。86 と 87 は main.rs を共有するが領域が非重複（86: 715-844 + 1255-1257 + tests モジュール / 87: all 腕 rendering）。コンフリクトが出たら両者を直列に格下げ |

依存関係: なし（4 件とも互いに独立）。CHANGELOG は全 PBI が未接触とし、マージ後に本体側で一括記載する。

## 次ラウンドの持ち越し候補（本ラウンドでは扱わない）

- pre_commit.rs の clean_* 5 本（:174、:193、:212、:227、:246）も TEST_LOCK + EnvGuard なし（PBI-84 の writer が実コード確認で発見。うち clean_fallback_deletes_dir は潜伏 flake 候補）。huggingface.rs の clean_dry_run_uses_cli_when_tool_available（:216-233）も guard なし
- retention モジュール抽出（keep_highest を gradle.rs から共通モジュールへ移動。PBI-85 の design-it-twice で案 B として記録済み）
- Option A（単発 = run_batch 1 要素の完全統合）: 発火条件は「次に新 entry point を新設する時」
- detect/sub_targets の二重 walk（xcode 先例に従い受け入れ済み）
- KNOWN_ENTRIES の config 化、hint 経路と app_support/library_caches の機能重複解消、log vs eprintln の統一（uv/brew 変種）
