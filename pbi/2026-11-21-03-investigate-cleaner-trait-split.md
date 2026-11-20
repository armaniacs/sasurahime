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

- [x] メソッド別オーバーライド率の実測表を本ファイルに記録する（12 メソッド × オーバーライド impl 数・呼び出し元モジュール）
- [x] 3 選択肢（現状維持 / 小トレイト split / 構造体分解）それぞれの波及見積もり（41 impl・registry・matrix pin・E2E）を記録する
- [x] 裁定と根拠を「裁定」セクションに記録する（次ラウンドでの再評価トリガーも併記する）
- [x] 裁定が「split しない」場合、src/cleaner.rs の trait doc に 4 消費者群の分類枠を追加する（byte 等価・挙動不変、cargo test / clippy / fmt green）
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

## 裁定

裁定: **split しない**。現状維持 + 4 消費者群分類 doc（選択肢 (a)）を採用する。src/cleaner.rs の trait doc（`pub trait Cleaner` 直上、:327-335）に分類枠 8 行を追記済み。split 実装は行わないため次ラウンド PBI 化も不要。

### 実測 1: メソッド別オーバーライド率

実装ブロック総数の実測は **43**（本番 29 + テスト専用スタブ 14）。背景節の「41 impl」は `impl Cleaner for` の素 grep 値で、うち 2 件はテスト内の文字列リテラル（cleaners/generic.rs:1956、cleaners/orphan_containers.rs:1055）、batch.rs の 4 実装は完全修飾形 `impl crate::cleaner::Cleaner for`（テストスタブ）で素 grep から漏れる。本番 impl のみで裁定に影響はない。

| メソッド | 群 | 本番 29 impl 中のオーバーライド | 本番率 | スタブ込み総数 |
|---|---|---|---|---|
| name（required） | core | 29 | 100% | 43 |
| detect（required） | core | 29 | 100% | 43 |
| clean（required） | core | 29 | 100% | 43 |
| is_available | core | 13 | 45% | 15 |
| risk | 無人選別述語 | 25 | 86% | 29 |
| auto_clean_allowed | 無人選別述語 | 6 | 21% | 6 |
| interactive_gated | 無人選別述語 | 3 | 10% | 4 |
| quits_running_apps | 無人選別述語 | 2 | 7% | 4 |
| advisory | scan / warnings | 5 | 17% | 5 |
| per_cleaner_filters | scan / warnings | 3 | 10% | 3 |
| sub_targets | TUI 専用 | 3 | 10% | 4 |
| sub_owned_paths | TUI 専用 | 1 | 3% | 2 |

### 実測 2: 呼び出し面（本番コードのみ。テスト内呼び出しと ManualOnly / HuggingFace / PreCommit の内部委譲は除く）

| メソッド | 本番呼び出し元 |
|---|---|
| name | 全モジュール（registry 名寄せ・notice・filter_support 等、設計上の全経路） |
| detect | scanner.rs detect_all（:193）のみ |
| clean | batch.rs 実行 seam（:112-122）+ main.rs CleanTarget dispatch |
| is_available | scanner.rs detect_all の単一パス（:176）のみ |
| risk | unattended.rs select（:153/:180/:209）、scanner.rs scan_preamble（:225） |
| auto_clean_allowed | unattended.rs select（:164/:183/:205）、main.rs all_cleaners の ManualOnly ラップ（:742）、interactive.rs gate_skipped_names（:171） |
| interactive_gated | interactive.rs gate_skipped_names（:172、--yes 通知）のみ |
| quits_running_apps | unattended.rs select（:161/:181）のみ |
| advisory | scanner.rs run_scan の Notes（:303）のみ |
| per_cleaner_filters | main.rs filter_support → 起動警告（:807/:811）のみ |
| sub_targets | interactive.rs 選択マッピング（:34）のみ |
| sub_owned_paths | interactive.rs 選択マッピング（:39）のみ |

背景節の整理に対する実測上の補正 2 点:

- interactive_gated の本番消費者は unattended.rs の select ではなく interactive.rs の gate_skipped_names（--yes の skipped-targets 通知）。select と 96 セル matrix（3 variant × 4 risk × include_manual × 2 status × quits = 96）が消費するのは risk / auto_clean_allowed / quits_running_apps で、Yes は interactive_gated を参照しない。分類枠では TTY 系述語として同じ群に置く（消費者の性質は無人選別と共通）。
- auto_clean_allowed は select 以外に registry 合成時の ManualOnly ラップ（main.rs:742）と --yes 通知（interactive.rs:171）の 2 消費者を持ち、無人選別述語群で唯一モジュール境界をまたぐ。risk も scanner の表示面で 2 モジュール目を持つ。それ以外の 8 述語は 1 モジュールに収まる。

### 3 選択肢の波及見積もり

| 選択肢 | 43 impl への波及 | registry 型 | matrix pin / E2E | 得られるもの |
|---|---|---|---|---|
| (a) 現状維持 + 4 群分類 doc | 0（doc 追記のみ） | `Box<dyn Cleaner>` 維持 | 無修正 | 次回メソッド追加時の分類判断が doc 1 読で完結する |
| (b) 小トレイト split（UnattendedInfo / TuiSubTargets 等への group 化） | 43 実装すべてに sub-trait の impl 追加（supertrait 合成で同一 impl ブロックにまとめても境界の型注釈は増える）。ManualOnly は委譲 6 メソッドの再配置 | supertrait 合成なら `Box<dyn Cleaner>` 維持だが利得は型注釈のみ。並列 `Vec<Box<dyn SubTrait>>` 化なら detect_all の index alignment と select Yes の「同一 slice の detect 結果」という型注釈（unattended.rs:191-195 の debug_assert）が崩れる | 96 セル matrix の fixture 型変更。E2E の語彙は不変でも回帰面が広がる | 「未実装群を型で強制」だが、強制先の default はオーバーライド率 3〜21% であり、29 本番 impl の大多数は default だけを持つ sub-trait 名の列挙追加になる |
| (c) 構造体分解 | 29 本番 impl を struct 合成へ再構成。macro テーブル・plugin discovery・custom cleaners・ManualOnly を全面書き換え | registry の型と構築経路の再設計 | E2E 含め全 pin 再検証 | なし。8 述語の消費者はすでに 1-2 モジュールに収まり、trait に共有状態が無いため分解すべき対象が存在しない |

### 根拠（裁定の 4 軸）

- (i) オーバーライド率: 9 つの default メソッドのうち 8 個が本番率 45% 以下（6 個は 21% 以下）。split の利得が最も薄い群（TUI 3%/10%、scan/warnings 10%/17%）こそ sub-trait 化の自然な候補であり、型で未実装を強制しても大多数の impl に新情報がない。required 3 + risk はどの split でも共有されるため移動対象にならない。
- (ii) registry: `Box<dyn Cleaner>` 単一オブジェクトが ManualOnly の合成時ラップ、detect_all の index alignment、select Yes の同一 slice 型注釈の 3 点を支える。split はこの 3 点のどれかを壊すか、supertrait 合成で形だけ残すかの二択で、後者は 43 実装の書き換えに見合う利得を持たない。
- (iii) 「1 つの事実 = 1 所有者」規律と YAGNI: 実測では 8 つの default 述語の本番消費者は 1-2 モジュールに収まっており、所有者規律はすでに成立している。sub-trait は所有者の追加であって統合ではない。第 2 の消費者家族は現時点で不在であり、split の動機になる未来の要件も観測されていない。
- (iv) 次回メソッド追加: 実際の保守コストは src/AGENTS.md が記録する「列挙の陳腐化」であり、trait doc の 4 群分類枠が追加時の判断（どの群・どの消費者モジュール・どの default）を doc 1 読に圧縮する。分類枠に収まらないメソッドが現れた時点で split 再審査が発火する。

### 次ラウンド再評価トリガー

- default 述語の本番消費者に第 3 モジュールが生えたとき（特に cleaner 側が advisory 等を参照し始めたとき）
- OwnedPaths の doc に記録済みの「第 2 の cleaner family が owned-path 注釈を要する」時点（型 split の議論として同時に審議）
- registry の `Box<dyn Cleaner>` 形状が別の理由で変わるとき（その変更に乗じて ride-along で再評価）
- 追加後 2 ラウンド経ってもオーバーライド 0 の default メソッドが出たとき（split ではなく削除審議）

### 検証（本 PBI の実施スコープ）

- cargo fmt --check / cargo clippy --tests -- -D warnings / cargo test --lib cleaner を green で確認済み。doc コメントのみの追加で src の挙動は不変。フルスイート・E2E pin は統合側の最終ゲートで実施する（受け入れ基準の残りチェックボックスと DoD の cargo test 系は統合側が [x] にする）。
