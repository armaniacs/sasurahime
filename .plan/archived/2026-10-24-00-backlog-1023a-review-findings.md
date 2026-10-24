# 1023a 全体レビューの発見事項バックログ（RICE順位表）

発見元: 1023a 全体レビュー（main..1023a、66コミット、Major 12件 + Minor 約20件）。

## RICE順位表

| PBI | タイトル | Reach(影響する実行経路) | RICE |
|-----|---------|------------------------|------|
| 40 | 削除ブロックリストのギャップ（/Volumes, /Users/Shared 等）+ doc 正確化 | 全削除経路（seam 適用は中央） | 280 |
| 41 | auto の no-policy ガード有効化（[policy] セクション実在時のみ許可） | auto + --yes ユーザー全員（先行 config 持ち） | 240 |
| 43 | PathCleaner/CliCleaner の skip 報告可視化（SkippedEntry + uses_trash 真実化） | PathCleaner 全対象 + fallback | 180 |
| 38 | プラグインターゲットの dry-run/help 処理修正 | プラグイン利用経路 | 150 |
| 39 | [custom] ターゲットの CLI 到達可能性 + 衝突検証 | [custom] 利用経路 + doc 整合 | 120 |
| 45 | trash モードの uchg 取り扱い検証 + trash.rs doc 正確化 | 全 trash モード削除（verify-first） | 100 |
| 42 | colima のリスク分類修正（Risky/Moderate） | --yes/auto 無人経路 | 96 |
| 44 | 4クリーナーが CleanOptions.trash_mode を尊重 | pin 契約（latent） | 64 |
| 46 | テスト衛生（mise ゲート実体化 + 未リセット グローバル + 空弁論） | テスト信頼性 + フレーク源 | 72 |
| 47 | Minor バンドル（scan --json flush、home() 空、BatchSummary skipped、steady tick、resolve_env_dir join、variant pin） | CLI/バッチ経路全般 | 56 |

## バッチ計画（ファイル競合を避ける）

- バッチ1（3並列）: 40+45（generic.rs, trash.rs）/ 44（4クリーナー）/ 41+42（config.rs, main.rs）
- バッチ2（3並列）: 38（main.rs, tests/dry_run.rs）/ 43（generic.rs）/ 46（tests/ + テストモジュール）
- バッチ3（2並列）: 39（main.rs, tests/）/ 47（main.rs, batch.rs, generic.rs, progress.rs）

## 実装外の参照候補（deferred）

- plugin.rs:329-351 の seam バイパス: プロトコル固有（プラグインが自身で削除）。テキスト勧告で既に文書化済み
- cleaner.rs:151 の "trash failed" 文字列分類: 原因でなく文字列で分類する脆弱性。seam 分類の変更を伴うため deferred
- ios_backup 削除経路の未テスト: Terminal guard により interactive-only が設計
- hint auto-clean の履歴未記録: pre-existing
- VerboseProgress の速度表示: pre-existing（main 由来）
- interactive.rs:244-260 のサブターゲット子プロセスのモードフラグ不在: pre-existing、--permanent 下でのみ顕在化
- main.rs:480-491 の死んだ is_empty 分岐（未展開 tilde 既定値）: config 側で到達不能
