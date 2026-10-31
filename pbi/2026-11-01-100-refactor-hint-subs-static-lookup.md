出典: なぜなぜ分析 round 6（.plan/whywhy-round6-freed.md、2026-10-31、main @ 53f0b88）の収束解 (iii) A3 と (iv) docs

# PBI-100: clean hints の subs 筋を KNOWN_ENTRIES の静的 lookup に寄せる

## ユーザーストーリー

hint flow の利用者（CLI offer の受諾、`clean hints --sub`、TUI の per-app 選択）として、明示選択した hint が黙って消えないことがほしい、なぜなら現行の subs 筋は clean で collect_hints（全 22 known dir の走査 + pgrep）を再導出し、offer から clean の間のログ回転・縮小で閾値割れになると selected が空になって何も削除されず履歴行も書かれず、top5 外の known 名を --sub に渡しても同じ silent no-op になるから。

## 優先度

RICE: Reach 2 × Impact 2 × Confidence 80% / Effort 0.5 人週 = スコア 6.4

- Reach 2: hint flow の利用者（CLI offer 筋: run_hint_flow、TUI sub-target 筋: build_selection_items、`clean hints --sub`）。silent no-op は明示選択した利用者全員に届く。
- Impact 2: silent no-op 2 種（閾値割れ / top5 外 known 名）の解消が主価値。offer の y に対する応答が一致し、collect_hints の 1+N 全走査（CLI）と 3 回再導出（TUI）が消える。docs 掲載による発見性も同時に上がる。
- Confidence 80%: is_known_hint_name（src/hint/cleaner.rs:183-187）が既に display_name と KNOWN_ENTRIES の完全一致を検証しており、sub-target validation の入口は実在する。閾値再判定は「明示選択 = 閾値判断済み」で省略でき、top5 truncate は表示の契約（detect / sub_targets）に留める。動作変更（top5 外 known 名の削除）を含むため 80%。
- 前提: なぜなぜ分析 round 6 の収束解 (iii) A3（subs 筋の静的解決）と (iv)（docs 掲載）。

## 背景/問題

1. hint offer 筋が collect_hints（全 22 known dir の走査 + pgrep）を CLI offer で 1+N 回、TUI で 3 回再導出する。offer が収集済みの ProcessHint（paths・sizes・running）を seam ハンドオフで捨てる設計（PBI-83: offer は Vec<&'static HintEntry> を返す、src/hint/cleaner.rs:391-448）の帰結。
2. subs 筋の再導出は silent no-op の源 2 つを持つ:
   - 閾値割れ: offer から clean の間にログが回転・縮小すると、clean の再導出で閾値割れ → selected 空 → 何も削除されず履歴行も書かれない（src/batch.rs:208 の `!dry_run && bytes_freed > 0` ガード、src/main.rs:960-971）。ユーザーの y が黙って消える。
   - top5 外 known 名: is_known_hint_name は KNOWN_ENTRIES 22 全体を検証する（src/main.rs:1083-1098）のに、collect は top5 truncate 済み（src/hint/detector.rs:44）→ top5 外の known 名で `clean hints --sub` しても今日は silent no-op。
3. 加えて collect_hints は進捗を何も出さないため、再導出の全走査（大規模な Application Support/Caches/Logs では 1 回が数秒になりうる）はスピナーの裏で黙って走る。

### 採らない裁定（記録）

なぜなぜ分析 round 6 の裁定:

- **履歴行 N→1（集約行）は採らない**: PBI-83 の per-seam-call 履歴行契約（src/main.rs:1197-1210、label = "hints"、1 cleaner = 1 name）と exit code 契約（PBI 70、run_hint_flow:1237-1239）が per-call 結果を前提とする。集約行は「どの app を消したか」の監査を粗くする。本 PBI の変更後も 1 seam 呼び出し = 1 履歴行は不変。
- **scan_cache::cached_dir_size の採用は不採用**: 再導出の回数問題は subs 筋の静的解決で構造的に消える。cached_dir_size 化は最大 1 時間（src/scan_cache.rs:14）の古さを閾値判定に持ち込み、「縮んだキャッシュが 1 時間 hint に載り続ける偽 offer」という新しい面を新規に作る。detector は format::dir_size 直接を維持する（scope fence に記録）。
- **offer_auto_clean の返却型変更（Vec<&'static HintEntry> → ProcessHint）も採らない**: offer 時点の size/running を seam に渡して信じるより、静的解決 + 削除直前の新鮮 pgrep が正。seam シグネチャの改変はしない。

## 発見の詳細（実コードの確認、main @ 53f0b88）

- subs 筋: `KnownAppHintsCleaner::clean`（src/hint/cleaner.rs:293-348）は `self.collect()`（:299）で collect_hints を再導出し、selected を display_name で交差させる（:300-306）。
- collect() は collect_hints を呼ぶ（:226-228）。detect()（:265-282、:266 で self.collect()）と sub_targets()（:284-291、:287 で self.collect()）も同じ collect() を呼ぶため、TUI は scan → detect（#1）→ build_selection_items（src/interactive.rs:28 → sub_targets、#2）→ clean（#3）の 3 回再導出。
- CLI offer 筋 = 1+N 回: run_hint_flow（src/main.rs:1211-1241）が collect_hints #1（:1219）→ print_hints → offer_auto_clean（:1221）。offer は Vec<&'static HintEntry> を返し（cleaner.rs:391-448、:396）、収集済みの size_bytes / running は捨てられる（accepted.push(hint.entry)、:443）。受諾 1 エントリごとに clean_by_name("hints", Some(display_name))（:1223-1232）→ run_hints_clean（src/main.rs:1075-1107）→ KnownAppHintsCleaner.clean → self.collect() #2..#N+1。
- collect_hints（src/hint/detector.rs:15-45）: KNOWN_ENTRIES（src/hint/apps.rs:9、22 エントリ = 11 Caches + 5 AppSupport + 6 Logs）× path_suffixes を format::dir_size で全走査（:23-27）+ pgrep を process_name ありのエントリ（表内 11 件、skip の GeoServices を除き実走査 10 件）に実行（:31-34）。閾値フィルタ（:28）→ 降順ソート（:43）→ top5 truncate（:44）。
- is_known_hint_name（src/hint/cleaner.rs:183-187）: KNOWN_ENTRIES 全体を走査し `!e.skip && e.display_name == name` で完全一致検証（skip 1 件を除く 21 名が --sub 可能）。run_hints_clean（src/main.rs:1083-1098）がこれを使い、unknown は bail（:1089-1096、PBI 63 形状）。
- 履歴行のガード: src/batch.rs:200-209 の write_history_entry_if_needed が `!dry_run && bytes_freed > 0`（:208）の単一所有者。src/main.rs:960-971 が ride する。selected 空 → bytes_freed 0 → 履歴行も書かれない。
- PBI 93 の削除時実測の哲学: src/cleaners/library_caches.rs:268-280（freed is the delete-time dir_size）+ drift pin（library_caches.rs:820-874）。running の削除直前の新鮮取得はこの哲学と整合する。
- sub_targets pin: src/hint/mod.rs:729-749（sub_targets must list display_name + size, largest first）は不変。
- docs: docs/HOWTO-USE.md に "hints" は 0 件（grep 実測）。`clean <target>` 表（:99-148）に未掲載。挿入位置はアルファベット順で gradle（:118）と huggingface（:119）の間。Tip 節（:93）に top5・64 MB/1 MB 閾値の説明は実在する。
- 既実装確認: subs 筋の静的 lookup は未実装（clean の selected は collect_hints 再導出 + display_name 交差のまま、cleaner.rs:300-306）。本 PBI の機能が既存コードに存在しないことを grep と実コード読みで確認済み。

## BDD 受け入れシナリオ

```gherkin
Scenario: top5 外の known 名の明示選択が削除される
  Given KNOWN_ENTRIES の非 skip 名のうち top5 表示に載っていない名前にしきい値以上の残骸が存在する
  When  `sasurahime clean hints --sub <その名前>` を実行する
  Then  そのエントリの残骸が削除される
  And   quit → 削除 → relaunch の契約と per-seam-call の履歴行契約は従来どおり変わらない

Scenario: 明示選択はしきい値を再判定しない
  Given `clean hints --sub` に渡す名前が KNOWN_ENTRIES の非 skip 名として有効である
  And   その残骸が存在するが現在のサイズはしきい値を下回っている（offer 受諾後の回転・縮小を含む）
  When  `sasurahime clean hints --sub <その名前>` を実行する
  Then  collect_hints の再導出（しきい値判定のやり直し）は行われず、symlink scope 検査を通ったパスがそのまま削除対象になる
  And   履歴行が 1 行書かれる

Scenario: subs 筋の静的 lookup は collect_hints を再導出しない
  Given KNOWN_ENTRIES の非 skip 名で `clean hints --sub` を実行する
  When  clean の subs 筋が走る
  Then  全 known dir の dir_size 走査（collect_hints）は再実行されない
  And   running だけが削除直前に 1 pgrep で新鮮に取られる（quit / relaunch の契約は不変）
```

## 受け入れ基準

- [ ] clean の subs 筋（--sub と TUI の per-app 選択）が display_name → KNOWN_ENTRIES の静的 lookup（完全一致、O(表 22)）になり、collect_hints の再導出と display_name 交差（cleaner.rs:300-306）が消える
- [ ] subs 筋で閾値/top5 の再判定が行われない（明示選択 = 閾値判断済み。top5 は表示の契約 detect / sub_targets に留まる）
- [ ] running だけが削除直前に 1 pgrep で新鮮に取られる（process_name なしのエントリは pgrep なしで running = false。quit / relaunch の契約は不変）
- [ ] top5 外の known 名で `clean hints --sub` が削除される（silent no-op の解消）
- [ ] collect_hints が run あたり CLI 1 回（run_hint_flow の offer 表示分）/ TUI 2 回（detect + sub_targets）になる
- [ ] 履歴行は per-seam-call のまま（1 seam 呼び出し = 1 履歴行、label = "hints"。N→1 集約行は書かない）
- [ ] docs/HOWTO-USE.md の `clean <target>` 表に hints 行が 1 行追加される（gradle と huggingface の間。SHIFT-JIS 可能文字のみ、em dash 不使用）
- [ ] 新 pin 2 本: subs 筋がディレクトリ walk をしないこと（pgrep のみのモックで pin）と top5 外 known 名が削除されること
- [ ] DoD の 3 コマンドが通る

## テスト戦略（Outside-In TDD）

- E2E Red（1 本、top5 外 known 名）: tests/hints.rs に 1 本。tempdir fixture で log 系エントリ 6 件にしきい値（1 MB）超過の debris を作り、選択対象（no-quit・no-process の "Diagnostic Reports"、~/Library/Logs/DiagnosticReports、既存ヘルパ make_diagnostic_reports tests/hints.rs:46）を 1.5 MB（1_572_864 バイト、4096 の倍数で dir_size 測定が確実）、他 5 件を 2 MB（既存 DEBRIS_BYTES tests/hints.rs:42 と同形状）にする → 降順ソート + truncate(5) で選択対象が top5 外になる。現行実装では selected 空 → silent no-op（debris 生存、exit 0）を Red で検知する。非選択 5 件の running 判定（pgrep）は結果が選択対象の clean に影響しないため決定論は保たれる（PATH は /usr/bin:/bin、quit 筋は選択対象にしか fire しない）。
- E2E Red（1 本、閾値割れ）: tests/hints.rs にもう 1 本。tempdir で "Diagnostic Reports"（閾値 1 MB）にしきい値未満の debris（例: 4096 バイト 1 ファイル）を作り、`clean hints --sub "Diagnostic Reports"` を実行する。現行実装では collect_hints の閾値フィルタ（src/hint/detector.rs:28）で selected 空 → silent no-op（debris 生存）を Red で検知する。修正後は symlink scope 検査を通ったパスがそのまま削除され、履歴行が 1 行書かれる。
- 統合 pin（pgrep のみのモック）: src/hint/mod.rs の test factory + CommandRunner モックで、subs 筋の clean が collect_hints を再導出しないことを pin する。process_name ありの閾値超過エントリ 2 件を作り subs = Some(1 件) で clean すると、走る pgrep は削除直前の 1 回のみ（現行は collect_hints 分の 2 回 + 削除時）。選択対象が process_name なしのとき pgrep が 0 回であることも同時に pin できる（whywhy doc (iii) の pin 更新）。
- E2E（回帰の pin）: tests/hints.rs の既存 8 本（clean_hints_deletes_hint_debris_and_writes_one_history_row :63 / clean_hints_sub_cleans_only_that_entry :101 / clean_hints_dry_run_deletes_nothing_and_lists_the_plan :132 / clean_hints_unknown_sub_exits_nonzero :156 / targets_lists_hints :178 / scan_shows_the_hints_row_* :205 と :223 / clean_hints_refuses_to_delete_through_an_intermediate_symlink :254）と tests/dry_run.rs、src/hint/mod.rs の tests 群（sub_targets pin :729-749 を含む）が無修正で green し続けること。
- モック: 外部コマンドは CommandRunner 注入（tests/AGENTS.md の設計方針）。running の新鮮取得は pgrep 1 回なのでモック契約は単純。

## 実装計画

### 実装順

1. E2E Red: 新 pin 2 本（top5 外 known 名が削除される / しきい値未満 debris が --sub で削除される）を tests/hints.rs に追加し、現行実装で落ちることを確認する。
2. Green: src/hint/cleaner.rs の subs 筋（:300-306）を静的 lookup に差し替える。subs = Some のとき display_name → KNOWN_ENTRIES 完全一致で entry を解決し、selected を collect_hints 再導出なしで組む。閾値/top5 の再判定は行わない。
3. Green: running だけを削除直前に 1 pgrep で新鮮に取る（process_name なしのエントリは pgrep なしで running = false）。削除 seam（auto_clean_hint の symlink scope 検査 → dir_size → delete_entries）は 1 行も変わらない。
4. 統合 pin: src/hint/mod.rs に pgrep のみのモックで「subs 筋が walk しない」pin を 1 本足す。
5. docs: docs/HOWTO-USE.md の `clean <target>` 表（:118 と :119 の間）に hints 行を 1 行追加する。
6. 検証ゲート（DoD の 3 コマンド + charset チェック）。

### 静的 lookup の形状（収束解 (iii)）

- subs = Some の clean は「明示選択 = 閾値判断済み」を契約とする。閾値の再判定（src/hint/detector.rs:28 の述語の再実行）と top5 truncate（detector.rs:44）は subs 筋から外す。top5 は表示の契約（detect / sub_targets、TUI 選択肢）に留まる。
- 解決は `KNOWN_ENTRIES.iter().find(|e| !e.skip && e.display_name == name)` の O(表 22) 完全一致で、is_known_hint_name（cleaner.rs:183-187）と同じ述語を使い 1 つの所有者に寄せる（述語の重複は作らない）。
- running は削除直前に新鮮に取る: 選択対象の process_name があるときだけ pgrep -x を 1 回（PBI 93 の削除時実測の哲学と整合。quit 判断は「今走っているか」で決まる）。offer 時点の running は信じない。
- subs = None の素の `clean hints` は現行契約（閾値以上の全 hint、top5 truncate）を維持する（fence: top5/閾値の変更はしない）。
- 効果: collect_hints は run あたり CLI 1 回（run_hint_flow の offer 表示分）/ TUI 2 回（detect + sub_targets）。1+N 全走査と silent no-op 2 種が消える。run_hint_flow（main.rs）と run_hints_clean（main.rs）のシグネチャと契約は無修正。

### 対象ファイル

src/hint/cleaner.rs（subs 筋 :300-306 付近と running の新鮮取得。プロンプト/dry-run 文言は触らない）、tests/hints.rs（新 pin 2 本）、src/hint/mod.rs（統合 pin 1 本）、docs/HOWTO-USE.md（表 1 行）。

### 追加する docs 行（1 行）

docs/HOWTO-USE.md の `clean <target>` 表（:118 gradle と :119 huggingface の間、アルファベット順）:

```markdown
| `hints`         | Known app hints: quit + delete + relaunch for the top 5 above-threshold caches (per-app via `--sub`) |
```

詳細（top5・64 MB/1 MB 閾値）は Tip 節（:93）に既にあるので 1 行で足りる。Examples（:150-158）はそのまま。SHIFT-JIS 可能文字のみ（上記 1 行は ASCII のみ）、em dash（U+2014）不使用。

### 期待する Red → Green

新 pin 2 本が先に Red（現行実装では top5 外 known 名としきい値未満 debris の明示選択が silent no-op になる）、実装後に全 Green。既存 E2E と単体テストは無修正で green が pin（回帰の検知）。

## pin（壊してはいけない契約）

- PBI-83 の per-seam-call 履歴行契約: src/main.rs:1197-1210（各 seam call が自履歴行を書く、label = "hints"、1 cleaner = 1 name、集約行なし）と clean_hints_deletes_hint_debris_and_writes_one_history_row（tests/hints.rs:63-98）は無修正で green
- PBI 70 の exit code 契約: run_hint_flow（src/main.rs:1237-1239）と all-skipped 経路は無修正で green
- PBI 91 の hint prompt/dry-run 文言 pin: src/hint/cleaner.rs の offer プロンプト（:431-439 付近）と dry-run 行（cleaner 側 :313-330、offer 側 :408-425）、tests/dry_run.rs と tests/hints.rs の文言 pin は無修正で green（本 PBI は同じファイルの subs 筋のみ触る）
- PBI 87 の quit 除外契約: quits_running_apps() = true、unattended 選別（auto / clean all）からの hints 除外は不変
- PBI 63 形状の unknown sub-target 契約: clean_hints_unknown_sub_exits_nonzero（tests/hints.rs:156-176）と is_known_hint_name（cleaner.rs:183-187）の bail は無修正で green
- KNOWN_ENTRIES 22 件の vetted 契約: src/hint/apps.rs:9 の表（文言・フィールド値・件数）と HintEntry 構造体（src/hint/mod.rs:19-33）は触らない
- sub_targets pin: src/hint/mod.rs:729-749（display_name + size、largest first）は無修正で green
- PBI 93 の契約（library_caches / app_support の削除時実測 + drift pin、library_caches.rs:268-280 と :820-874）は無修正で green
- symlink scope 検査の契約: clean_hints_refuses_to_delete_through_an_intermediate_symlink（tests/hints.rs:254）と PBI 89 の per-component 検査（cleaner.rs:139-151）は無修正で green（静的解決後も削除 seam の安全契約は不変）
- pbi/README.md 設計方針: Outside-In TDD / CommandRunner モック / macOS 専用

## scope fence（絶対禁止）

1. offer_auto_clean の返却型変更（Vec<&'static HintEntry> → ProcessHint など）はしない（seam シグネチャの改変禁止）
2. 履歴行 N→1（集約行）はしない（PBI-83 per-seam-call 契約 + PBI 70 exit code 契約が per-call 結果を前提）
3. scan_cache::cached_dir_size の採用（detector.rs の cached_dir_size 化）はしない（1 時間の閾値の古さによる偽 offer という新しい面を作る。detector は format::dir_size 直接を維持）
4. KNOWN_ENTRIES の表統合・config 化・外部ファイル化はしない（PBI-83 受け入れ基準と round 4 裁定の継続）
5. top5 / 閾値の変更はしない（src/hint/detector.rs:28 と :44 は触らない。top5 は表示の契約に留まる）
6. src/explorer.rs は変更しない
7. src/hint/cleaner.rs のプロンプト/dry-run 文言は変更しない（PBI 91 着地済み。同一ファイルの subs 筋のみ）
8. PBI 93 の既存 pin（library_caches / app_support）は変更しない
9. TUI build_selection_items（src/interactive.rs）への接触はしない
10. CHANGELOG.md は触らない（統合側で一括記載）
11. Linux / Windows 対応なし（AGENTS.md の安全規則）

## DoD

- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 履歴行 per-call 契約 pin（PBI-83 / PBI 70）が無修正で green
- [ ] PBI 91 の hint prompt/dry-run 文言 pin が無修正で green
- [ ] 新 pin 2 本（top5 外 known 名の削除 / しきい値再判定なしの削除）+ 統合 pin 1 本が green
- [ ] docs 行追加の charset チェック（docs-charset スキルの check_docs_chars.py）が 0

## 備考

- 実装は worktree で subagent-driven（AGENTS.md の実装ワークフロー）。
- 並行で src/prompt.rs と src/hint/cleaner.rs 6 箇所（PBI 99）、src/cleaners/library_logs.rs と orphan_containers.rs（PBI 97）、src/scanner.rs（PBI 96/98）、src/progress.rs と cleaner.rs（PBI 95）を触る別 PBI が進行中。マージ順とコンフリクト解消は統合側で判断する。
- 静的 lookup の実装形状（述語を is_known_hint_name と共有するか、selected の ProcessHint 再構築をどう組むか）は Negotiable（実装時に確定してよい）。削除 seam（symlink scope 検査、uchg、skip/fatal 分類）は 1 行も変わらない。
- 形式の前例: .plan/archived/2026-10-31-91-feat-hint-path-display.md（PBI 91、アーカイブ済み）。
