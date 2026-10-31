出典: 持ち越し候補深掘り（round 5、2026-10-31）の収束した解「順 1 ― 準備 PBI（段 1+2、R1）」（.plan/whywhy-round5-option-a.md、読み取り専用分析・コード変更なし）。前段 3 段階のうち段 1（failures を BatchFailure に）と段 2（BatchSummary に results 追加）が本 PBI の該当範囲。段 3（run_clean_target 置換）は発火条件まで待つ

# PBI-92: BatchSummary に failures と results の運搬型を載せる（段 3 統合の準備）

## ユーザーストーリー
sasurahime の保守担当者として、バッチの failures と results が段 3（単発/バッチ統合）に必要な情報を失わずに運搬されることがほしい、なぜなら failures が (String, String) で anyhow チェーン（cause/context/型）を捨てており、BatchSummary は「成功かつ exit_code()==1」の単発結果を復元できる材料を持っていないため、段 3 の逆写像（BatchSummary → 単発契約）で Err 再構築バグと「成功かつ exit 1」の消失という攻撃面が構造的に生まれるから。

## 優先度
RICE: Reach 2 × Impact 2 × Confidence 95% / Effort 0.2 人週 = スコア 19（≒ 20）

- Reach 2: 全バッチ経路（clean all / auto / TUI、run_batch 呼び出し 5 筋）が failures の運搬と report_batch_failures の表示を共有する。直接の読み手は今日 report_batch_failures の display のみだが、段 3（全 entry point の統合）と将来のエラー報告改善（cause チェーン表示）の 2 つの経路がこの型に乗る。
- Impact 2: 今日のユーザー実害はゼロ（R1: 本番 clean 経路に `.context()` / `.with_context()` は 0 箇所、全エラーが単段 `anyhow!()` のため e.to_string() と本体表示の差は実質ゼロ）。価値は「準備」で、段 3 の逆写像 6 破れ目のうち 2 つ（Err 再構築、results 欠落で「成功かつ exit 1」の消失）を構造的に消す予防投資。
- Confidence 95%: 型変更は additive で機械的（本体 3 箇所 + テスト 6 箇所 + 構造体リテラル 6 箇所 + doc 2 点）。source pin（batch.rs:426-459）の assert 文字列が failures の型に依存しないことを実測で確認済み。
- 「バグ修正」ではなく「準備（予防投資）」として扱う。今日の実害ゼロ（R1）であるため、PBI の型は refactor で、出力・exit code・履歴・pin はすべて不変。

## 背景（なぜなぜ分析 round 5 の発見）
1. failures が String でエラー本体を運ばない（R1 残差分）。batch.rs:10 の `failures: Vec<(String, String)>` は batch.rs:122 の `e.to_string()` で蓄積し、anyhow チェーン（cause/context/型）を捨てる。ただし今日のユーザー実害はゼロ: 本番 clean 経路に `.context()` / `.with_context()` は 0 箇所（grep 実測）で、全エラーが単段 `anyhow!()` メッセージのため、e.to_string() と本体表示の差は実質ゼロ。価値は「段 3 の Err 再構築の前提」+「将来 cause/context を持つエラーが入ったときの情報保持」にある。
2. 段 3（run_clean_target を run_batch(1 要素) + report_single_target レイヤーに置換）の逆写像には 6 つの破れ目がある: (1) 段 3 の Err 再構築、(2) results 欠落で「成功かつ exit 1」の消失、(3) skip 行の Named 流用（Bare を明示）、(4) SkipConfirmGuard hoist、(5) animated 導出の report_single_target 側再現、(6) note の 2 重 gate。うち (1) と (2) は段 1+2 が構造的に消す。BatchSummary は成功を total_freed に圧縮するため、「成功したが exit_code()==1」（0 freed かつ skipped 非空、cleaner.rs:164-170）の単発結果が summary から消える。
3. round 4 の「段 1 は pin に触れる」前提が実測で崩れた: source pin（batch.rs:426-459 run_batch_status_line_is_owned_by_the_reporter_kind）の assert 文字列（`let result = if animated {`、`match result {`、`} else if reporter.show_spinner() {`、`animated = ` の不在）は failures の型に依存しないため、段 1 は pin 無傷で green を保てる。切り戻し単位の分離根拠（「段 1 は pin に触れる/段 2 は additive」）が失われたため、段 1+2 は 1 本の準備 PBI に統合できる。

### 実コードの確認（main @ 56e96cf、着手前確認を含む）
- BatchSummary（src/batch.rs:6-18）: failures は batch.rs:10 の `Vec<(String, String)>`。results field は未実装（grep 実測、フェーズ0 確認）。run_batch 本体の summary 初期化は batch.rs:61-66。
- Err 枝の運搬（src/batch.rs:112-124）: push は batch.rs:122 の `summary.failures.push((c.name().to_string(), e.to_string()))`。cancelled 分岐（batch.rs:119-120）と skipped carry（batch.rs:88-93）は本 PBI では触らない。
- Ok 枝（src/batch.rs:86-111）: results への push の追加先。r.skipped.len() / r.uses_trash を batch.rs:108-109 で使うため、push は write_history_entry_if_needed（batch.rs:104-110）の後に置く。
- report_batch_failures（src/interactive.rs:178-212）: destructure は interactive.rs:183 の `for (name, e) in &summary.failures`、error 出力は interactive.rs:185-186。`f.error.to_string()` への置き換えで出力は byte 等価。failures の本番消費者はこの 1 箇所のみ（grep 実測）。
- テスト 6 箇所: batch.rs 3 本（run_batch_sums_only_successful_cleans_and_collects_failures :311-339 の :332-338、run_batch_collects_cancelled_separately_from_failures :341-378 の :368-373、run_batch_animated_path_accumulates_like_the_plain_path :405-424 の :421-423 のタプルアクセス `.0` / `.1`）+ interactive.rs 3 本（:713 / :729 / :748 の `failures: vec![("brew".to_string(), "boom".to_string())]`）。failures のアクセスが無修正の 3 箇所: batch.rs:397-401（failures.is_empty()）、interactive.rs:766 / :779（failures: vec![]）。
- source pin（src/batch.rs:426-459 run_batch_status_line_is_owned_by_the_reporter_kind）: 走査範囲 `let result = if animated {` から `match result {` まで。assert 文字列は failures の型に依存しない（実測）。pin は無傷で green 維持。
- 行家族所有者表（src/batch.rs:36-52）と Option A 発火条件（src/batch.rs:49-52）は PBI 90 で追加済み。
- correspondence pin 2 本（src/main.rs:2017-2103 special_arm_labels_match_the_cleaner_names / src/main.rs:2105-2123 standard_arm_labels_match_the_cleaner_names）、ollama pin 2 本（tests/ollama.rs:99 / :145 の "Error cleaning ollama" 不在 assert）。
- run_batch 呼び出し 5 筋: src/main.rs:1006（clean caches）/ :1366（auto）/ :1709（clean all）、src/interactive.rs:331（run_auto）/ :436（TUI whole-target）。results field の追加は additive でこの 5 筋は無視して安全。
- CleanResult（src/cleaner.rs:154-161）: name は pub field（cleaner.rs:155-156、doc は "pub field for downstream inspection" としか言わない）で、results[0].name == c.name() は pin されていない非不変条件。exit_code()（cleaner.rs:164-170）は 0 freed かつ skipped 非空 → 1。
- 着手前確認（フェーズ0）: BatchFailure は src/ に未定義（grep 実測 0 件）、BatchSummary に results field なし。pbi/ の未着手 PBI 3 本に重複なし。.plan/archived/ の PBI 78（cancel 契約と status 行の所有者）/ PBI 90（note gate の統一）は隣接 scope で重複なし。

## BDD 受け入れシナリオ

```gherkin
Scenario: バッチ失敗の 1 行表示は今日と byte 等価で出続ける
  Given FailingCleaner（anyhow!("boom")）を 1 target 含むバッチを run_batch で走らせた
  When  report_batch_failures が Named 形状で report を出す
  Then  "Error cleaning bad: boom" の 1 行が今日と byte 等価で出る
  And   Bare 形状（"Error: boom"）、cancelled 行、skip 行も今日と byte 等価である

Scenario: failures はエラー本体を anyhow::Error で運ぶ
  Given run_batch の Err 枝が anyhow::Error を受け取る
  When  1 件の失敗が BatchSummary.failures に積まれる
  Then  要素は BatchFailure{name, error} で error は anyhow::Error のまま運搬される
  And   消費者は report_batch_failures の f.error.to_string() のみで、今日の出力は変わらない

Scenario: results は CleanResult を運ぶが、今日の出力と exit code は不変
  Given Ok 枝の cleaner を 1 target 含むバッチを run_batch で走らせた
  When  run_batch が完了する
  Then  summary.results に CleanResult が loop 順で積まれている
  And   stdout、stderr、履歴行、exit code は今日と byte 等価である

Scenario: source pin は failures の型変更に依存せず green を保つ
  Given batch.rs の source pin が `let result = if animated {` から `match result {` を走査する
  When  failures を Vec<BatchFailure> に置換する
  Then  pin の assert 文字列は failures の型に依存しないため無修正で green する
  And   animated は show_spinner から導出されず、status 行の gate も変わらない
```

## 受け入れ基準
- [ ] src/batch.rs に `pub struct BatchFailure { pub name: String, pub error: anyhow::Error }` が追加され、BatchSummary.failures が `Vec<BatchFailure>` になる（batch.rs:10）。Err 枝の push（batch.rs:122）は `BatchFailure { name: c.name().to_string(), error: e }` になる（R1 解消: エラー本体を anyhow::Error で運ぶ）
- [ ] BatchSummary に `results: Vec<CleanResult>` が追加される（additive）。Ok 枝（batch.rs:86-111）の末尾で `summary.results.push(r)` される。run_batch 呼び出し 5 筋（main.rs:1006 / 1366 / 1709、interactive.rs:331 / 436）は無修正で green する
- [ ] report_batch_failures（interactive.rs:183）は `for f in &summary.failures` + `f.name` / `f.error.to_string()` になり、出力は byte 等価（既存 3 本の期待値文字列を無修正のまま green することが等価の証拠）
- [ ] テスト 6 箇所が機械的に書き換えられる: batch.rs 3 本の `.0` / `.1` → `.name` / `.error`（assert 期待値 "bad" / "bad-anim" / contains("boom") は不変）+ interactive.rs 3 本の構造体リテラル → BatchFailure リテラル（error は anyhow!("boom") のまま）
- [ ] run_batch の doc comment に 2 点が追記される: (1) failures の anyhow::Error と results は段 3（report_single_target）用の運搬型である旨（display の読み手は今日 report_batch_failures のみ。「準備」である旨を明記し、読み手ゼロの型になり削除 PR の標的になるのを防ぐ）、(2) results[0].name == c.name() は pin されていない非不変条件で、段 3 の report_single_target 側が results[0].name を表示・履歴に使ってはいけない旨（段 3 の落とし穴）
- [ ] cancelled チャネル（batch.rs:119-120）、skipped carry（batch.rs:88-93）、履歴 guard（batch.rs:158-168）、post_clean_note 本体（batch.rs:134-147）は無修正
- [ ] 出力不変: 新規 E2E は置かない（既存 E2E 全 suite が pin）

## テスト戦略
Outside-In TDD。ただし本 PBI は出力不変の additive 型変更のため、Red は「既存テスト 6 箇所のコンパイル失敗」として現れる（Rust では Red = compile error。E2E Red は置かない）。

- Red: failures の型変更で batch.rs 3 本（タプルアクセス `.0` / `.1`）と interactive.rs 3 本（構造体リテラル）がコンパイル落ちする。assert 期待値は 1 行も変えない。
- Green: 機械的書き換えだけで green。report_batch_failures の既存 3 本（Named / Bare / cancelled 後置き）は期待値文字列を無修正のまま green する = 出力 byte 等価の pin。interactive.rs の失敗 0 本（:766 / :779）と batch.rs の is_empty（:397-401）も無修正で green。
- E2E（追加なし）: 出力不変のため新規 E2E は置かない。既存 E2E（tests/ 全 suite）が無修正で green する回帰 pin。
- --verbose は failures の運搬形式に無関係: report_batch_failures は reporter を受け取らず out（stderr）に書くだけで、VerboseProgress が替えるのは reporter 種別のみ。--verbose の下でも failures の運搬形式は変わらない（whywhy round 5 立場 (c) 4-6 段の実測）。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針どおり）。本 PBI は外部コマンド経路に触れないため、既存 FailingCleaner（anyhow!("boom")）の seam で駆動する。

## 見積もり
0.2 人週

内訳: BatchFailure 追加 + failures 置換（0.05）+ results 追加（0.03）+ report_batch_failures 書き換えとテスト 6 箇所（0.04）+ doc comment 2 点（0.04）+ 検証ゲートと byte 等価確認（0.04）。

## 実装計画

### 実装順
1. src/batch.rs: `BatchFailure { name, error: anyhow::Error }` を追加し failures を `Vec<BatchFailure>` に置換（batch.rs:10）。Err 枝の push（batch.rs:122）を `BatchFailure { name: c.name().to_string(), error: e }` に。cancelled / skipped は触らない（R1 解消: エラー本体を anyhow::Error で運ぶ）
2. 同じ Ok 枝の末尾に `results: Vec<CleanResult>` を追加（additive、field は batch.rs:6-18、push 1 行は batch.rs:86-111 内）。push は write_history_entry_if_needed の後に置く（r.skipped.len() / r.uses_trash を batch.rs:108-109 で使うため、前へ出すと move 後使用でコンパイル落ちする）
3. src/interactive.rs: report_batch_failures の destructure（interactive.rs:183）を `for f in &summary.failures` + `f.name` / `f.error.to_string()` に。**出力は byte 等価**
4. テスト 6 箇所の機械的書き換え: batch.rs 3 本（:332-338 / :368-373 / :421-423）+ interactive.rs 3 本（:713 / :729 / :748）。failures のアクセスが無修正の 2 箇所（batch.rs:397-401、interactive.rs:766 / :779）は failures としては触らない（ただし results 追加分の構造体リテラル対応は下記の落とし穴参照）
5. run_batch doc comment に 2 点追記: (1) failures の anyhow::Error と results は段 3 の report_single_target 用の運搬型である旨（準備である旨を明記。display の読み手は今日 report_batch_failures のみ）、(2) results[0].name == c.name() は pin されていない非不変条件で、report_single_target 側が results[0].name を表示・履歴に使ってはいけない旨。doc の言語は既存の英語に合わせる
6. 検証ゲート（DoD のコマンド + 出力 byte 等価の確認）

### 対象ファイル
src/batch.rs（BatchFailure + results + doc comment 2 点 + テスト 3 本）、src/interactive.rs（report_batch_failures + テスト 3 本 + 構造体リテラル 5 本の results 追加）。

### 期待する Red → Green
型変更で既存テスト 6 箇所がコンパイル落ち（Red）→ 機械的書き換え後に全 Green。assert 期待値と既存 pin は無修正で green が pin（出力 byte 等価と回帰の検知）。

### design-it-twice
- Option A（採用）: 段 1+2 を 1 本の準備 PBI に統合。round 5 の実測で段 1 は pin 無傷（source pin の assert 文字列は型非依存）のため、round 4 の「段 1 は pin に触れる/段 2 は additive」による切り戻し単位の分離は根拠を失った。両者とも出力 byte 等価で、切り戻し単位は「段 3 と分離」で十分。
- Option B（却下）: 段 1 と段 2 を別 PBI に分離。段 2 単独着地は「読み手ゼロの field」の生存期間が伸び、削除 PR の標的になる。doc 1 行の理由説明も 2 重化する。
- Option C（却下）: 段 3 を本 PBI に同梱。発火条件未成立（実測）+ 出力再配置 + E2E pin 調査 5 suite + 所有者表単発列の更新が残り、攻撃面 4 箇所（Bare 明示、guard hoist、animated 導出、note 2 重 gate）が 1 PR に混入する。whywhy round 5 判定 1（段 3 は発火条件まで待つ）に反する。

### 落とし穴
- results への push は Ok 枝の末尾（write_history_entry_if_needed の後）に置く。r.skipped.len() / r.uses_trash を batch.rs:108-109 で使うため、push を前へ出すと move 後使用でコンパイル落ちする。
- BatchSummary への field 追加は構造体リテラルを exhaustive にする: 初期化 batch.rs:61-66 + interactive.rs 5 本（:711-716 / :727-732 / :746-751 / :764-769 / :777-782）のすべてに `results: ...` の 1 行が必要。interactive.rs の failures: vec![] 2 本（:764-769 / :777-782）は failures のアクセスは無修正だが、results: vec![] の追加は必要（「テスト 6 箇所」は failures の型変更分で、results 追加分は構造体リテラル 6 箇所に及ぶ）。
- interactive.rs の 3 本のリテラルは error を anyhow!("boom") のままにする（assert 期待値 "Error cleaning brew: boom" を変えない = byte 等価の pin）。
- batch.rs の 3 本は `summary.failures[0].0` → `summary.failures[0].name`、`.1` → `.error` に置き換えるが、期待値 "bad" / "bad-anim" と contains("boom") は変えない。
- report_batch_failures は cancelled 行と skip 行を failures の後に続けて出す: destructure を `for f in ...` に変えても行順と文言は変えない。

## pin（壊してはいけない契約）
- batch.rs:426-459 run_batch_status_line_is_owned_by_the_reporter_kind（source pin）: 走査範囲 `let result = if animated {` から `match result {` までと assert 文字列 4 つ（`let result = if animated {`、`match result {`、`} else if reporter.show_spinner() {`、`animated = ` の不在）は無修正。failures の push（batch.rs:122）は match の Err 枝で pin 範囲内にあるが、assert 文字列は failures の型に依存しないため pin は無傷で green 維持（実測）
- correspondence pin 2 本: main.rs:2017-2103 special_arm_labels_match_the_cleaner_names、main.rs:2105-2123 standard_arm_labels_match_the_cleaner_names。label 契約は本 PBI では触らない
- ollama pin 2 本: tests/ollama.rs:99 / :145（"Error cleaning ollama" 不在 assert）。単発のエラー報告は print しない契約 ― 本 PBI は report_batch_failures の出力を変えないため無修正で green
- report_batch_failures の出力形（`Error cleaning {name}: {e}` / `Error: {e}` / `{name}: cancelled by user` / `cancelled by user` / skip 行）と行順は byte 等価（interactive.rs:183-212 の文言・順序を無修正のまま消費者だけ書き換える）
- BatchSummary.cancelled チャネル + downcast 分岐（batch.rs:119-120）、skipped carry（batch.rs:88-93）、履歴 guard の単一所有者 write_history_entry_if_needed（batch.rs:158-168、zero-byte 無記録を含む）、post_clean_note とその単体テスト 3 本（batch.rs:529-556）は無修正
- 単発経路: run_clean_target / run_and_exit の出力、Ok(cancelled) 無音契約、exit_code() による exit 1 は無修正
- exit code 契約: バッチは常時 exit 0（PBI 78 scope fence 2、所有者表 6 行目）、単発は exit_code()。本 PBI はどちらも触らない
- E2E: tests/ 全既存 suite が無修正で green
- pbi/README.md 設計方針: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用

## scope fence（絶対禁止）
1. 段 3（run_clean_target の run_batch(1 要素) + report_single_target への置換）への着手はしない（発火条件 batch.rs:49-52 の条件、または明示的な実装指示まで待つ）
2. report_single_target（新設ヘルパ）への接触はしない（実装・命名・doc を含む。run_clean_target / run_and_exit も無修正）
3. PBI 78 の 4 guard（SkipConfirmGuard hoist、animated 導出、履歴 guard、label）への接触はしない
4. report_batch_failures の出力形変更はしない（byte 等価を維持。文言・行順・出力先を含む。書き換えるのは destructure の消費者だけ）
5. results[0].name を表示・履歴に使う実装はしない（pin されていない非不変条件。results は段 3 用の運搬のみ）
6. cancelled / skipped チャネル、履歴 guard、post_clean_note 本体、CleanResult / CleanCancelled の定義（cleaner.rs）の変更はしない
7. CHANGELOG.md は触らない（マージ後に本体で一括記載）
8. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 既存 pin が無修正で green していることを確認する（batch.rs:426-459 source pin、main.rs:2017-2103 / 2105-2123 correspondence pin 2 本、tests/ollama.rs:99 / :145 ollama pin 2 本）
- [ ] report_batch_failures の出力が byte 等価であることを確認する（既存 3 本の期待値文字列を無修正のまま green することが等価の証拠）
- [ ] run_batch doc comment に「段 3 用の運搬型」と「results[0].name は非不変条件」の 2 点が書かれている

## 備考
- 実装は worktree で subagent-driven。対象が src/batch.rs + src/interactive.rs で pin との接触範囲が狭いため、単独タスク 1 本で着手できる。
- 段 3 は発火条件（batch.rs:49-52 の「エンジンの意味論に触る変更」の条件、または明示的な実装指示）まで待つ。段 3 PBI は本 PBI に依存し、failures の anyhow::Error と results の CleanResult が揃うため Err 再構築と「成功かつ exit 1」の復元はゼロコストになる。
- 「読み手ゼロ」対策: doc comment の 2 点（段 3 用の運搬型である旨）が理由の一次記録。これがないと半年後に「使われていない型/field」の削除 PR の標的になる（whywhy round 5 立場 (b) 14-17 段）。
- RICE 換算 ≒ 20（2 × 2 × 0.95 / 0.2 = 19）。着手可能な下限帯の準備 PBI。
- 段 3 以外の R 残差分（R2/R3/R6/R7）は本 PBI の対象外: R2 は意図的契約（README 1 行で緩和可）、R3/R6 は概念差（所有者表 batch.rs:36-52 に文書化済み）、R7 は本 PBI で実体が満たされ残差は PBI 78 契約として解かない。
