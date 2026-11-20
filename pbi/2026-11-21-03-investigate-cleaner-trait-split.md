出所: holistic-code-improvement round 10（RICE 3 位）。round 9 台帳送りの再検討トリガー発火（メソッド数 11 → 12、次ラウンド再評価の到来）による investigate PBI。実測アンカーは main @ 892ae1d。

# PBI 2026-11-21-03: Cleaner trait 12 メソッドの capability split 裁定（investigate）

- 種別: investigate（BDD の Then は「裁定が記録される」。テスト戦略の代わりに調査手順を置く）
- 規模: S
- 優先度: round 10 中 3 位（RICE 1.2 = Reach 3 × Impact 0.5 × Confidence 0.8 / Effort 1 SP）
- 依存: なし（2026-11-21-01 と並列可。担当ファイルが src/cleaner.rs のみで排他成立）

## 背景（実測済みの事実。推測で書き足さない）

- `Cleaner` trait は required 3（name / detect / clean、src/cleaner.rs:328-342）+ default 9（is_available :345、risk :351、auto_clean_allowed :357、interactive_gated :368、quits_running_apps :383、advisory :388、per_cleaner_filters :394、sub_targets :399、sub_owned_paths :412）= **12 メソッド、41 impl**（round 9 記録時は 11 メソッド → トリガー「11 を超えたら / 次ラウンドで再評価」が発火）
- round 9 の積み残し記録: 「capability split の要否は 41 impl への波及が大きく、今回未評価」
- 消費者別の整理（実測）:
  - **core**（全経路）: name / detect / clean / is_available
  - **無人選別述語**（unattended.rs の select + matrix）: risk / auto_clean_allowed / interactive_gated / quits_running_apps
  - **scan / warnings**（scanner.rs 表示・main.rs 起動警告）: advisory / per_cleaner_filters
  - **TUI 専用**（interactive.rs の selection mapping）: sub_targets / sub_owned_paths
- 制約: registry は `Box<dyn Cleaner>` の単一トレイトオブジェクトで、capability split は registry 型・呼び出し面・41 impl に波及する。`quits_running_apps` と `interactive_gated` は PBI 87/75 の選別契約の一部で、unattended.rs の 96 セル matrix pin が形状を固定している
- src/AGENTS.md は trait の全列挙を意図的に避け「the real trait has grown default predicates since」と記録している（列挙の陳腐化が既知の保守コスト）

## ユーザーストーリー

As a 保守担当者, I want a recorded verdict on whether the 12-method Cleaner trait should be split by consumer, so that the next method addition lands on a decided structure instead of re-litigating the trade-off every time.

## BDD シナリオ

1. **裁定の記録**: Given 4 消費者群ごとのメソッド整理と 41 impl への波及コストの分析, When 裁定を行う, Then 「split する / しない」の判断と根拠が本 PBI ファイルに記録される（裁定セクション）
2. **判断の運搬**: Given 裁定が「split しない」の場合, When 次に default method が追加されるとき, Then src/cleaner.rs の trait doc（または本 PBI の裁定参照）が「どの消費者群に属するか」の分類枠を提供し、新規メソッドが群をまたいで混在しない

## 調査手順（テスト戦略の代わり）

1. 実測: 12 メソッド × 41 impl のうち、default 実装からオーバーライドしている impl の数をメソッド別に数える（`rg` で各メソッドのオーバーライド箇所を実測）。オーバーライド率が低い群（例: sub_owned_paths は hints のみ）ほど split の利得が薄い
2. 呼び出し面の実測: 各メソッドの呼び出し元を列挙し、消費者 4 群の境界がモジュール境界と一致するかを確認（unattended.rs / scanner.rs / interactive.rs / main.rs 警告）
3. split 選択肢の評価: (a) 現状維持（doc で 4 群を明記）(b) `UnattendedInfo` / `TuiSubTargets` 等の小トレイト split + registry への blanket impl または dyn 合成 (c) 構造体分解。それぞれの波及（41 impl・registry 型 `Box<dyn Cleaner>`・selection matrix pin・E2E）を見積もる
4. 裁定: round 2〜9 の先例（macro テーブル統一・UnattendedPolicy 統一など「1 つの事実 = 1 所有者」規律）と YAGNI の緊張を明示して裁定する。**本 PBI は裁定までで、split の実装はしない**（採用時は次ラウンドの PBI 化対象として台帳に記録）
5. 裁定の運搬: src/cleaner.rs の trait doc に 4 群の分類枠を 4〜8 行で追記する（split 不採用の場合も、この分類枠が次回追加メソッドの判断を速くする）。もしこの doc 追加が不要な裁定内容なら、その理由を記録して省略してよい

## BDD

- BDD1: 裁定（split の要否）と根拠（オーバーライド率実測・波及見積もり）が本 PBI ファイルに記録される（Then: 裁定が記録される）
- BDD2: 裁定が「split しない」の場合、src/cleaner.rs に 4 消費者群の分類枠 doc が実在する（Then: 分類枠が記録される）

## 受け入れ基準

- [ ] メソッド別オーバーライド率の実測表を本ファイルに記録する（12 メソッド × オーバーライド impl 数・呼び出し元モジュール）
- [ ] 3 選択肢（現状維持 / 小トレイト split / 構造体分解）それぞれの波及見積もり（41 impl・registry・matrix pin・E2E）を記録する
- [ ] 裁定と根拠を「裁定」セクションに記録する（次ラウンドでの再評価トリガーも併記する）
- [ ] 裁定が「split しない」場合、src/cleaner.rs の trait doc に 4 消費者群の分類枠を追加する（byte 等価・挙動不変、cargo test / clippy / fmt green）
- [ ] 既存テスト・E2E pin に無修正で green（doc 追加のみの実装変更）
- [ ] src/cleaner.rs 以外の実装変更をしない（本 PBI は裁定まで）

## 見積もり

1 SP（実測 2 点 + 裁定記録 + doc 追記 4〜8 行）

## DoD

- [ ] cargo test が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 裁定セクションが本 PBI ファイルに記録されている
- [ ] PBI アーカイブ（統合側が実施: git mv → `.plan/archived/`、INDEX 追記）

## scope 外（やらないこと）

- split の実装（裁定採用時は次ラウンド PBI 化）
- Cleaner trait のメソッド追加・削除・署名変更（本 PBI は doc と裁定のみ）
- src/AGENTS.md の書き換え（列挙回避方針は維持）
