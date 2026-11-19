出典: holistic-code-improvement round 9（候補 3「無人バッチ実行フローの 3 重複」）+ なぜなぜ分析 round 9（`.plan/whywhy-round9-entry-dispatch.md` §2・§3、2026-11-20、main @ c107011 実測、読み取り専用分析）。RICE 採点表は `pbi/2026-11-20-00-backlog-holistic-round9.md` の候補 3 行。

# 2026-11-20-03: 無人バッチ骨格の 1 所有者化 + skip レンダラ 1 箇所化

- 種別: refactor
- 規模: M
- 優先度: 2 位（RICE 2.0 = Reach 5 / Impact 1 / Confidence 0.8 / Effort 2 SP）
- 先行依存: 2026-11-20-02（main() フロー抽出）— 同一ファイル・同一領域のため直列。02 で抽出された `fn run_<flow>` が本 PBI の共有化の当て先になり、diff が最小になる（なぜなぜ round 9 §3 着手順）
- 対象ファイル: `src/main.rs` + `src/batch.rs` + `src/interactive.rs`。`src/unattended.rs` は触れない（PBI 74 決定）

## 趣旨（一文）

PBI 74 の rendering 呼び出し側所有を維持したまま、実行順序（骨格）を 1 所有者に寄せる。

## ユーザーストーリー

As a 保守担当者（半年後の sasurahime を保守する人）, I want 「UnattendedPolicy::select → skipped 描画 → run_batch → report_batch_failures → report_total_freed」の 5 段骨格と SkipReason を描画する match が 1 箇所に住んでいること, so that 次に SkipReason に新変数を足すとき、auto / clean all / --yes の 3 経路すべての描画更新が非網羅 match の build error として義務付けられ、「--yes だけ新変数が汎用 1 行に黙って流れ込み、skip の理由が沈黙する」事故が構造的に起こせなくなる。**表示は 1 文字も変わらない**（byte 等価が本 PBI の前提。文言・流れる stream・出力順・終了コードはすべて現行のまま）。

## 背景（確定事実、file:line 実測）

- 5 段骨格の 3 重複: 「UnattendedPolicy::select → skipped 描画 → run_batch → report_batch_failures → report_total_freed」の順序が 3 箇所に手書きされている。(1) auto arm `src/main.rs:1333-1400`、(2) clean all arm `src/main.rs:1670-1743`、(3) `interactive::run_auto` `src/interactive.rs:324-431`
- auto arm の skip 描画は SkipReason 3 変数（`NeverUnattended` / `NeedsOptIn` / `NeedsExplicitSelection`、unattended.rs:86-103）の match で advice 文言を組みて eprintln に流す（main.rs:1348-1372、guidance 行 1393-1399）。all arm は同型の match を println で（main.rs:1687-1715、count 行 1734-1739）
- **run_auto の語彙沈黙構造（本 PBI の核心）**: run_auto だけが match せず `reason.risk().label()` の汎用 1 行（interactive.rs:348-352）。SkipReason に新変数を足すと auto/all は非網羅で build error になり修正が強制されるが、run_auto は variant を区別しないため**コンパイルが通ってしまい**、新変数の意味（「--include-manual で解放できる」か「明示選択が必要」か）が --yes の出力から沈黙する。回帰温床の実在（なぜなぜ round 9 §2.a 1-13）
- run_auto 固有の実行時文脈: `gate_skipped_names` による TTY 観測（interactive.rs:334、PBI 74 が「per-run observation」として切り分けた TTY の事実）、permanent 確認（interactive.rs:374-393）、`SkipConfirmGuard`（interactive.rs:396）
- `run_batch` の署名は位置引数 bool 3 つ（dry_run / animated / with_notes、batch.rs:91-98）で、3 呼び出しが main.rs:1378-1385 / main.rs:1721-1728 / interactive.rs:409-416 に並ぶ（`false, false, true` 等の意味が目の前で読めない形。なぜなぜ round 9 §2.a 14-18）
- PBI 74 の記録決定: rendering（文言・stdout/stderr 選択）は呼び出し側に置く（unattended.rs:38-44 が明記）。select は純粋決定で unit-testable に保つ（96 セル matrix pin `selection_matrix_pins_the_three_policies`、unattended.rs:418-479）。この決定は「rendering と stream 選択」を語るもので、実行順序（骨格）については沈黙しており、沈黙が骨格複製を黙認してきた（なぜなぜ round 9 §2.b 4-7）
- stream の非対称: auto が skipped を stderr、all と --yes が stdout に流す現行挙動。skip 報告の文言は既存 E2E pin の対象だが、pin が stdout/stderr を連結した `combined` で assert している箇所が実在する（tests/auto_policy.rs:150-158、tests/clean_all.rs:155-163）ため、**stream 分離自体が pin 済みかは棚卸しで確認し、不足分を先に足す**（parity pin の順序規律。なぜなぜ round 9 §1.b 28-31・§2.b 27-28）
- 棚卸し対象の既存 pin（実測）: tests/auto_policy.rs:141・:195・:259・:320（auto の skip + guidance 系）、tests/clean_all.rs:24・:148・:236・:255（clean all の skip 系）、tests/ollama.rs:120（`--yes` の gate_skipped 行）、tests/scan.rs:428（`--yes` の skip 系）

## BDD 受け入れシナリオ

```gherkin
Scenario: auto / clean all / --yes の skip 報告が現行と byte 等価
  Given auto / clean all / --yes の skip 出力（文言と流れる stream を含む）の pin 棚卸しを移設の前に済ませてあり、不足分の pin は移設に先立って追加してある
    And 棚卸し対象の既存 pin が実在する（tests/auto_policy.rs:141・:195・:259・:320、tests/clean_all.rs:24・:148・:236・:255、tests/ollama.rs:120、tests/scan.rs:428）
  When  無人バッチ骨格の共有化（3 経路の移設）を適用する
  Then  3 経路の skip 報告が移設前と byte 等価である（auto は stderr の advice 行 + guidance 行、clean all は stdout の "Skipped <name> (<risk>): ..." 行 + count 行、--yes は stdout の汎用 1 行 + gate_skipped 行）
    And  stream の分離（auto = stderr、all と --yes = stdout）を含めて等価である
    And  履歴記録・終了コード・report_batch_failures / report_total_freed の出力も含めて等価である

Scenario: SkipReason の新変数追加が 3 経路の描画更新を build error で義務付ける
  Given SkipReason の全変数を扱う match が共有レンダラ 1 箇所に集約され、auto / clean all / --yes の 3 経路がそのレンダラ経由で skip を描画している
    And 共有レンダラ match の所在（関数定義行）が source pin で固定されている
  When  SkipReason に仮想の第 4 変数を足す変更を仮定する（または一時的に追加して取り消す）
  Then  共有レンダラの match が非網羅になり、3 経路分の描画更新漏れが 1 箇所の build error として検知される
    And  この構造（match が 1 箇所のみ・run_auto が汎用 1 行を直接書いていない）が source pin として記録される
```

## 受け入れ基準

- [x] 「selection → run_batch → BatchSummary」の骨格を batch.rs に 1 所有者として新設する（`run_batch` / `post_clean_note` / `write_history_entry_if_needed` と同じモジュール）。3 呼び出し（main.rs:1378-1385 / :1721-1728 / interactive.rs:409-416）が骨格関数を通る。bool 位置引数の意味付き呼び出しは骨格 1 箇所に集約される
- [x] skip 描画は advice 文言を引数に取る共有レンダラへ寄せ、SkipReason の match を 1 箇所化する。stream への書き込み（println!/eprintln! の選択）は呼び出し側が行う（PBI 74 決定の維持）。行構造 "{name} ({risk}): {advice}" は現行維持
- [x] run_auto の `gate_skipped_names` による TTY 観測（interactive.rs:334）、permanent 確認（:374-393）、`SkipConfirmGuard`（:396）は呼び出し側に維持される（骨格に吸収しない。TTY の事実と確認文脈は PBI 74 が呼び出し側に置いたもの）
- [x] rendering（文言・stream 選択）を unattended.rs に入れない（PBI 74 決定の維持、unattended.rs:38-44）。`UnattendedPolicy::select` は純粋決定のまま。96 セル matrix pin（unattended.rs:418-479）は無修正で green
- [x] 移設前に auto/all/yes の skip 出力 pin（文言・stream 含む）を棚卸しし、不足分を先に足す（parity pin は「現行出力を pin してからリファクタ」の順序で初めて意味を持つ）。`combined` 連結 assert で stream 分離が未 pin の箇所は、分離 pin を移設前に追加する
- [x] SkipReason 全変数を扱う match が 1 箇所のみであることを source pin（include_str! grep、match 定義行アンカー）で固定する。pin の運用は tests/AGENTS.md の規約（定義行アンカー、pinned literal を pin 自身のソースに直接書かない）に従う。新変数追加時に非網羅 build error が 3 経路すべてに効く構造である
- [x] 全 E2E pin 無傷（byte 等価の実測を記録）+ `cargo clippy --tests -- -D warnings` 0 + `cargo fmt --check` clean

## テスト戦略

parity pin 先行（pin 棚卸し → 不足 pin 追加 → 移設 → pin 無修正 green の順）。selection matrix 96 セル pin は無傷（select は触れないため fixture も無修正）。

- 準備（parity pin）: auto/all/yes の skip 出力を stream 含めて棚卸しし、`combined` 連結のまま stream 分離が未 pin の箇所に分離 pin を先に足す（現行実装で green になる characterisation pin）。不足 pin が先行するので、移設時の red は「移設で壊した」ことのみを意味する
- 新規 unit pin: 共有レンダラの純関数 pin（SkipReason 3 変数 × advice 引数 → 行文字列。risk label の組合せを含む）。advice を引数に取る形状のため CommandRunner モック不要
- 移設後の走査: 3 経路の既存 E2E pin（tests/auto_policy.rs、tests/clean_all.rs、tests/ollama.rs:120、tests/scan.rs:428）が無修正で green することで byte 等価を担保。期待値の書き換えは原則禁止（pin の無修正が移設の正しさの唯一の観測）
- source pin: 共有レンダラ match の所在固定（BDD2 の Then）。振る舞いとして現れない構造の最後の手段として、tests/AGENTS.md の source pin 要件に合致する

## 見積もり

2 SP（RICE 採点表どおり。骨格新設 1 + レンダラ寄せ 1 + pin 棚卸し・走査。02 済み前提で arm は既に `fn run_<flow>` の形）

## DoD

- [x] cargo test が green（既知の TCC 依存 4 失敗を許容: `cleaners::huggingface::tests::clean_fallback_deletes_dir`、`cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries`、`cleaners::pre_commit::tests::clean_fallback_deletes_dir`、`trash::tests::delete_ctx_trash_mode_moves_to_trash`。一覧は PBI 94 / 101 の DoD と同一、実装時に green でない場合はその時点の既知失敗に合わせる）
- [x] cargo clippy --tests -- -D warnings が 0
- [x] cargo fmt --check が通る
- [x] byte 等価の実測記録（3 経路の skip 出力を移設前後で比較した結果を PBI 完了報告に残す。pin の green だけでなく実測の記録を残す）
- [x] PBI アーカイブ（統合側が実施: `git mv` → `.plan/archived/`、INDEX 追記。サブエージェントは git 操作しない）

## scope 外（やらないこと）

- **--yes の汎用 1 行への語彙追加**: 出力変更（ユーザー可視）であり別判断事項。台帳の台帳送りに記録済み（`pbi/2026-11-20-00-backlog-holistic-round9.md`「--yes の skip 汎用 1 行への語彙追加」、再検討トリガー: 候補 03 着地後）。本 PBI の共有レンダラ + match 1 箇所化が将来の足場になる
- **advice 文言の統一**: 挙動変更。auto の "set [policy] include_manual = true to include"（main.rs:1357-1361）と all の "needs --include-manual"（main.rs:1700-1704）は経路固有の advice として現行維持
- **stream 非対称の解消**: 挙動変更。auto が skipped を stderr、all と --yes が stdout に流す現行の非対称は維持する。本 PBI が担保するのは「stream 選択が呼び出し側のコードに明示される」ことまで

## 備考

- PBI 74 決定の再読が本 PBI の出発点: 決定文（unattended.rs:38-44）は rendering と stream 選択を呼び出し側に置くと言うが、実行順序（骨格）については沈黙している。骨格の複製は rendering ではなく**実行順序**の複製で、実行順序を共通化しても PBI 74 の決定に触れない（なぜなぜ round 9 §2.a 21-23）
- 共有先を batch.rs とする根拠: run_batch / post_clean_note / write_history_entry_if_needed という実行の契約の既存の所有者だから。unattended.rs は PBI 74 が純粋決定のみに保った場所で、実行（I/O）を入れると selection matrix 96 セル pin の前提（純粋性）を汚染する
- 触れるファイル: `src/main.rs`（auto/all arm の骨格呼び出しへの置き換え、02 で抽出された `fn run_<flow>` 内）、`src/batch.rs`（骨格関数 + 共有レンダラ新設）、`src/interactive.rs`（run_auto の寄せ）。`src/unattended.rs` は触れない

## 実装記録（統合側、2026-11-20）

- batch.rs に新設: run_selected_batch（selection → run list → run_batch → BatchSummary の骨格 1 所有者、位置引数 bool の意味付き呼び出しを集約）、SkipAdvice（経路別 advice テーブル。None が --yes の汎用行形状）、render_skip_line + render_advised_line（SkipReason の match を集約する唯一の純粋関数、印刷しない）
- 集約位置を batch.rs にした根拠: main.rs 置き場だと lib 側の interactive.rs から参照できないため、実行の契約の既存所有者である batch.rs が唯一の共有可能位置
- 3 呼び出しの移設: run_auto_flow（match 廃止 → advice テーブル + レンダラ、guidance 行の出力位置・文言は現行維持）、run_all_flow（同型 match 廃止 → レンダラ経由 println、batch 前の stdout・現行順序維持）、interactive::run_auto（汎用 1 行廃止 → 全 None advice のレンダラ、byte 等価）。gate_skipped / permanent confirm / SkipConfirmGuard は呼び出し側に維持。unattended.rs には無接触、96 セル matrix pin 無修正で green
- 構造 pin: the_skip_reason_match_lives_only_in_the_shared_renderer（batch.rs tests。レンダラ定義行アンカーで 3 変数リテラル各 1 回・match 存在・ワイルドカード不在を pin。main.rs / interactive.rs 全文で SkipReason 参照 0 件・.risk().label() 直書き 0 件）
- 棚卸し結果: auto の NeedsOptIn advice 文言（"set [policy] include_manual = true to include"）が未 pin → 移設前に auto_skip_optin_targets_name_the_policy_optin_on_stderr を補完し現行実装で green 実測（12 passed）。all / --yes は PBI 02 補完分と既存 pin で充足
- byte 等価実測（/tmp/whywhy/byte-parity/ にキャプチャ）: auto stderr "  - node-gyp (Moderate): set [policy] include_manual = true to include" / "  - hints (Moderate): never runs unattended" / "Run `sasurahime clean <target>` explicitly for those."、all stdout "Skipped spm (Moderate): needs --include-manual" + stderr "Skipped 40 target(s) that need --include-manual."、--yes stdout "Skipped 2 target(s) that need an explicit selection: cargo (Moderate), vm-swap (Manual)"。終了コード 3 経路とも 0、pin 期待リテラルと完全一致（既存 pin 無修正）
- 検証実測: cargo fmt --check clean / clippy 0 / cargo test --bin sasurahime 50 passed / cargo test --lib 904 passed / cargo test --tests 41 ターゲット全 green
- 注記: run_caches_batch（clean caches arm）は無人 3 経路の外（unattended.rs doc の scope 外明記）のため run_batch 直呼びのまま維持
- 実装コミット: 2270865（batch.rs + main.rs + interactive.rs + tests/auto_policy.rs）
