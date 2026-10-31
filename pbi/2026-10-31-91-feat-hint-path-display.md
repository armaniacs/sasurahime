出典: なぜなぜ分析 round 4（.plan/whywhy-round4-known-entries.md、2026-10-31）の収束した解「実装すべきもの（今すぐ 2）」

# PBI-91: hint の選択プロンプトと dry-run に削除先パスを表示する

## ユーザーストーリー
sasurahime の hint 掃除（`sasurahime scan` の対話プロンプト、`sasurahime clean hints`）を使う開発者として、選択プロンプトと dry-run に削除先パスが表示されることがほしい、なぜなら現行は display_name とサイズしか表示されず、ユーザーは「vetted された名前 ⇔ パス対応」を信じて y を押すしかなく、何がどこに削除されるのかを目視確認できないから。

## 優先度
RICE: Reach 2 × Impact 2 × Confidence 85% / Effort 0.25 人週 = スコア 13.6

- Reach 2: hint の対話プロンプト（`scan` / TUI の hint フロー）と `clean hints --dry-run` の 2 経路の利用者。
- Impact 2: 「名前を信じる契約」の監査可能性。削除先パスが見えないため、ユーザーは display_name だけを信じて削除を承認するしかない。パス表示で「何がどこに消えるか」が目視確認できるようになる。
- Confidence 85%: KNOWN_ENTRIES は vetted 静的テーブル（22 件、実数確認済み）で、パスの組み立てに必要な情報（base_dir + path_suffixes、src/hint/mod.rs:19-33 と :58-64）が既にある。表示追加のみで動作・削除経路は不変。
- 前提: なぜなぜ分析 round 4（config 化は見送り裁定）の「今すぐ 2」。PBI-80/83 の出力文言 pin に触れるため、pin 置換がセット。

## 背景/問題

1. hint の選択プロンプトは display_name + サイズのみでパスを表示しない（src/hint/cleaner.rs:386-394）。dry-run も同様（cleaner 側 :302-311、offer 側 :369-377）。ユーザーは「vetted された名前 ⇔ パス対応」を信じて y を押すしかない（分析の根本原因: エンドユーザー立場「名前を信じる契約」）。
2. `[[custom]]`（CustomPathCleaner）は verbose でパスを表示する（src/cleaners/custom.rs:47-49、`is_verbose()` → `with_target(self.path ...)`）ため、config 由来のパスの方が詳細を見せるという契約の不一致がある。hint 側 detect() の verbose は src/hint/cleaner.rs:261-264 の総称文字列（"Library/Caches, Library/Application Support, Library/Logs (known app hints)"）のみで、個別のパスは出ない。
3. KNOWN_ENTRIES は vetted 静的テーブル（src/hint/apps.rs:6、22 件 = 11 Caches + 5 AppSupport + 6 Logs、実数確認済み）なので、表示追加は安全。動作（quit / relaunch / 削除経路）と freed 計測は 1 行も変わらない。

### config 化は見送りの裁定（記録）
なぜなぜ分析 round 4 の裁定: KNOWN_ENTRIES 22 件の config 化は現時点では見送り（vetted 静的テーブルを維持）。本 PBI はその裁定に含まれる表変更・config 化を一切行わず、「名前を信じる契約」を監査可能にする表示追加のみを行う。

## 発見の詳細（実コードの確認、main @ d437647）

- 対話プロンプト: `offer_auto_clean`（src/hint/cleaner.rs:349-403）。running は :386-389、non-running は :391-394。どちらも display_name + `{size_str} will be freed` のみでパスは出ない。
- dry-run 行（cleaner 側）: `KnownAppHintsCleaner::clean`（src/hint/cleaner.rs:278-332）の dry-run ブロック :298-314。eprintln は :302-311（running :302-305、non-running :307-310）。
- dry-run 行（offer 側、本 PBI で確認した追加箇所）: `offer_auto_clean` の dry-run ブロック :365-380。eprintln は :369-377（running :369-372、non-running :374-377）。`scan --dry-run` 経路で表示され、tests/dry_run.rs:371 が pin。cleaner 側と同じ文言のため、パス追加は両方に同期させる。
- doc comment の引用: src/hint/cleaner.rs:223-229 が `Quit ... and clear cache? [y/N]` のプロンプト形状を引用している。文言変更とともに doc も同期させる。
- KNOWN_ENTRIES: src/hint/apps.rs:6、22 件（11 Caches + 5 AppSupport + 6 Logs）。`HintEntry` は src/hint/mod.rs:19-33、`path_suffixes: &'static [&'static str]`。`BaseDir` は src/hint/mod.rs:12-17（Caches / AppSupport / Logs）。`base_path()`（src/hint/mod.rs:58-64）が `~/Library/{Caches, Application Support, Logs}` に解決する（hint モジュール内から参照可能）。
- verbose 前例: `[[custom]]` → `CustomPathCleaner`（src/cleaners/custom.rs:47-49）。
- `offer_auto_clean` の呼び出し元: src/main.rs:1221（`run_hint_flow` :1211 が home を受けている）。パス表示のために home を渡す形にする場合、シグネチャ変更は本番 1 箇所（:1221）+ 単体テスト 4 箇所（src/hint/mod.rs:543 / :568 / :591 / :619）に波及する。
- テストで pin している debris: no-quit の "Diagnostic Reports"（BaseDir::Logs + ["DiagnosticReports"]、src/hint/apps.rs:212-222）→ 削除先パスは `~/Library/Logs/DiagnosticReports`。osascript が呼ばれない決定論の担保に使われている（tests/hints.rs:46-51、tests/dry_run.rs:328-335）。
- 既実装確認: パス表示は未実装（`will be freed` は src/hint/cleaner.rs:387 と :392 のみ、パス表示の実装は存在しない）。本 PBI の機能が既存コードに存在しないことを grep と実コード読みで確認済み。

## BDD 受け入れシナリオ

```gherkin
Scenario: 対話プロンプトが削除先パスを表示する
  Given KNOWN_ENTRIES の no-quit 候補（Diagnostic Reports）にしきい値以上の残骸が存在する
  When  `sasurahime scan` の hint プロンプトが表示される（パイプ stdin でも表示される）
  Then  プロンプトに display_name とともに削除先パス（base_dir + path_suffixes の解決結果）が表示される
  And   [y/N] の形式と quit → 削除 → relaunch の契約は従来どおり変わらない

Scenario: dry-run が削除先パスを一覧表示する
  Given KNOWN_ENTRIES の候補に残骸が存在する
  When  `sasurahime clean hints --dry-run` を実行する
  Then  dry-run 行に display_name とともに削除先パスが表示される
  And   何も削除されない（現行の dry-run 契約のまま）

Scenario: dry-run はプロンプトを出さない（現行維持）
  Given KNOWN_ENTRIES の候補に残骸が存在する
  When  `--dry-run scan` を y 付きパイプ stdin で実行する
  Then  dry-run 行にはパスが表示される
  And   プロンプト（[y/N]）は出力されず、y の入力は消費されない
```

## 受け入れ基準
- [ ] `offer_auto_clean` の対話プロンプト（running と non-running の両分岐、:386-394）に削除先パス（base_dir + path_suffixes の解決結果）が表示される
- [ ] `KnownAppHintsCleaner::clean` の dry-run 行（:302-311）に削除先パスが表示される
- [ ] `offer_auto_clean` の dry-run 行（:369-377）にも同一形式でパスが表示される（2 つの dry-run 箇所の文言がずれない）
- [ ] KNOWN_ENTRIES 22 件・`HintEntry` 構造体・quit / relaunch / 削除経路 / freed 計測は 1 行も変わらない（src/hint/mod.rs の tests と既存 E2E が無修正で green）
- [ ] 文言 pin を新文言に更新済み（置換対象は「実装計画」に列挙）
- [ ] 新 pin 1 本: パスが表示されること（E2E、パス文字列の assert）
- [ ] src/hint/cleaner.rs:223-229 の doc comment が新文言と一致している
- [ ] DoD の 3 コマンドが通る

## テスト戦略（Outside-In TDD）

- E2E Red（1 本）: パスが表示されることの pin。現行実装ではパスが表示されないことを Red で検知する。場所は tests/dry_run.rs（`scan` のプロンプトパス表示）または tests/hints.rs（`clean hints --dry-run` の行パス表示）のいずれか。パイプ stdin の `run_with_stdin` ヘルパ（tests/dry_run.rs:340-357）と no-quit の "Diagnostic Reports" debris で決定論を担保する（osascript が呼ばれない）。assert は `~/Library/Logs/DiagnosticReports`（BaseDir::Logs + ["DiagnosticReports"] の解決結果）の文字列で検証する。
- E2E（回帰の pin）: tests/dry_run.rs:359-382（dry-run はプロンプトを出さない + 実ファイル生存）と tests/hints.rs:132-153（dry-run は何も削除しない）が無修正で green し続けること。
- 統合 / 単体: src/hint/mod.rs の `offer_auto_clean_*`（:530 / :559 / :580 / :601）と `hints_cleaner_*`（:770 付近）は選択意味論と no-execution 契約の pin で文言を pin しないため、シグネチャ変更（home を渡す場合）の呼び出し更新のみで green になる。パス組み立てを関数化した場合はその単体テストを 1 本足してよい（Negotiable）。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針）。本 PBI は表示追加のみなので、モックの既存契約は変わらない。

## 実装計画

### 実装順
1. E2E Red: 新 pin 1 本（パスが表示されること）を追加し、現行実装で落ちることを確認する。
2. Green: src/hint/cleaner.rs の対話プロンプト（:386-394）にパス（base_dir + path_suffixes）を追加する。
3. Green: dry-run 行にもパスを追加する（cleaner 側 :302-311 と offer 側 :369-377 の両方。同一形式で同期させる）。
4. pin 置換: 下記の文言 pin を新文言に更新し、src/hint/cleaner.rs:223-229 の doc comment を同期させる。
5. 検証ゲート（DoD の 3 コマンド）。

### 文言 pin の置換対象（列挙）
- tests/dry_run.rs:375（`!combined.contains("Clear Diagnostic Reports cache?")`）: 非 running プロンプトの接頭辞を hard-code した否定 pin。新文言に合わせて更新する（「dry-run はプロンプトを出さない」契約自体は不変）。
- tests/dry_run.rs:371 と tests/hints.rs:146（`combined.contains("[dry-run] would")`）: 接頭辞 pin。新文言でも green するが、新 pin をこの近傍に置く場合は assert をパス表示に強化してよい。
- src/hint/cleaner.rs:223-229（doc comment の `Quit ... and clear cache? [y/N]` 引用）: 新文言と同期して更新。
- 無修正で green が pin（触れない）: tests/interactive.rs:107-125（`no_args_without_tty_exits_with_hint`、非 TTY pin）、tests/dry_run.rs:384-403（`scan_y_without_dry_run_deletes_hint_dirs`、control）、src/hint/mod.rs の tests 群（文言を pin しない）、tests/hints.rs の他の E2E（履歴 1 行 / --sub / unknown sub / targets / scan 行）。

### 対象ファイル
src/hint/cleaner.rs（プロンプト + dry-run 2 箇所 + doc comment）、src/main.rs（home を渡す場合のみ :1221）、src/hint/mod.rs（シグネチャ変更に伴う単体テスト 4 箇所。`base_path` を公開する場合は :58）、tests/dry_run.rs、tests/hints.rs（新 pin + pin 置換）。

### 期待する Red → Green
新 pin（パス表示 1 本）が先に Red、実装後に全 Green。既存 E2E と単体テストは pin 置換箇所を除き無修正で green が pin（回帰の検知）。

## pin（壊してはいけない契約）
- PBI-83 の clean hints E2E: tests/hints.rs（`clean_hints_deletes_hint_debris_and_writes_one_history_row` / `clean_hints_sub_cleans_only_that_entry` / `clean_hints_unknown_sub_exits_nonzero` / `targets_lists_hints` / `scan_shows_the_hints_row_*`）が pin 置換箇所を除き無修正で green。quit → 削除 → relaunch の順序（src/hint/mod.rs の RecordingRunner tests）は不変
- PBI-80 の dry-run 契約: dry-run は何も削除せずプロンプトも出さない（tests/dry_run.rs:359-382 の実ファイル生存と否定 prompt pin の契約部分）。offer の dry-run が y を消費しない（src/hint/mod.rs:601-633）
- PBI-87 の quit 除外契約: `quits_running_apps() = true`、unattended 選別（auto / clean all）からの hints 除外は不変（src/hint/mod.rs:640-664 と tests/auto_policy.rs / tests/clean_all.rs）
- KNOWN_ENTRIES 22 件の vetted 契約: src/hint/apps.rs:6 の表（文言・フィールド値・件数）と `HintEntry` 構造体（src/hint/mod.rs:19-33）は触らない。display_name の pin（src/hint/mod.rs tests、no-quit の "Diagnostic Reports" を使う E2E）は無修正で green
- pbi/README.md 設計方針: Outside-In TDD / CommandRunner モック / macOS 専用

## scope fence（絶対禁止）
1. KNOWN_ENTRIES の表変更（文言・フィールド値・件数）・config 化・外部ファイル化はしない（PBI-83 受け入れ基準と PBI-87 fence #6 の継続。round 4 裁定: config 化は見送り）
2. quit / relaunch フロー（`quit_app_and_wait`、`relaunch_app`、`auto_clean_hint` の quit → ledger → relaunch 順序）は変更しない
3. 削除経路・freed 計測（src/trash.rs の `DeleteCtx`、履歴 `freed_bytes`、`CleanResult` の bytes_freed / uses_trash / skipped）は変更しない
4. src/hint/cleaner.rs の `auto_clean_hint` 内 path.exists() は触れない（PBI-89 の管轄。同じファイルの別関数領域）
5. src/hint/detector.rs の top5 truncate と threshold_bytes は触れない
6. CHANGELOG.md は触らない（統合側で一括記載）
7. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: `cleaners::huggingface::tests::clean_fallback_deletes_dir`、`cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries`、`cleaners::pre_commit::tests::clean_fallback_deletes_dir`、`trash::tests::delete_ctx_trash_mode_moves_to_trash`）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] BDD 受け入れシナリオ 3 本が自動テストとして実装され、パスする
- [ ] 文言 pin の置換対象列挙がすべて更新済み、新 pin（パス表示 1 本）が green

## 備考
- 実装は worktree で subagent-driven（AGENTS.md の実装ワークフロー）。PBI-89 と src/hint/cleaner.rs を共有するが別関数領域（本 PBI は `offer_auto_clean` と `KnownAppHintsCleaner::clean` の表示行、PBI-89 は `auto_clean_hint` 内の path.exists()）、マージ順は 89 → 91。
- docs/HOWTO-USE.md:88 と :672 が現行プロンプト文言を引用している（例: `Quit Claude (desktop) and clear cache? (428 MB will be freed) [y/N]`）。実装後に文言が変わるため、例示の更新要否を実装時に判定する（例示の差し替えは本 PBI の表示変更の範囲内とし、docs の更新はマージ側に回してもよい）。
- パスの表示形式（区切り・~ 表記・パスを name の前後どちらに置くか）は Negotiable（実装時に確定してよい）。
