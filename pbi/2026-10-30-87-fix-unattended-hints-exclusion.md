出典: 前ラウンド（PBI 83 観察）の持ち越し候補「無人実行からの hints 除外」（深掘り 2026-10-30）

# PBI-87: 無人実行（auto / clean all）から quit 系クリーナーの hints を除外する

## ユーザーストーリー
スクリプトや cron で `sasurahime auto` を走らせる開発者として、無人実行が作業中のアプリ（VS Code、Slack など KNOWN_ENTRIES の quit_app 設定済みアプリ）を勝手に終了させないことがほしい、なぜなら `[policy] include_manual = true` や `clean all --include-manual` を設定したままバックグラウンドで走らせると、hints の clean() が osascript で作業中アプリを終了させて再起動し、保存前の作業を失うから。

## 優先度
RICE: Reach 2 × Impact 2 × Confidence 70% / Effort 0.8 人週 = スコア 3.5（本ラウンド 4 位 / 4）

- Reach 2: registry-driven 無人実行の 2 経路（`sasurahime auto` + `[policy] include_manual`、`clean all --include-manual`）の利用者。`--yes` は scan-driven で hints を常に除外済みのため対象外。
- Impact 2: 作業中アプリの意図しない終了。quit は osascript 経由で TTY の有無を問わず発火し、確認を一切求めない。保存前の作業を失う実害があり、無人実行の信頼を損なう。
- Confidence 70%: select() は純粋な決定で 48 セル matrix が pin 済みのため分類変更の影響は局所的。ただし clean all 腕の skipped 行の文言が現行の "needs --include-manual" から別れるため、E2E への影響を検証してから Green にする。
- 前提: PBI 83（hints cleaner 登録）の着地後。KnownAppHintsCleaner が registry に載ったことで、hints が registry-driven 選別の対象になったことが本 PBI の発端。

## 発見の詳細（深掘り結果）

**実コードの確認**（main @ f3366a4）:

- KnownAppHintsCleaner（src/hint/cleaner.rs:226-228）は `risk() = Moderate`、`interactive_gated() = false`（:232-234）。`auto_clean_allowed()` はトレイト既定（src/cleaner.rs:324-326）で `risk() == Safe` から導出されるため false。
- `detect()`（src/hint/cleaner.rs:236-246）は残骸があれば `Pruneable(total)`、なければ `NotFound`。
- `clean()`（src/hint/cleaner.rs:264-318）は hint ごとに `auto_clean_hint`（:111-164）を呼び、running かつ quit_app 設定済みなら `quit_app_and_wait`（:15-59）が osascript `quit app "..."` を送り pgrep で最大 10 秒ポーリングする。osascript は TTY の有無を問わず発火し確認を求めない。削除後 running なら `relaunch_app`（:74-95）が `open -a` で再起動する。
- `UnattendedPolicy::Auto.select()`（src/unattended.rs:138-149）は risk のみを見る registry-driven 分類: Moderate は `include_manual = true` で run に入る。`UnattendedPolicy::All`（:155-161）は `--include-manual` で全 non-Safe を解放する。したがって hints はこの 2 経路で run に選ばれ、clean() が quit → 削除 → relaunch を無人実行中に実行する。
- `--yes`（`UnattendedPolicy::Yes`、src/unattended.rs:176-185）は scan-driven で Pruneable かつ `auto_clean_allowed()` のみ run。hints は Moderate なので bytes > 0 なら `NeedsExplicitSelection` で常に除外済み。--yes には opt-in フラグ自体がなく、どの flag でも解放されない。
- colima（src/cleaners/generic.rs の colima_prune）は `Risk::Risky`（`.with_risk(Risk::Risky)`）のため auto では `NeverUnattended` で既に保護済み（tests/auto_policy.rs:259）。hints は Moderate なのでこの保護の外にいる。
- src/hint/cleaner.rs:223-225 の doc コメントは「never runs unattended and needs an explicit selection」と主張しているが、auto + include_manual と clean all --include-manual では実挙動がこの記述に反する（文書化された契約と実挙動の割れ）。
- tests/unattended.rs は現存しない。unattended 選別の pin は src/unattended.rs の tests モジュール（selection_matrix_pins_the_three_policies、3 × 4 × 2 × 2 = 48 セル）と tests/auto_policy.rs / tests/clean_all.rs の E2E。

**なぜなぜ分析**:

1. 問題: バックグラウンドで走らせた `sasurahime auto` が作業中アプリを終了させる。
2. なぜ: clean() が running な hint に `quit_app_and_wait` を呼ぶから（osascript は確認を求めない）。
3. なぜ clean() が選ばれた: `UnattendedPolicy::Auto.select()` が hints を「Moderate + auto_clean_allowed なし」として include_manual = true で run に分類するから。`clean all` の All も同様（`--include-manual` が全 non-Safe を解放）。
4. なぜ select() は include_manual で解放するのか: include_manual は「ユーザーが Moderate を含めることを選んだ」という opt-in として設計されたが、選別は実行時の TTY を考慮しない。config を書いた時点と無人実行の時点の間に時間差があり、opt-in は「無人実行でも構わない」という意味を運べない。
5. なぜ opt-in に無人実行の意味が無いまま実装されたのか: PBI 74 で select() は「risk だけを見る純粋な決定（I/O なし、TTY 事実なし）」と意図的に設計された。しかし quit 系クリーナーは「TTY がなくても外部（GUI アプリ）に被害が出る」という risk（破壊性）とは別の軸であり、risk 分類だけでは表現されない。
6. 根本原因: 「無人実行で安全か」の判断が risk のみから導出され、「clean() がユーザーの作業環境（GUI アプリの quit/relaunch）に干渉するか」という軸が選別に無い。hints の clean() は必ずこの軸で Unsafe。

`--yes` と `auto` / `all` の差: `--yes` は scan-driven で、選別対象が `detect()` の結果に限定され、opt-in フラグが存在しないため hints は構造的に除外される。一方 `auto --include-manual` と `clean all --include-manual` は registry-driven で、risk と opt-in の 2 軸だけで選別するため、quit/relaunch を伴う target が opt-in の意味論の裏で選ばれる。穴は registry-driven の 2 経路だけにある。

**解（design-it-twice の決定）**: unattended 選別が quit 系クリーナーをどう扱うかの設計判断として、「clean() が GUI アプリを quit しうる target は、opt-in があっても無人実行で選ばない」を採用する。既存の SkipReason 語彙（NeverUnattended / NeedsOptIn / NeedsExplicitSelection）の中で最小の変更で済む形:

- `NeverUnattended(risk)` の語義「no opt-in can ever unlock it in this flow」を quit 系クリーナーに拡張する。SkipReason は 3 variant のまま、UnattendedPolicy 型も変更しない。
- quit 系の判定は Cleaner trait に additive な述語（案: `quits_running_apps() -> bool`、既定 false）を足し、KnownAppHintsCleaner のみ true を返す。述語の名前は実装時に確定してよい（Negotiable）。
- Auto 腕: risk が Safe なら現行どおり run。Moderate は include_manual があっても述語 true なら `SkipReason::NeverUnattended(risk)` に分類する（述語 false なら現行どおり NeedsOptIn / run）。Risky / Manual は現行どおり NeverUnattended。
- All 腕: risk が Safe なら現行どおり run。non-Safe は述語 true なら `--include-manual` の有無にかかわらず `SkipReason::NeverUnattended(risk)` に分類する（述語 false なら現行どおり）。
- Yes 腕: 変更なし（auto_clean_allowed = false で既に除外される。述語は参照しない）。
- Safe かつ述語 true の target は今日存在しない。現行分類（Safe は run）を維持し、将来生えたときは別 PBI で判断する。

## BDD 受け入れシナリオ

Scenario: TTY なしの auto は include_manual があっても hints を選ばない
  Given TTY なしで `sasurahime auto` を実行できる
  And  ~/.config/sasurahime/config.toml に `[policy] include_manual = true` が書かれている
  And  KNOWN_ENTRIES の quit_app 設定済み候補にしきい値以上の残骸が存在する
  When auto を走らせる
  Then hints は skipped に分類され "never runs unattended" の警告付きで除外される
  And  osascript は呼び出されず、残骸の実ファイルは 1 つも変わらない

Scenario: clean all --include-manual も hints を選ばない
  Given TTY なしで `sasurahime clean all --include-manual` を実行できる
  And  KNOWN_ENTRIES の quit_app 設定済み候補にしきい値以上の残骸が存在する
  When clean all を走らせる
  Then hints は skipped に分類され "never runs unattended" の警告付きで除外される
  And  osascript は呼び出されず、実ファイルは 1 つも変わらない

Scenario: TTY ありの対話 offer は従来どおりプロンプトを出す
  Given TTY で `sasurahime scan` を実行する
  And  KNOWN_ENTRIES の候補に残骸が存在する
  When scan の hint プロンプトが表示される
  Then 対話 offer は従来どおり `Quit ... and clear cache? [y/N]` を出す
  And  `sasurahime clean hints` も従来どおり quit → 削除 → relaunch を実行する

Scenario: --yes は scan-driven 除外の現状維持
  Given TTY なしで `sasurahime --yes` を実行する
  And  KNOWN_ENTRIES の候補にしきい値以上の残骸が存在する
  When --yes を走らせる
  Then hints は今日と同じく NeedsExplicitSelection で除外される（現状維持）
  And  他の Moderate ターゲットの分類は変わらない

## 受け入れ基準
- [ ] Cleaner trait に additive な述語（案: `quits_running_apps() -> bool`、既定 false）が追加され、KnownAppHintsCleaner のみ true を返す（述語の既定値を pin するトレイト単体テスト 1 本）
- [ ] `UnattendedPolicy::Auto.select()` が述語 true かつ non-Safe の target を include_manual の有無にかかわらず `SkipReason::NeverUnattended(risk)` に分類する
- [ ] `UnattendedPolicy::All.select()` が述語 true かつ non-Safe の target を `--include-manual` の有無にかかわらず `SkipReason::NeverUnattended(risk)` に分類する
- [ ] `UnattendedPolicy::Yes.select()` は無修正で green（述語を参照しない）
- [ ] main.rs の all 腕の skipped rendering に `SkipReason::NeverUnattended` 分岐が追加され "never runs unattended" を出す（auto 腕と同一文言）。既存 `NeedsOptIn` 行の "needs --include-manual" は現行維持
- [ ] src/unattended.rs のモジュール doc と src/hint/cleaner.rs:223-225 の doc コメントが、quit 除外を含む実挙動と一致する記述に更新される
- [ ] TTY なしの auto + `[policy] include_manual = true` で hints が削除されない（E2E、実ファイル生存 + skipped 行の文言）
- [ ] TTY なしの `clean all --include-manual` で hints が削除されない（E2E、同様）
- [ ] `--yes` の scan-driven 除外は現状維持（hints は `NeedsExplicitSelection` のまま）
- [ ] 対話経路は不変: tests/hints.rs / tests/interactive.rs / tests/dry_run.rs が無修正で green
- [ ] 既存 pin が壊れていない: 48 セル matrix、tests/auto_policy.rs 既存、tests/clean_all.rs 既存、auto 腕の rendering
- [ ] 新規単体テスト 3 本以上（matrix 述語セル + トレイト既定 pin + All 腕 rendering pin は実装形に応じて）

## テスト戦略
Outside-In TDD。E2E で Red を確認してから、内側へ向かって Red → Green → Refactor を繰り返す。

- E2E: tests/auto_policy.rs に `auto_never_runs_hints_even_with_include_manual`（write_policy ヘルパで `[policy] include_manual = true`、KNOWN_ENTRIES 一致の残骸を tempdir HOME に作り、実ファイル生存と "hints (Moderate): never runs unattended" 行で検証）。tests/clean_all.rs に `clean_all_include_manual_never_runs_hints`（同様、実ファイル生存で検証）。osascript 不呼び出しは binary spawn では観測できないため、E2E は実ファイル生存 + skipped 行で担保する。
- エラー系 E2E（対話経路は無修正で green が pin）: tests/hints.rs（clean_hints_deletes_hint_debris_and_writes_one_history_row 等、対話 quit 経路）、tests/dry_run.rs（hint の dry-run pin）、tests/interactive.rs:108（非 TTY は hint に到達せず exit 1）。
- 統合: src/unattended.rs の tests モジュールの matrix 拡張（実質は unit だが select() が直接の契約面）。述語軸（quits: false / true）を足して 3 × 4 × 2 × 2 × 2 = 96 セルに広げるか、hints 専用セルを 48 セルの外に足すかは実装時に design-it-twice で決める（推奨は軸追加: 単一の expected() 期待表が保たれ、quits = false 側が今日の 48 セルと等価になる）。期待値: Auto は include_manual = true でも述語 true かつ non-Safe は `NeverUnattended(Moderate)`、All も同様、Yes は述語に関係なく現行どおり `NeedsExplicitSelection`。既存 48 セル相当（quits = false 側）は期待値無修正で green が pin。
- 単体: 述語の既定値 pin（src/cleaner.rs、trait 実装すべてで false）、KnownAppHintsCleaner の述語 true pin（hint 側）。tests/unattended.rs は現存しないが、新設する場合は unit matrix と重複しない範囲（arm rendering の対応など）に限る。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針どおり）。select() は純粋な決定のため clean() が panic する StubCleaner で駆動し、「選ばれない」ことを構造的に担保する。

## 見積もり
0.8 人週（0.5 から 1.0）

内訳: 述語追加 + select 分岐 + matrix 拡張（0.4 から 0.6）+ E2E 2 本 + all 腕 rendering 分岐（0.1 から 0.2）+ doc 更新と検証ゲート（0.1 から 0.2）。

## 実装計画

### 実装順
1. 単体 Red: src/unattended.rs tests に述語セルを追加（StubCleaner に述語 field、expected() 拡張）。実装前に落ちることを確認する。
2. E2E Red: tests/auto_policy.rs / tests/clean_all.rs に上記 2 本を追加。実装前に落ちることを確認する。
3. Green: src/cleaner.rs に述語既定（`quits_running_apps() -> bool { false }`）と既定値 pin テストを追加。src/hint/cleaner.rs で true を返し、:223-225 の doc を実挙動に一致させ更新する。
4. Green: src/unattended.rs の Auto 腕と All 腕に述語分岐を追加（`SkipReason::NeverUnattended(risk)`）。モジュール doc を更新する。
5. Green: main.rs all 腕の skipped rendering に `NeverUnattended` 分岐を追加する（auto 腕の文言と同一）。
6. 検証ゲート（DoD の 3 コマンド）。

### 対象ファイル
src/cleaner.rs（述語既定 + トレイト pin テスト）、src/hint/cleaner.rs（述語 true + doc 更新）、src/unattended.rs（Auto / All 分岐 + matrix 拡張 + doc 更新）、main.rs（all 腕 rendering 分岐）、tests/auto_policy.rs、tests/clean_all.rs。

### 期待する Red → Green
単体（matrix 述語セル）と E2E（auto / clean all 2 本）が先に Red、実装後に全 Green。既存 48 セル matrix と既存 E2E は無修正で green が pin（回帰の検知）。

### design-it-twice
- Option A（採用）: select() での分類変更 + Cleaner trait 述語追加。PBI 74 pin「select() が選択の唯一所有者」に一致し、述語は 1 実装で Auto / All 両腕に効く。SkipReason 語彙は 3 variant のまま（NeverUnattended の語義を quit 系に拡張するだけ）で、UnattendedPolicy 型も変わらない。
- Option B（却下）: main.rs 腕でのフィルタ追加。腕ごとの手書きフィルタが復活し（PBI 74 が排除した形状）、registry-driven 2 経路で同じ述語を 2 回書く増殖が起きる。arm での exit gate（[policy].present）と selection input の区別も崩れる。
- Option C（却下）: interactive_gated() の再利用。PBI 75 の意味（--yes が NonInteractive adapter で prompt gate する per-run 観測）と衝突し、src/unattended.rs:41-43 の doc（TTY runtime 事実は policy 入力ではない）に反する。hints は interactive_gated() = false なのでそもそも直らない。
- Option D（却下）: hints を Risk::Risky に変更。PBI 83 pin（risk() = Moderate、Safe にしない）に反し、Moderate の意味論（明示的選択が要るデータ）と合わない。scan --json / TUI の表示も変わる。

## pin（壊してはいけない契約）
- PBI 74 契約: select() が選択の唯一所有者（純粋な決定、I/O なし、レンダリングなし）、3 policy（Auto / All / Yes）と SkipReason 語彙（NeverUnattended / NeedsOptIn / NeedsExplicitSelection）は 3 variant のまま、48 セル matrix（src/unattended.rs selection_matrix_pins_the_three_policies、3 × 4 × 2 × 2）は期待値無修正で green
- PBI 83 の clean hints E2E: tests/hints.rs（clean_hints_deletes_hint_debris_and_writes_one_history_row / clean_hints_sub_cleans_only_that_entry / clean_hints_dry_run_deletes_nothing_and_lists_the_plan / clean_hints_unknown_sub_exits_nonzero / targets_lists_hints）が無修正で green。対話経路（scan の hint プロンプト、`clean hints`、TUI の CleanByName seam）は不変
- PBI 80 の hint 契約: hint の dry-run pin（tests/dry_run.rs、RecordingRunner に osascript / rm / chflags / open が現れない）、quit_app_and_wait の osascript + pgrep 経路（PBI 53 の osascript 失敗 bail 含む）、is_yes_answer 共有
- PBI 80 の hint TTY 限定契約: 非 TTY は run_interactive が exit 1 で hint に到達しない（tests/interactive.rs:108）が無修正で green
- `--yes` の scan-driven 除外（現状維持）: hints は Pruneable でも `auto_clean_allowed() = false` のため `NeedsExplicitSelection` で除外される。Yes 腕は述語を参照しない
- colima 等の他ターゲットの分類: colima は `Risk::Risky`（tests/auto_policy.rs auto_never_runs_colima_even_with_include_manual が pin）、node-gyp など他の Moderate ターゲットは include_manual で解放される現行分類（auto_runs_moderate_targets_when_include_manual / clean_all_include_manual_cleans_moderate_targets が pin）。述語 true の target は今日 hints のみ
- auto 腕の rendering（main.rs）: `NeverUnattended` → "never runs unattended"、`NeedsOptIn` → "set [policy] include_manual = true to include" の既存文言
- clean_all.rs 既存の E2E（"Skipped spm (Moderate)" + "needs --include-manual"、--include-manual で spm 掃除 + "Permanently deleted"）は無修正で green
- pbi/README.md 設計方針: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用

## scope fence
1. PBI 74 の policy 語彙変更はしない（3 policy・SkipReason 3 variant のまま）。UnattendedPolicy 型の変更（新 variant、field 追加）はしない
2. hints cleaner 本体（quit/relaunch 経路、auto_clean_hint、quit_app_and_wait、relaunch_app、KNOWN_ENTRIES、threshold）は変更しない（述語の戻り値追加と doc 更新のみ）
3. `--yes` 経路（`UnattendedPolicy::Yes`）は変更しない
4. offer（TTY）経路は変更しない（offer_auto_clean、run_hint_flow、scan の hint プロンプト、TUI、`clean hints` 腕）
5. `clean caches` 腕（7 つの Moderate 汎用キャッシュを無条件で走らせる）は変更しない
6. KNOWN_ENTRIES の config 化・外部ファイル化、top5 truncate（detector.rs）と threshold_bytes は触れない
7. config.toml の [policy] スキーマ変更（新 flag 追加など）はしない。include_manual の意味論自体は変えず、unattended 選別側でのみ quit 系を除外する
8. Linux / Windows 対応なし
9. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] BDD 受け入れシナリオ 4 本が自動テストとして実装され、パスする
- [ ] pin の列挙がすべて無修正で green していることを確認する
