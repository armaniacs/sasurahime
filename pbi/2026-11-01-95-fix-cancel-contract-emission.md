出典: アーキテクチャ探索 round 6 計上/出力エンジン グループ（2026-11-01、.plan/whywhy-round6-counting.md、読み取り専用分析・コード変更なし）の収束解 (i) A2（cancel 契約の spinner 漏れ・Strong）。同判定 (i) の最小形 4 点が本 PBI の実装計画

# PBI-95: ユーザーキャンセルがバッチエンジンで [FAILED] と印字される矛盾を閉じる

## ユーザーストーリー
スクリプトや cron で `sasurahime clean all --include-manual` を走らせる開発者として、colima の confirm を拒否した target が [FAILED] と印字されず静かに終わることがほしい、なぜなら cleaner 側は「colima: cancelled by user」と語るのに直前の「Cleaning colima... [FAILED]」が並ぶと、失敗したのか止めたのか読み手が分からず、行数と語彙で grep / sed するスクリプトが矛盾 2 行を拾ってしまうから。

## 優先度
RICE: Reach 1 × Impact 3 × Confidence 90% / Effort 0.1 人週 = スコア 27（whywhy round 6 全候補の最上位）

- Reach 1: 実在経路 `clean all --include-manual` × colima の confirm 拒否（main.rs:1709-1716 の run_batch(..., animated=false, with_notes=true)）。confirm を通らない利用者（--suppress、非 TTY、colima 未検出）には現象が出ないため 1。
- Impact 3: ユーザー可視の自己矛盾。1 つの事実（拒否）を emission 層は [FAILED]、caller は「cancelled by user」と 2 語彙で語る。--include-manual は「確認対象も含める」の意味なので、拒否の結果が [FAILED] になるのは意味の逆転。
- Confidence 90%: 修正は helper への分類追加（trait 1 + impl 1 + helper 枝 2 + doc 表 1 行 + test 2）のみ。矛盾 2 行はこの 1 行の欠落で説明でき、印字済み行を後から消す処理は不要（helper は spinner を閉じた直後、印字の前に Err を分類できる）。
- 前提: PBI 78（.plan/archived/2026-10-30-78-fix-batch-cancel-contract.md）の着地済み。cancel 契約は clean エンジン側（main.rs の downcast / batch.rs の Err 枝）には届いたが、progress.rs の共通 emission には届いていない。

## 背景（なぜなぜ分析 round 6 の発見）
1. status 行（[OK]/[FAILED]）の emission は 3 重所有で、cancel 分類はその 1 つにしかない。
   - **run_clean_target inline**（src/main.rs:893-923）: SharedSpinner を自前で起動し、[OK]/[FAILED]/無音（cancel 分類）を自前で分岐 ― **分類あり**（main.rs:906-910 と :920、いずれも `e.is::<CleanCancelled>()`）。main.rs:907-908 に「User cancelled、clean shutdown、no [FAILED]、no Freed line」の pin コメント。
   - **`with_spinner_result`**（src/progress.rs:190-205）: run_batch 非 `animated` 枝 + TUI sub-target ― **分類なし**（is_err() → [FAILED]）。
   - **`with_spinner_result_progress`**（src/progress.rs:168-182）: run_batch animated 枝 ― **分類なし**（同型の is_ok/else）。
2. PBI 78 の cancel 契約（src/batch.rs:22-24「a cancel is a run-level decision, not an error」）は clean エンジン側には届いた: run_batch の Err(CleanCancelled) downcast 枝（src/batch.rs:156-157）が summary.cancelled に回し、caller が中立行「cancelled by user」を出す。だが progress.rs の 2 helper は共通 emission として `E: std::fmt::Display` 境界で一般化されており、Err の中身（分類）を見ない。
3. 矛盾 2 行の到達経路（実在・コードで実測済み）: `clean all --include-manual`（main.rs:1709-1716、animated=false × with_notes=true）→ CLI reporter（show_spinner() = true）→ run_batch 非 `animated` 枝（src/batch.rs:107-110）→ `with_spinner_result`。colima は ManualOnly 包装（src/cleaners/manual_only.rs:26-33 の透過転送）で generic.rs の confirm（src/cleaners/generic.rs:527-533、CleanPrompt::confirm_on_tty）に届き、拒否時に「colima: cancelled」を印字して（generic.rs:529）Err(CleanCancelled) を返す（generic.rs:531）。戻った Err を helper が is_err() で [FAILED] にするため、「Cleaning colima... [FAILED]」+「colima: cancelled by user」の矛盾 2 行になる。
4. バッチ経路の cancel 時 status 行（現状の [FAILED] / 修正後の無音）を pin する test は 0 本。単発の無音は main.rs:907 のコメント pin のみ。test のない経路は契約の捜索対象にならず、PBI 78 以降も欠落が残った。
5. 逆に、helper への分類追加だけで閉じる。1 つの CleanCancelled が status 行（行家族 1）と cancel 行（行家族 2）の両方の材料になるため、分類の真実を 1 箇所（CleanClassified impl）に置けば、run_batch の downcast 枝 → summary.cancelled → caller の中立行だけが cancel を語る 1 本道になり、矛盾 2 行は消える。

### 実コードの確認（main @ 53f0b88、着手前確認を含む）
- with_spinner_result（src/progress.rs:190-205）: 境界は `E: std::fmt::Display`（:190）。ProgressBar::new_spinner → finish_and_clear → is_ok/else で [OK]/[FAILED] を印字（:194-203）。Err の中身は見ない。
- with_spinner_result_progress（src/progress.rs:168-182）: 同型。SharedSpinner::start → finish → is_ok/else（:173-180）。境界も `E: std::fmt::Display`（:168）。
- run_clean_target inline（src/main.rs:893-923）: show_spinner() 枝で SharedSpinner 自前起動（:898）。`Err(e) if e.is::<CleanCancelled>()` 枝で無音 shutdown（:906-910）、非 spinner 枝も同分類（:920）。
- run_batch（src/batch.rs:84-116）: animated 枝 → with_spinner_result_progress（:104）、非 `animated` × show_spinner() 枝 → with_spinner_result（:110）、quiet 枝 → 印字なし（:111-115）。Err 枝は `e.is::<crate::cleaner::CleanCancelled>()` で summary.cancelled に回し（:156-157）、残り target は続行。
- CleanCancelled の在処: src/cleaner.rs:233（pub struct CleanCancelled）。分類 trait と 1 impl は同じファイルに置ける。
- helper の呼び出し筋（境界変更の影響面）: batch.rs:104（E = anyhow::Error）、batch.rs:110（同）、interactive.rs:233 の clean_sub_target（E = anyhow::Error）。src/progress.rs 単体 2 本（:542-545、:548-552）だけが E = String / &str で、境界変更時に型の差し替えが必要（落とし穴参照）。
- 着手前確認（フェーズ0）: CancelClassified のような分類 trait は src/ 配下に存在しない（grep 実施済み、未実装を確認）。単発の無音契約は実装済みのため、本 PBI は「共通 emission への分類運搬」のみ。.plan/archived/ に重複 PBI なし（PBI 78 は cancel 契約の導入、PBI 90 は note gate の統一で隣接 scope）。

## BDD 受け入れシナリオ

```gherkin
Scenario: clean all --include-manual で colima の confirm を拒否すると矛盾 2 行が出ない
  Given TTY で `sasurahime clean all --include-manual` を実行できる
  And  colima が confirm 対象として選ばれている
  When colima の confirm プロンプトに拒否を答える
  Then 「Cleaning colima... [FAILED]」は出力されない
  And  cancel は caller の中立行「colima: cancelled by user」だけが語る
  And  残りの target の掃除は続行され exit 0 で終わる

Scenario: 単発の colima 拒否は今日どおり無音（現行維持）
  Given TTY で `sasurahime clean colima` を実行できる
  And  colima の confirm プロンプトが表示される
  When 拒否を答える
  Then 「Cleaning colima... [FAILED]」は今日どおり出力されない
  And  main.rs:906-910 の pin コメントと単発の無音契約は無修正のまま

Scenario: 本当の失敗は今日どおり [FAILED] を印字する（分類が失敗を飲まない）
  Given バッチ経路で掃除対象の 1 つが CleanCancelled 以外の Err で失敗する
  When run_batch が非 `animated` 枝 × show_spinner() でその cleaner を走らせる
  Then 「Cleaning <target>... [FAILED]」は今日どおり出力される
  And  失敗は summary.failures に入り caller が報告する
```

## 受け入れ基準
- [ ] src/cleaner.rs に分類 trait（例: CancelClassified、is_user_cancelled）と anyhow::Error への 1 impl（downcast_ref::<CleanCancelled>() の 1 箇所）が追加される（CleanCancelled の隣、:233 付近）
- [ ] src/progress.rs の 2 helper（with_spinner_result :190、with_spinner_result_progress :168）の境界が `E: std::fmt::Display + CancelClassified` になり、cancel 枝では status トークンを印字しない（行の掃除は既存の finish / finish_and_clear に従い、Err はそのまま返す）
- [ ] [CANCELLED] トークンは新設しない（単発の無音契約と batch 行家族 2 の中立行に整合する「印字なし」のみ）
- [ ] src/batch.rs の所有者表 doc（:57-64、1 行目 status line の Gate 列）に「CleanCancelled はどの経路でも status トークンを出さない、分類の owner は CancelClassified impl」を 1 行追記する（英語 doc comment）
- [ ] test 2 本追加: (a) helper の cancel 無音（source pin）、(b) run_batch 非 `animated` 枝 × Err(CleanCancelled) で summary.cancelled が 1 件（未 test だったバッチ cancel status 行を閉じる）
- [ ] 単発の無音契約 pin（main.rs:906-910 付近の pin コメント）と単発 colima 拒否の挙動は無修正で green
- [ ] cargo test --no-fail-fast（既知 TCC 4 失敗を除く）、cargo clippy --tests -- -D warnings 0、cargo fmt --check

## テスト戦略
Outside-In TDD。ただし矛盾 2 行の E2E 再現は colima confirm の TTY 前提（src/prompt.rs:89-98 の confirm_on_tty は is_interactive() で gate）により CI では不可能なため、unit と source pin で担保する（PBI 90 の note gate と同じ判断）。helper の eprintln も in-process の観測 seam を持たない。

- Red（source pin）: src/progress.rs tests モジュールに helper の cancel 無音 1 本を追加。with_spinner_result_does_not_enable_a_steady_tick（src/progress.rs:555-576）と同じ include_str!("progress.rs") 走査スタイルで、2 helper の本体に cancel 分類（is_user_cancelled）があり、[FAILED] 印字が分類の後に来ること（cancel では status トークンに到達しないこと）を pin する。現行コードでは cancel 枝が存在しないため先に Red になる。この Red が「helper に分類がなく、Err は無条件に [FAILED] になる」= 矛盾 2 行の発生条件を実証する。
- Green: src/cleaner.rs の CancelClassified impl + progress.rs 2 helper の境界と cancel 枝で閉じる。
- 単体（挙動 pin）: src/batch.rs tests モジュールに run_batch 非 `animated` 枝 × Err(CleanCancelled) 1 本。clean() が Err(anyhow::Error::from(CleanCancelled)) を返す fake cleaner を用意し、SuppressReporter（show_spinner() = true、src/progress.rs:329-338、pin は :392 suppress_reporter_shows_spinner）で run_batch(..., animated=false, with_notes=false) を通す。assert は summary.cancelled == 1（cleaner 名付き）+ summary.failures == 0 + 残り target の続行。この経路の status 行は helper が唯一の印字者なので [FAILED] 不在は source pin (a) が担保し、summary 側は挙動 pin が担保する。現行でも cancelled 1 件は green（未 test 経路の pin）で、helper 修正後も green を保つ（回帰の検知）。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針どおり）。fixture は tempdir HOME。

## 見積もり
0.1 人週

内訳: CancelClassified trait + 1 impl（0.02）+ progress.rs 2 helper の境界と cancel 枝 + 既存単体 2 本の E 型差し替え（0.02）+ batch.rs 所有者表 1 行（0.01）+ test 2 本（Red/Green 込み）（0.05）。

## 実装計画

### 実装順
1. Red: src/progress.rs tests モジュールに helper cancel 無音の source pin 1 本を追加し、現行（cancel 分類なし）で落ちることを確認する。
2. Green: src/cleaner.rs に CancelClassified trait と anyhow::Error への 1 impl を追加（CleanCancelled の隣、:233 付近）。progress.rs の 2 helper（:168、:190）の境界を `E: std::fmt::Display + CancelClassified` に変え、cancel 枝では status トークンを印字しない（finish / finish_and_clear は現行どおり、Err はそのまま返す）。[CANCELLED] トークンは新設しない。
3. 既存単体の型差し替え: src/progress.rs の with_spinner_result_returns_ok / returns_error（:542-552）の E を CancelClassified を満たす型（例: anyhow::Error）に変える（assert の挙動は維持）。
4. doc comment: src/batch.rs の所有者表（:57-64、1 行目 status line の Gate 列）に 1 行追記する。言語は既存の英語に合わせる。
5. 挙動 pin: src/batch.rs tests モジュールに run_batch 非 `animated` 枝 × Err(CleanCancelled) の単体 1 本を追加する。
6. 検証ゲート（DoD の 3 コマンド）。

### 対象ファイル
src/cleaner.rs（trait + impl）、src/progress.rs（2 helper の境界と cancel 枝 + source pin 1 本 + 既存 2 本の E 型差し替え）、src/batch.rs（doc 表 1 行 + 挙動 pin 1 本）。

### 期待する Red → Green
helper cancel 無音の source pin が先に Red、実装後に全 Green。既存 pin と既存 E2E は無修正で green が pin（回帰の検知）。単発の無音契約（main.rs inline 分類）と単発 colima 拒否の挙動は無変更。

### batch.rs 所有者表に追記する 1 行（Gate 列）
1 行目（status line）の Gate 列への追記（英語）:

「a `CleanCancelled` Err prints no status token on either path; the classification is owned by the `CancelClassified` impl beside `CleanCancelled` (cleaner.rs), not by the emission」

2 行目（cancel line）は文言変更不要 ― 「single silent / batch neutral」は今日は helper の [FAILED] が偽にしているが、修正後に真になる。検証で 2 行目が全経路で真であることを確認する。

### design-it-twice
- Option A（採用）: 分類の真実を 1 箇所（CancelClassified impl、CleanCancelled の隣）に置き、helper の境界に足す。cancel では status トークンを印字しない。単発の無音契約（main.rs:906-910）と batch 行家族 2（caller の中立行）に整合し、新しい行家族を作らないので Option A 段 3 の発火条件に触れない。
- Option B（却下）: helper の境界を anyhow::Error 固定にする。分類の契約が emission の型に縛られ、TUI sub-target など他の呼び出し筋の E 型変化を emission が吸収できなくなる。分類の owner は CleanCancelled の在処（cleaner.rs）であるべき。
- Option C（却下）: run_batch の Err 枝側で直す。spinner は cleaner.clean() を包み、Err が戻った時点で [FAILED] は印字済み。後から消せないため修正位置として無効（半年後の保守担当者が選びたくなる誘惑地点）。
- Option D（却下）: [CANCELLED] トークンの新設。新しい行家族の追加 = Option A 段 3 の発火条件に当たり、単発の無音契約とも不整合。scope fence で禁止。

### 落とし穴
- src/progress.rs の既存単体 2 本（with_spinner_result_returns_ok :542-545 / returns_error :548-552）は E = String / &str で、境界変更でコンパイルが落ちる。挙動 assert を維持したまま E を anyhow::Error に差し替える（tests/progress.rs の統合テストは別物で触れない）。
- helper の cancel 枝は「行の掃除の後、印字の前」に置く。finish / finish_and_clear を cancel 枝で飛ばすと spinner の行が残る（掃除は現行どおり）。
- anyhow::Error への downcast は `downcast_ref::<CleanCancelled>()` で行う。helper 側に `e.is::<CleanCancelled>()` を直接書くと分類の重複が生える（分類の owner は CancelClassified impl の 1 箇所）。
- run_batch 挙動 pin の fake cleaner は CleanResult::cancelled() ではなく Err を返すこと。CleanResult::cancelled() は Ok 枝の材料で、Err(CleanCancelled) の downcast 枝（batch.rs:156）を通さないと pin にならない。
- helper 境界の変更で TUI sub-target（interactive.rs:233）にも cancel 無音が波及する。CleanByName は anyhow::Result を返すため契約に整合するが、実装時に TUI 側の挙動変更がないか 1 回確認する。

## pin（壊してはいけない契約）
- 単発の無音契約: run_clean_target の inline 分類（main.rs:906-910/920）と pin コメント（main.rs:907-908「no [FAILED]」）は無修正。helper への統一は Option A 段 3 の領土
- 単発 colima 拒否の挙動: generic.rs:527-533 の confirm（confirm_on_tty）、拒否時の「colima: cancelled」印字（generic.rs:529）と Err(CleanCancelled)（generic.rs:531）、fallback delete の非 TTY 拒否（PBI 69 pin）は無修正
- PBI 78 の契約整列: run_batch の Err(CleanCancelled) downcast 枝（batch.rs:156-157、summary.cancelled チャネル、caller の中立行、残り target 続行）は無修正
- batch.rs:469-502 run_batch_status_line_is_owned_by_the_reporter_kind（source pin）: `let result = if animated {` から `match result {` までの走査範囲は無修正。helper 境界の変更と doc 表への追記はこの範囲に触れない
- 行家族 3〜6（note / freed / skip / exit）の owner と gate: with_notes 条件（PBI 90 で統一済み、batch.rs:130）、freed 行、skip 行、exit code（PBI 78 scope fence 2）は無修正
- progress.rs の既存 pin: with_spinner_result_does_not_enable_a_steady_tick（:555-576）、with_spinner_result_progress_keeps_the_animated_choice（:579-595）をはじめとする no-tick-while-prompting / steady-tick 関連の pin は無修正で green
- reporter 4 種（CleaningProgress / VerboseProgress / DeepSuppressReporter / SuppressReporter）の署名と show_spinner() の返り値、build_reporter の分岐は無修正
- 3 チャネルの arm 実数: animated=false × with_notes=true（clean all main.rs:1709-1716、auto main.rs:1366-1373）、clean caches main.rs:1006、TUI（interactive 側）は無修正（形態 enum への置換は Option A 段 3）
- tests/progress.rs 全既存、tests/clean_all.rs 全既存、tests/auto_policy.rs、tests/interactive.rs、tests/history.rs が無修正で green
- pbi/README.md 設計方針: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用

## scope fence（絶対禁止）
1. [CANCELLED] トークンの新設や新しい行家族の追加はしない（cancel は「印字なし」のみ。行家族 1〜6 の構成は維持）
2. 行家族の追加 / gate の移動 / gate 条件の変更（Option A 段 3 の発火条件、batch.rs:80-83）はしない。**本修正は emission 層の分類修正であり、段 3 の発火条件（行家族の追加 / gate の移動 / gate 条件の変更）には当たらない**
3. 単発の無音契約（main.rs inline 分類、run_clean_target / run_and_exit）の変更はしない
4. reporter 4 種（CleaningProgress / VerboseProgress / DeepSuppressReporter / SuppressReporter）の追加・削除・署名変更、show_spinner() の返り値変更、build_reporter の分岐変更はしない
5. SKIP_CONFIRM まわり（confirm_suppressed、ConfirmPrompt、generic.rs の confirm）の変更はしない
6. run_batch のシグネチャ（animated / with_notes の bool 引数）と BatchSummary の型、BatchFailure は変更しない
7. CHANGELOG.md は触らない（統合側で一括記載）
8. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] BDD 受け入れシナリオが自動テストとして実装され、パスする（E2E 再現不可能分は unit + source pin で担保）
- [ ] pin の列挙がすべて無修正で green していることを確認する（単発の無音契約 pin、単発 colima 拒否の挙動を含む）

## 備考
- 実装は worktree で subagent-driven。対象が 3 ファイル（cleaner.rs / progress.rs / batch.rs）に跨るが pin との接触範囲が狭く、単独タスク 1 本で着手できる。
- 並行で src/scanner.rs（PBI 96/98）、src/cleaners/library_logs.rs・orphan_containers.rs の別領域（PBI 97/99）、prompt.rs（PBI 99）を触る別 PBI が進行中。src/cleaner.rs / src/progress.rs / src/batch.rs は本 PBI が排他的 owner。merge 時の競合は main 側で解消する。
- 本修正は emission 層の分類修正であり、Option A 段 3 の発火条件には当たらない（scope fence 2 にも記載）。A2 修正済みの helper（cancel 分類済み emission）は、将来段 3 が発火した時の report_single_target / JSON 経路の土台になる。
