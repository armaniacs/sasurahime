# PBI-78: run_batch と単発経路の契約を整列する（cancelled チャネル + status 行の所有者）

出典: /improve-codebase-architecture ラウンド 2 候補 4（深掘り 2026-10-30）

## ユーザーストーリー
sasurahime の保守担当者として、「1 target を 1 回掃除する」の意味論が単発経路とバッチ経路で 1 つの契約に落ちることがほしい、なぜなら run_clean_target と run_batch で CleanCancelled の扱いと [OK] status 行の所有者が割れており、経路ごとに出力が変わる姿はユーザーの信頼を損ない、cancel という run レベルの概念に所有者がいないから。

## 優先度
RICE: Reach 2 × Impact 2 × Confidence 90% / Effort 0.5 人週 = スコア 7.2（本ラウンド 2 位）
- Reach 2: clean all / auto の両バッチ経路が影響を受ける（colima 拒否の誤報告と deep-suppress の [OK] リーク）
- Impact 2: cancel の誤分類は「Error cleaning ...」としてユーザーに見え、status 行の所有者割れ（reporter 種別 vs engine bool）は経路追加のたびに増殖する
- Confidence 90%: [OK] 行への E2E assert が 0 本で所有者の移動は自由、BatchSummary への追加 field は additive で安全

## BDD 受け入れシナリオ
Scenario: colima 拒否は failure 行ではなく cancelled 行として報告され残 target は続行する
  Given TTY で `clean all --include-manual` を実行でき colima が存在する
  When  ユーザーが colima の confirm に拒否する
  Then  colima は `colima: cancelled by user` の cancelled 行として stderr に報告される
  And   残りの target の掃除は続行する

Scenario: auto --deep-suppress の Cleaning 行に [OK] が出ない
  Given `sasurahime auto --deep-suppress` を実行する
  When  バッチ経路が status 行の要否を reporter に問い合わせる
  Then  `Cleaning ... [OK]` 行は出力されない（単発経路と一致）

Scenario: clean uv --deep-suppress の [OK] は今日と同じく出ない
  Given `sasurahime clean uv --deep-suppress` を実行する
  When  単発経路が動く
  Then  `Cleaning ... [OK]` 行は出力されない（現行維持）

Scenario: 特殊 arm のリテラルが Cleaner::name() と一致しなくなったら correspondence テストが落ちる
  Given 特殊 arm の履歴ラベル リテラル
  When  対応する Cleaner::name() だけをリネームする
  Then  correspondence pin の単体テストが失敗する（履歴行の黙って分裂を防止）

Scenario: run_batch は cancelled と failures を別々に収集する
  Given run_batch に Err(CleanCancelled) と Err(その他) が混在する
  When  run_batch が完了する
  Then  BatchSummary.cancelled に cancelled target 名が入り
  And   BatchSummary.failures にその他の failure が入り
  And   2 つは混ざらない

## 受け入れ基準
- [ ] BatchSummary に cancelled: Vec<String> を追加し、run_batch の Err 枝は e.is::<CleanCancelled>() で downcast 分岐する（名前は loop 変数 c.name() から。CleanResult::cancelled() は意図的に nameless のため CleanResult.name からは取れない）
- [ ] report_batch_failures は cancelled 行を中立文言（`{name}: cancelled by user`）で failures の後、stderr に出す
- [ ] 単発の Ok(cancelled)・exit 0 は不変（main.rs:1607-1620 run_clean_target_cancelled_returns_ok_zero が無修正で green）
- [ ] run_batch 非 animated 枝の status 行は reporter.show_spinner() のときだけ出る（auto --deep-suppress の [OK] リーク解消）
- [ ] animated を show_spinner() から導出しない（no-tick-while-prompting 契約の維持、tick が prompt の下に再描画される回帰の防止）
- [ ] SkipConfirmGuard は呼び出し側に残留し、All arm に意図コメント + confirm_suppressed()==false を pin する単体テストを置く
- [ ] 履歴ラベル correspondence pin の単体テスト（特殊 arm のリテラル == 対応 Cleaner::name()、標準 arm の command_name() == cleaner.name()）
- [ ] run_clean_target:790-794 のコメントを「整列済み契約の記述」に書き換える
- [ ] 新規単体テスト 6 本以上（batch 2 + interactive 2 + All arm pin 1 + correspondence pin 1）

## テスト戦略
Outside-In TDD。外側の既存 E2E が pin となり、内側へ向かって新規テストを Red → Green → Refactor で書く。
- E2E（外側、無修正で green が pin）: tests/clean_all.rs / tests/auto_policy.rs / tests/interactive.rs / tests/progress.rs / tests/history.rs。[OK] 行への E2E assert は 0 本なので status 行の所有者移動は自由。
- Integration（内側 1）: batch.rs の run_batch 単体テスト 2 本（run_batch_collects_cancelled_separately_from_failures / run_batch_reports_cancelled_not_failed）を先に Red → Green。
- Unit（内側 2）: interactive.rs report_batch_failures の cancelled 行 2 本、main.rs All arm pin + ラベル correspondence pin、progress.rs の quiet 枝（quiet ヘルパ追加か inline）。
- モック: 外部コマンドは CommandRunner 注入。CleanCancelled の発生源（cleaners/generic.rs）は触らないため、fake reporter + 手製の Err で駆動する。

## 見積もり
0.5 人週（0.4 から 0.75）

## 実装計画

### 問題（深掘り結果）
「1 target を 1 回掃除する」に 2 つの並列エンジン（run_clean_target: src/main.rs:718-804 / run_batch: src/batch.rs:28-79）があり、行単位で同じことを別条件で書いている。意味論のドリフト:
- CleanCancelled は単発では成功（Ok(cancelled)・exit 0）、バッチでは無区分の failure 行。confirm_message を持つのは colima だけ（generic.rs:226-249）なので到達面は実質 1 経路（TTY で clean all --include-manual + colima 存在 + ユーザー拒否）だが、そのとき「Error cleaning colima: cleaning cancelled by user」が出て残 target は続行する。cancel は run レベルの概念なのに所有者がいない。
- deep-suppress の [OK] リーク（新バグ形状）: run_batch の非 animated 枝（progress.rs:190-205 の with_spinner_result_progress）が reporter に関係なく `Cleaning X... [OK]` を出す。単発経路は show_spinner()==false 枝で [OK] を出さない。結果: `sasurahime auto --deep-suppress` は [OK] 行が出るが `sasurahime clean uv --deep-suppress` は出ない。2 エンジンで status 行の所有者が割れている（reporter 種別 vs engine bool）。[OK] 行への E2E assert は 0 本なので移動は自由。
- 履歴ラベルの由来割れは潜在 fork: 特殊 arm のリテラルと Cleaner::name() は今日一致しているが無強制。片側だけリネームすると履歴行が黙って分裂する。対応 pin が存在しない。

### 解（Option B: 意味論整列。完全統合 Option A は次に新 entry point が来た時のみ発火）
1. BatchSummary に cancelled: Vec<String> を追加。run_batch:75 の Err 枝を downcast 分岐（e.is::<CleanCancelled>() → cancelled.push(名前) else failures）。名前は loop 変数 c.name() から（CleanResult::cancelled() は意図的に nameless のため CleanResult.name からは取れない）。report_batch_failures（interactive.rs:169-194）は cancelled 行を中立文言（`{name}: cancelled by user`）で failures の後、stderr に出す。
2. 単発の Ok(cancelled)・exit 0 は維持。理由は記録済み原理と一致: interactive.rs:289-294「TUI の decline は interactive ユーザーの選択で script failure ではない」+ PBI 69 の --yes --permanent abort 非ゼロは無人実行特有の契約。TTY の clean all decline は Ok(0) で中立行が正しい。
3. [OK]/[FAILED] status 行の所有者を reporter 種別に統一: run_batch 非 animated 枝は reporter.show_spinner() のときだけ status 行を出す。注意: animated を show_spinner() から導出してはいけない ― animated は no-tick-while-prompting 契約（progress.rs:44-46, 187-189, pin :554-576）であり clean all は TTY で prompt しうるため、導出すると tick が prompt の下に再描画される回帰。
4. SkipConfirmGuard は呼び出し側維持 + All arm の意図明記: batch.rs:3-4 の doc が既に「skip policies stay at the call sites」と宣言。run_batch 側に移すと clean all --include-manual が colima の fallback 全削除を無確認で実行する安全回帰。All arm（main.rs:1397-1399 付近）に意図コメント + confirm_suppressed()==false を pin する単体テスト。
5. 履歴ラベル correspondence pin: 特殊 arm のリテラル == 対応 Cleaner::name()、標準 arm の command_name() == cleaner.name() を pin する単体テスト（main.rs:1650+ の every_clean_target_variant... の兄弟）。機械的単一ソース化は PBI 82 のテーブル列に畳む。
6. run_clean_target:790-794 の「The single-target path keeps its own flow」コメントを「整列済み契約の記述」に書き換え。

### 実装順
(1) batch.rs cancelled チャネル + テスト 2 本（run_batch_collects_cancelled_separately_from_failures / run_batch_reports_cancelled_not_failed）→ (2) interactive.rs report_batch_failures に cancelled 行 + 単体テスト 2 本 → (3) main.rs All arm コメント + pin テスト + コメント書き換え → (4) main.rs ラベル correspondence pin → (5) [OK] status 行の reporter 種別ゲート化（progress.rs に quiet ヘルパ追加か inline）→ (6) 検証ゲート（DoD）。

### 裁定済み事項（deep-dig の未解決疑問への回答）
- cancel 後の残 target は継続（現行維持、1 target ごとの決定として自然）
- cancelled 行は stderr + `{name}: cancelled by user`（中立性）
- F2 の [OK] 修復は本 PBI に同梱（同一 engine・同一 pin 群）

## pin（壊してはいけない契約）
- batch.rs:200-338 + 340-356 + 358-386: skipped carry、sum+failures、animated 枝の等価性、履歴 guard（hist-a 1 行）、ゼロバイト無記録、post_clean_note 3 本。BatchSummary への追加 field は additive で安全
- main.rs:1607-1620 run_clean_target_cancelled_returns_ok_zero（Ok(cancelled) 契約）、:1622-1648 skip 非パニック、:1650+ variant correspondence
- interactive.rs 単体: :673 ReportShape::Bare が名を省く、:688 無失敗で無音、:700 Total freed 行、:321-328 の PBI-33 記録済み決定（run_batch は収集のみ、callback で signature を分岐させない）は絶対
- E2E: tests/auto_policy.rs（colima never runs unattended + disk.img 生存）、tests/clean_all.rs（needs --include-manual、success + Permanently deleted）、tests/interactive.rs（:467-506 PBI 69 abort 非ゼロ含む）、tests/progress.rs（:55-76 と :123-139 deep-suppress Freed 非表示、:40-47 suppress Freed 表示）
- PBI 73 採用 pin（main.rs:2284-2308 include_str! ソース pin「batch arms ride the shared reporters」）: caches/all/auto arm は report_batch_failures + report_total_freed を保持
- PBI 74 契約: select() が選択の唯一所有者、matrix test（unattended.rs:369-425）
- no-tick-while-prompting: with_spinner_result は steady tick を付けない（progress.rs:554-576 source pin）、with_spinner_result_progress は caller の animated を通す
- 履歴: write_history_entry_if_needed が唯一の guard（batch.rs:100-119）、tests/history.rs:149-186

## scope fence
1. 新 entry point / CLI 変更 / clean --json の新設なし
2. batch 系 flow の process exit code 変更なし（failure は報告のみのまま）
3. Freed 行・Total freed の文言/出力先の変更なし
4. run_clean_target/run_and_exit の run_batch への完全統合（Option A）は後回し ― 発火条件（次の新 entry point）だけ記録
5. 履歴ファイル形式・ラベル値の変更なし（由来の pin のみ）
6. UnattendedPolicy / SkipConfirmGuard の位置 / prompt.rs への接触なし
7. cleaners/ 内部（CleanCancelled 発生源 generic.rs:563 含む）の変更なし
8. pin 済み E2E 文言の書き換えなし
9. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
