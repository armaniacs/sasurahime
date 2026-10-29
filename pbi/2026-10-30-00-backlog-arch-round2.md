# Backlog: アーキテクチャ深掘り（2026-10-30 第三陣）

## 出典
/improve-codebase-architecture ラウンド 2。ラウンド 1（PBI 69-76）で埋めた深い interface（delete_ledger、Cleaner trait 単一メソッド、CleanResult named constructor、PromptPort、UnattendedPolicy、SnapshotMode）の周囲に残った摩擦を、cleaner 層とコアフローの 2 体の探索ウォーカーが 6 候補として特定し、6 体の並列サブエージェントで深掘り（隠れた仮定・未発見リスク・未検討の意思決定）して PBI 化した。
主な深掘り発見: ledger 契約の迂回は 5 サイト（ollama が新規発見、同一イベントに 3 種の挙動）、hint 経路で --dry-run + y が実削除になる（TTY 限定）、caches arm だけが解決済み clean_opts を捨てて --permanent を破る（バグ実証済み）、app_support ⇔ orphan_containers の相互参照 cycle と root list ドリフト（不可視は /Applications/Utilities の 2 階層ネストのみ）、CliCleaner 250 行本体への port 未モデル第 3 セマンティクス、correspondence pin は現行 55 件（マクロは dispatch 系を既に生成済みで残存分散は special 側のみ）。

## RICE 採点と最終順位

| 順位(NN) | 候補 | type | Reach | Impact | Conf | Effort(人週) | スコア | 出典 |
|---|---|---|---|---|---|---|---|---|
| 77 | delete_ledger 契約の seam 外漏れ（5 迂回サイト移行 + 可視性絞り込み + ソースピン） | fix | 3 | 2 | 90% | 0.75 | 7.2 | 候補 2（PBI 70 の未回収残部） |
| 78 | run_batch と単発経路の契約整列（cancelled チャネル + status 行所有者 + guard 意図明記 + ラベル pin） | fix | 2 | 2 | 90% | 0.5 | 7.2 | 候補 4 Option B |
| 79 | installed_apps module 切り出し（cycle 解消 + canonical root list 統合 + README 同期） | feat | 2 | 1.5 | 85% | 0.75 | 3.4 | 候補 6 |
| 80 | hint 経路に dry_run と履歴を届ける（twin 統合 + is_yes_answer 共有 + ledger 移行） | fix | 2 | 2.5 | 80% | 1.25 | 3.2 | 候補 1 Phase A+B |
| 81 | CliCleaner::clean_with_mode の 2 internal seam 分割 + confirm_on_tty + warn_tool_failure | refactor | 2 | 2 | 90% | 1.25 | 2.9 | 候補 5 |
| 82 | CleanTarget special 変種のマクロ表昇格（identity 全生成）+ caches group + xcode 1 関数化 + core_cleaners | refactor | 3 | 2 | 95% | 2.75 | 2.1 | 候補 3 |
| 83 | hint 掃除を hints cleaner として registry 登録（1 cleaner + sub_targets） | feat | 2 | 1.5 | 70% | 1.5 | 1.4 | 候補 1 Phase C |

## 同点の決め手
- 77/78（7.2 同点）: Reach の順（77 が 6 cleaner 面、78 が報告系 2 面）。77 を先にすると 80/81 のパターンが確立する

## 依存による順序調整
- 77 → 81: generic.rs + cli_fallback.rs を共有。双方向の重複のため直列必須。77 を先にすると fallback 削除が delete_entries 1-call 行になり、81 が移すコードが最小化される
- 77 → 80: hint が最後の safe_delete 直呼びのため、77 は pub(crate) で止め、80 の着地後に private 化
- 78 → 80: main.rs 領域（78 は comments/tests、80 は 1082/1117）。78 先行
- 82 → 83: define_cleaners! を共有。82 の special 行形式の上に 83 が 1 行追加する
- 79 は独立（app_support / orphan_containers / README を触る候補は他にない）
- 82 は最大物（2.75 人週）のため後半に配置し、小さい fix（77/78/80）を先に着地させる

## バッチ計画（実装順）
- A（並列、非重複）: 77（cleaners + trash 可視性）+ 78（batch/interactive/main comments+tests）+ 79（app_support/orphan/lib.rs/README）
- B（単独）: 80（hint + main.rs run_hint_flow）
- C（単独）: 81（generic.rs + cli_fallback.rs + prompt.rs + command.rs）
- D（単独）: 82（main.rs 本体 + cleaner.rs + docs）最大物
- E（単独）: 83（define_cleaners! + hints cleaner）
- 各バッチ後にゲート検証、全着地後にフルスイート 1 回、アーカイブ（.plan/archived/ + .plan/00-INDEX.md 行追加）、push
- CHANGELOG は各 PBI で触らず、マージ後に本体で一括記載（ラウンド 1 の pbi-75 マージ競合の回避）

## 判断が必要な項目
- なし（深掘りで caches arm バグの修正方針、CliCleaner 分割の confirm gate 位置、canonical root list の 5 root 採用、Option A 完全統合の発火条件まで確定済み）
