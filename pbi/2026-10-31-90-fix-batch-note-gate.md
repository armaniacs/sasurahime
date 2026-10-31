出典: 持ち越し候補深掘り（round 4、2026-10-31）の R4「note gate の所有者割れ」（.plan/whywhy-round4-option-a.md、読み取り専用分析・コード変更なし）。中間解の (1) note gate 統一と (2) 行家族所有者表が本 PBI の該当範囲

# PBI-90: バッチ経路の note 出力を reporter 条件で統一する

## ユーザーストーリー
スクリプトや cron で `sasurahime clean all --deep-suppress` / `auto --deep-suppress` を走らせる開発者として、深い抑音（deep-suppress）を選んだときにバッチ経路から進行系の出力が 1 行も出ないことがほしい、なぜなら `clean all --deep-suppress` は 1 target ごとの note（"Permanently deleted: no Trash copy was kept." 等）を出し続ける一方、単発の同一操作（`clean library-logs --all --deep-suppress`）は出さないため、行数で grep / sed するスクリプトが経路ごとに別の期待を書かされるから。

## 優先度
RICE: Reach 2 × Impact 2 × Confidence 95% / Effort 0.1 人週 = スコア 38

- Reach 2: deep-suppress × バッチ経路（`clean all` / `auto`、main.rs:1372 と main.rs:1715 の with_notes=true）の利用者。--suppress と既定は現行どおり note 表示のため対象外。
- Impact 2: スクリプト出力を静かに壊す唯一の残差分。deep-suppress の行数想定が 1 target ごとに +1 行され、単発と同じ期待を書いた script が破れる。この組合せ（deep-suppress × バッチ × note）への E2E が 0 本のため、破れが CI で表面化しない。
- Confidence 95%: 修正は batch.rs の note 枝への 1 条件追加のみ。到達面（with_notes=true × show_spinner=false）はコードで実測済みで、--suppress と既定の現行表示は既存 pin が守る。
- 前提: PBI 78（.plan/archived/2026-10-30-78-fix-batch-cancel-contract.md）の着地済み。note gate は PBI 78 の scope fence の外に置かれ、整列対象として列挙されなかった。

## 背景（なぜなぜ分析 round 4 の発見）
1. PBI 78 が整列したのは status 行・cancel・履歴 guard・SkipConfirmGuard 位置までで、note（post_clean_note）の gate は漂い続けている。run_batch は with_notes（engine bool）だけで note を出し（src/batch.rs:76-81）、単発 run_clean_target は show_spinner() 枝の内側で出す（src/main.rs:925-941）。行家族ごとの所有者が割れたまま。
2. run_batch が with_notes だけで gate するため、`clean all --deep-suppress` / `auto --deep-suppress` は with_notes=true × show_spinner=false の組合せになり、per-target の note（"Permanently deleted: no Trash copy was kept." / "Restore from Finder (Trash) or run `open ~/.Trash`."）を 1 target ごとに出す。単発の同一操作は出さない。
3. スクリプト（grep / sed）の行想定を静かに壊す。バッチ側の note は E2E 0 本で、deep-suppress の pin は単発の Freed 非表示のみ（tests/progress.rs:55-76）。

### 実コードの確認（main @ d437647、着手前確認を含む）
- run_batch の note 枝（src/batch.rs:76-81）: `if with_notes { ... post_clean_note(...) ... }` だけで出す。reporter には一切問い合わせない。post_clean_note 本体は src/batch.rs:112-125。
- run_clean_target の note（src/main.rs:925-941）: `if reporter.show_spinner()` 枝の内側。note 出力は 934-940 行目（freed_message の直後）。
- with_notes=true の呼び出し筋: auto（main.rs:1366-1373、with_notes は 1372）、clean all（main.rs:1709-1716、with_notes は 1715）。
- with_notes=false の呼び出し筋: clean caches（main.rs:1006、"this entry point never printed them"）、TUI --yes の run_auto（interactive.rs:337）、TUI whole-target（interactive.rs:442）。この arm 差は各 arm の過去出力を再現する保存方針で、本 PBI では触れない。
- 既存 E2E の pin 範囲: tests/progress.rs の deep_suppress_flag_hides_freed_line（54-77 行目、assert は 71-76）は単発の Freed 非表示のみ。note 行への E2E assert は 0 本。既定（--deep-suppress なし）のバッチ note 表示は tests/clean_all.rs:77（clean_all_include_manual_cleans_moderate_targets の "Permanently deleted" assert）が pin。
- 既存 source pin: batch.rs:405-437（run_batch_status_line_is_owned_by_the_reporter_kind）の走査範囲は `let result = if animated {` から `match result {` まで。note 枝はこの範囲の外（match の Ok 枝）にあり、gate 追加は pin を壊さない。
- 着手前確認（フェーズ0）: note 枝に reporter.show_spinner() 条件は存在しない（未実装を確認）。.plan/archived/ に重複 PBI なし（PBI 78 は status 行と cancel の整列、PBI 87 は hints 除外で隣接 scope）。

## BDD 受け入れシナリオ

```gherkin
Scenario: clean all --deep-suppress は per-target の note を出さない
  Given TTY なしで `sasurahime clean all --deep-suppress --include-manual` を実行できる
  And  掃除対象のキャッシュにしきい値以上の残骸が存在する
  When clean all を走らせる
  Then 1 target ごとの note（"Permanently deleted: no Trash copy was kept."）は出力されない
  And  掃除自体は実行され exit 0 で終わる
  And  単発の `clean <target> --deep-suppress` と同じ行数になる

Scenario: clean all --suppress と既定は note を出し続ける（現行維持）
  Given TTY なしで `sasurahime clean all --include-manual`（--deep-suppress なし）を実行できる
  And  掃除対象のキャッシュにしきい値以上の残骸が存在する
  When clean all を走らせる
  Then 1 target ごとの note は今日どおり出力される
  And  --suppress も show_spinner() が true を保つため note は現行どおり出る
```

## 受け入れ基準
- [ ] src/batch.rs の note 枝（76-81 行目付近）に reporter.show_spinner() 条件が追加される（1 行の gate）。--deep-suppress（quiet reporter）では note を出さない
- [ ] --deep-suppress なし（既定）と --suppress では note を出し続ける（show_spinner() が true のとき現行と同じ振る舞い、単発と一致）
- [ ] `clean all --deep-suppress` で per-target note が出ない（E2E 1 本を tests/progress.rs 付近に追加）
- [ ] 既定側の note 表示は既存 pin で済む: tests/clean_all.rs:77 が無修正で green するため、既定側の追加 E2E は置かない（本 PBI で明記）
- [ ] run_batch の doc comment（src/batch.rs:20-34）に行家族所有者表が追加される（status / cancel / note / freed / skip / exit の 6 家族 × 単発・バッチの所有者と gate 条件を 1 表）
- [ ] doc comment に Option A（完全統合）の発火条件の備考が 1 行書かれる（「次にエンジンの意味論を触る変更」を発火とする。Option A の先送り自体は維持）
- [ ] 既存 pin が無修正で green: batch.rs:405-437 の source pin、tests/progress.rs 全既存、tests/clean_all.rs 全既存、tests/auto_policy.rs、tests/interactive.rs、tests/history.rs

## テスト戦略
Outside-In TDD。E2E で Red を確認してから、内側へ向かって Red → Green → Refactor を繰り返す。

- E2E Red: tests/progress.rs 付近に 1 本。既存 pin clean_all_include_manual_cleans_moderate_targets（tests/clean_all.rs:54-80）と同じ残骸（spm キャッシュ + --include-manual）に --deep-suppress を足し、stdout+stderr combined に "Permanently deleted" が現れないこと、exit 0、残骸は削除されること（消えるのは note だけで掃除は変わらない）を assert する。現行コードでは note が出るため先に Red になる。
- E2E（既定側は追加しない）: 既定（--deep-suppress なし）の note 表示は tests/clean_all.rs:77 が pin。追加せず、無修正で green することを確認する。
- 単体: post_clean_note の既存 3 本（src/batch.rs tests モジュール、restore guidance / permanent audit trail / zero freed 無音）は無修正で green が pin。note 枝の gate 自体は indicatif が出力 capture seam を公開しないため println を直接観測できず、E2E 1 本で担保する。追加の source pin（batch.rs:405-437 と同じスタイル）を置くかは実装時に design-it-twice で決める。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針どおり）。E2E は tempdir HOME + 実残骸で駆動する。

## 見積もり
0.1 人週

内訳: note 枝への 1 条件追加（0.02）+ E2E Red/Green 1 本（0.04）+ doc comment の行家族所有者表と Option A 備考（0.04）。

## 実装計画

### 実装順
1. E2E Red: tests/progress.rs 付近に `clean all --deep-suppress` の note 非表示 1 本を追加し、現行（note が出る）で落ちることを確認する。
2. Green: src/batch.rs:76-81 の note 枝に reporter.show_spinner() 条件を追加する（1 行）。--deep-suppress 時は note 非表示に統一、既定と --suppress は現行どおり note 表示。pin（batch.rs:405-437 の source pin）には触れない。
3. doc comment: run_batch の doc comment（src/batch.rs:20-34）に行家族所有者表を追加し、Option A 発火条件の備考を 1 行書く。doc comment の言語は既存の英語に合わせる。
4. 検証ゲート（DoD の 3 コマンド）。

### 対象ファイル
src/batch.rs（note 枝の gate + doc comment）、tests/progress.rs（E2E 1 本）。

### 期待する Red → Green
E2E（clean all --deep-suppress の note 非表示）が先に Red、実装後に全 Green。既存 pin と既存 E2E は無修正で green が pin（回帰の検知）。

### 行家族所有者表（doc comment に置く内容）
実測サマリ（whywhy round 4 の 0. 実測サマリ表）を短縮した 6 家族 1 表:

| # | 行家族 | 単発（run_clean_target）の所有者 | バッチ（run_batch）の所有者 | gate 条件 |
|---|---|---|---|---|
| 1 | status 行（[OK]/[FAILED]） | show_spinner() 枝（main.rs:893-915） | reporter 種別: animated → with_spinner_result_progress / show_spinner() → with_spinner_result / quiet → なし | animated は呼び出し側 flag、show_spinner() から導出しない |
| 2 | cancel 行 | Ok(cancelled)、無音 | summary.cancelled → 呼び出し側の中立行 "cancelled by user" | Err(CleanCancelled) の downcast 分岐 |
| 3 | note 行（post_clean_note） | show_spinner() 枝の内側（main.rs:934-940） | with_notes かつ reporter.show_spinner()（本 PBI で統一） | --deep-suppress は非表示、既定と --suppress は表示 |
| 4 | freed 行 | per-target の freed_message、show_spinner() 枝のみ | 呼び出し側の締め行 "Total freed"（report_total_freed） | per-target と合計は概念差として別物 |
| 5 | skip 行 | 直出力 "N file(s) skipped" + "path: reason"（名前なし） | summary.skipped → report_batch_failures（Named 形状、cleaner 名付き） | skipped が空でないとき |
| 6 | exit code | exit_code() != 0 → exit 1 | 常時 exit 0（報告のみ） | PBI 78 scope fence 2 で意図的に固定 |

備考（1 行）: Option A (full unification) stays deferred; it fires on the next change that touches either engine's semantics (a new line family, a gate move, or a gate condition change), since the previous trigger "next new entry point" never fires naturally.

### design-it-twice
- Option A（採用）: note 枝への reporter.show_spinner() 条件追加。status 行と同じ reporter 税別の gate に統一され、単発（show_spinner() 枝の内側、main.rs:934-940）と一致する。pin（batch.rs:405-437）は無修正で green。
- Option B（却下）: run_clean_target を with_notes 引数化して run_batch に統合。whywhy round 4 判定 1（Option A 先送り維持）に反し、単発経路の変更が本 PBI の scope fence で禁止されている。
- Option C（却下）: post_clean_note 関数側に reporter を渡す。post_clean_note は bytes / trash から note を作る純粋な決定で、reporter の知識を入れると関数の契約が割れ、既存単体テスト 3 本の形状も変わる。

## pin（壊してはいけない契約）
- batch.rs:405-437 run_batch_status_line_is_owned_by_the_reporter_kind（source pin）: `let result = if animated {` から `match result {` までの走査範囲は無修正。note 枝への gate 追加はこの範囲の外（match の Ok 枝）にあり、pin が無修正で green のまま維持されること
- PBI 78 の契約整列: [OK]/[FAILED] status 行の所有者 = reporter 種別、animated は show_spinner() から導出しない（no-tick-while-prompting）、BatchSummary.cancelled チャネル + downcast 分岐、SkipConfirmGuard は呼び出し側残留、履歴 guard の単一所有者 write_history_entry_if_needed（batch.rs:136-146、zero-byte 無記録を含む）
- post_clean_note（batch.rs:112-125）とその単体テスト 3 本（restore guidance / permanent audit trail / zero freed 無音）は無修正
- 単発経路: run_clean_target（main.rs:882-974）の note 出力（show_spinner() 枝の内側、main.rs:934-940）と freed_message、Ok(cancelled)・exit 0 契約は無修正
- バッチ arm の with_notes 実数: clean caches = false（main.rs:1006）、auto = true（main.rs:1372）、clean all = true（main.rs:1715）、TUI = false（interactive.rs:337 / 442）。この arm 差（各 arm の過去出力を再現する保存方針）は現行維持
- E2E: tests/progress.rs 全既存（:40-52 suppress の Freed 表示、:55-77 deep-suppress の Freed 非表示、:79-95 既定の Freed 表示）、tests/clean_all.rs 全既存（:54-80 --include-manual で掃除 + "Permanently deleted"）、tests/auto_policy.rs、tests/interactive.rs、tests/history.rs が無修正で green
- pbi/README.md 設計方針: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用

## scope fence（絶対禁止）
1. run_clean_target / run_batch の統合（Option A）はしない（whywhy round 4 判定 1 で先送り維持。発火条件の備考は doc comment に 1 行書くのみ）
2. 単発経路の変更はしない（run_clean_target / run_and_exit の note 出力、freed_message、skip 行、Ok(cancelled) 契約すべて不変）
3. --suppress（非 deep）と既定の note 表示は変更しない（show_spinner() が true のとき現行と同じ振る舞いのみ）
4. PBI 78 の 4 guard（SkipConfirmGuard hoist、animated 導出、履歴 guard、label）と exit code・failures 型への接触はしない（BatchSummary の型変更、failures の Vec<(String, String)> 置換はしない）
5. post_clean_note 関数本体の変更はしない（呼び出し側の gate のみ）
6. `clean caches` 腕（with_notes=false、main.rs:1006）と TUI 経路（interactive.rs:337 / 442、with_notes=false）の変更はしない（arm 差は保存方針）
7. CHANGELOG.md は触らない（マージ後に本体で一括記載）
8. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] BDD 受け入れシナリオが自動テストとして実装され、パスする
- [ ] pin の列挙がすべて無修正で green していることを確認する

## 備考
- 実装は worktree で subagent-driven。対象が独立ファイル（src/batch.rs + tests/progress.rs 付近）で pin との接触範囲が狭いため、単独タスク 1 本で着手できる。
- 実装時に、単発 cancel の中立行（R5）を入れるかの判断を 1 行記録する（現行維持も GenericCleaner hint 前提で成立。本 PBI の scope の外、whywhy round 4 中間解 (3)）。
- 完全統合（Option A）の前段 3 段階（failures の型変更 → BatchSummary に results 追加 → report_single_target ヘルパ化）は本 PBI の範囲外。発火条件は doc comment の備考 1 行に記録する。
