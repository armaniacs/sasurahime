出典: /improve-codebase-architecture ラウンド 2 候補 1（深掘り 2026-10-30）

# PBI-80: hint 経路に dry_run と履歴を届ける（twin 統合 + ledger 移行）

## ユーザーストーリー
sasurahime を TTY で操作する開発者として、`--dry-run` を付けて hint プロンプトに y と答えても実削除されず、hint で削った容量が履歴と stats に反映されることがほしい、なぜなら dry-run は「絶対に消さない」という最上位の安全契約であり、hint 経路だけがその契約と履歴記録の外にいると CLI 全体の信頼を損なうから。

## 優先度
- RICE: Reach 2 × Impact 2.5 × Confidence 80% / Effort 1.25 人週 = スコア 3.2（本ラウンド 4 位）
- 根拠: dry_run が届かない実害は TTY に限られる。非 TTY は run_interactive が exit 1 で hint に到達せず（tests/interactive.rs:108 で pin）、scan --json は early return する。Reach 2 はこの限定込みの読み
- 根拠: ただし dry-run は安全契約の中核で、TTY でも契約破れが起きる現状は CLI 全体の信頼を損なう。加えて hint で削った bytes が履歴に書かれないため、`sasurahime stats` の計測が永続的に歪む
- 根拠: 削除ループの twin 重複と y 判定 3 実装はドリフトの温床で、PBI 70 / 74 / 75 が範囲外として明記した文書化された経路に蓄積している

## 発見の詳細（深掘り結果）
問題: hint 経路（src/hint/cleaner.rs）は Cleaner trait / run_batch / 履歴ガードの interface の裏に隠れるべき複雑さを表面に再実装した shallow な第 2 エンジン。
- dry_run が届かない（バグ形状）: offer_auto_clean のシグネチャに dry_run がない（hint/cleaner.rs:158-164）。--dry-run は global=true（main.rs:355-356）なのに main.rs:1082 / 1117 は trash_mode しか渡さない → TTY で `sasurahime --dry-run` を実行し hint プロンプトに y と答えると実削除される。実害は TTY 限定（非 TTY は run_interactive が exit 1 で hint に到達しない、tests/interactive.rs:108 で pin、scan --json は early return）
- 履歴が書かれない: 履歴ガードは batch.rs:109-119（write_history_entry_if_needed）に統一済みだが、hint 経路は run_batch / run_clean_target を通らない → hint で削った bytes は stats に出てこない
- 削除ループの同型重複: auto_clean_hint（hint/cleaner.rs:96-123）と clean_hint_dirs（:127-151）は quit/relaunch の括り以外ほぼ同一の twin
- y/N プロンプトの第 3 実装: hint/cleaner.rs:187-188（input.trim().eq_ignore_ascii_case("y")）≒ interactive.rs:107-117（ask_yn）≒ prompt.rs:109-124（PromptPort::confirm）
- hint の E2E pin が tests/ に存在しない（lib テストのみ）
- PBI 70 が hint を「範囲外: hint/cleaner.rs の 2 ループ」と明記済み（.plan/archived/2026-10-29-70:42）。PBI 74 / 75 も範囲外で、ドリフト蓄積の文書化された経路になっている

5 Whys の根本原因: offer_auto_clean が「提示（tip 表示）」と「実行（quit → delete → relaunch）」を 1 関数に抱えた shallow 設計で、registry pin（1 cleaner = 1 name）と dry_run 契約が構造的に適用されなかった。解 = 実行部を CleanByName seam に寄せ、offer_auto_clean は選択 UI に縮退、dry_run / 履歴の契約を trait フローの既存所有者に返す（実行部の Cleaner 化は Phase C、別 PBI 83）。

## BDD 受け入れシナリオ

Scenario: TTY で --dry-run を付けて hint プロンプトに y と答える
  Given KNOWN_ENTRIES に一致する hint ディレクトリが存在する
  And  該当 app は running でない
  When `sasurahime --dry-run` を実行し hint プロンプトに y と答える
  Then 実削除は行われない
  And  dry-run 行のみが出力される
  And  osascript / rm / chflags / open が呼び出されない

Scenario: dry_run=true では hint プロンプト自体が出ない
  Given KNOWN_ENTRIES に一致する hint ディレクトリが存在する
  When `sasurahime --dry-run` を実行する
  Then offer_auto_clean はプロンプトを出さず dry-run 行のみで return する
  And  実ファイルは 1 つも変わらない

Scenario: hint で削除が完了する
  Given KNOWN_ENTRIES に一致する hint ディレクトリが存在する
  And  該当 app は running でない
  When `sasurahime` を実行し hint プロンプトに y と答える
  Then 削除した bytes が履歴に記録される
  And  `sasurahime stats` に hint の削除分が反映される

Scenario: hint の削除中に 1 エントリで fatal エラーが出る
  Given 削除対象の 1 エントリが削除に失敗する（権限エラーなど）
  When hint プロンプトに y と答えて削除を実行する
  Then clean 全体は中断せず残りを続行する
  And  失敗エントリは skip として記録される
  And  全エントリが skip されたときは exit 1 で終了する

Scenario: app が running で quit 設定がある
  Given 該当 app が running である
  And   KNOWN_ENTRIES の該当エントリに quit_app が設定されている
  When  hint プロンプトに y と答える
  Then  削除前に quit_app_and_wait が呼ばれる（既存 pin 維持）

Scenario: hint プロンプトに大文字 Y や前後空白付きの入力を答える
  Given hint プロンプトが表示されている
  When  ` Y ` と答える
  Then  is_yes_answer が yes と判定し削除が実行される
  And   interactive.rs の ask_yn と同一の判定関数を共有している

## 受け入れ基準
- [ ] offer_auto_clean が dry_run: bool を受け取り、dry_run=true ではプロンプトを出さず dry-run 行のみで return する
- [ ] TTY で `sasurahime --dry-run` + hint プロンプト y でも実削除されない（osascript / rm / chflags / open 不呼び出し）
- [ ] auto_clean_hint と clean_hint_dirs が 1 関数に統合され、quit 分岐は running && quit_app ありのみで発火する（completion_line と print_hints は維持）
- [ ] hint/cleaner.rs:187-188 の y 判定が pub(crate) fn is_yes_answer(&str) -> bool（interactive.rs の ask_yn 判定行 :116 を共有）に置換される
- [ ] main.rs:1082 / 1117 の 2 呼び出しが 1 つのヘルパ（run_hint_flow）に統合され dry_run = cli.dry_run を渡す
- [ ] hint の safe_delete 直ループ（hint/cleaner.rs:108-119）が trash.rs の delete_entries / delete_ledger（PBI 70 契約）に寄せられ CleanResult を返す。items = (base.join(suffix), ProcessHint.size_bytes)。fatal は prose skip + 継続、all-skipped は exit 1、dry-run は ledger を通らない
- [ ] run_hint_flow が CleanResult を write_history_entry_if_needed に渡し、label = "hints"（1 cleaner = 1 name）で履歴が記録される。zero-byte は batch.rs:116 が記録しないので quit タイムアウトも安全
- [ ] hint/mod.rs に dry_run pin を追加: RecordingRunner に osascript / rm / chflags / open が現れない
- [ ] tests/dry_run.rs に E2E pin を新設: --dry-run + y で実削除されない（hint の E2E pin が tests/ に存在しないため新設が必要）
- [ ] hint/mod.rs lib テスト 8 本、completion_line 契約 3 pin、tests/interactive.rs:108、tests/scan.rs が無修正で green
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
Outside-In TDD。E2E → 統合 → 単体の順に内側へ書き、各レイヤーで赤を確認してから実装する。

- E2E: tests/dry_run.rs に新設。tempdir を HOME にして binary を `--dry-run` 付きで spawn、hint プロンプトに y を答えて実削除されないことを exit code と stdout で assert（hint の E2E pin が tests/ に存在しないため新設が必要）
- 統合: hint/mod.rs の RecordingRunner に osascript / rm / chflags / open が現れない dry_run pin。FakePrompt 注入で offer_auto_clean の dry_run 挙動を pin。run_hint_flow が CleanResult を write_history_entry_if_needed に渡す pin
- 単体: is_yes_answer の判定表（"y" / "Y" / 前後空白付き / "n" / 空文字）
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md:85 の設計方針どおり）

## 見積もり
1.25 人週（1.0 から 1.5。Phase A 0.75 から 1.0 + Phase B 0.5 から 0.75 を統合）

内訳: Phase A（dry_run + twin 統合 + プロンプト統一）と Phase B（ledger 寄せ + 履歴統合）の合算。tests/dry_run.rs の E2E pin 新設を含む。

## 実装計画

### Phase A（fix: dry_run + twin 統合 + プロンプト統一、最優先）
1. hint/cleaner.rs: offer_auto_clean に dry_run: bool を追加。dry_run=true はプロンプトを出さず dry-run 行のみで return
2. hint/cleaner.rs: auto_clean_hint と clean_hint_dirs を 1 関数に統合。running で quit 分岐（quit_app_and_wait は running && quit_app ありのみ）。completion_line（:207-213）・print_hints は維持
3. hint/cleaner.rs:187-188 の y 判定を interactive.rs から切り出した pub(crate) fn is_yes_answer(&str) -> bool（ask_yn の判定行 :116 を共有）に置換。FakePrompt 注入テストと ask_yn の pin が両方生存
4. main.rs:1082 / 1117 → 1 つのヘルパ（run_hint_flow）に統合し dry_run = cli.dry_run を渡す

### Phase B（refactor: ledger + 履歴統合）
1. hint の safe_delete 直ループ（:108-119）を trash.rs の delete_entries / delete_ledger（PBI 70 契約）に寄せ、CleanResult を返す。items = (base.join(suffix), ProcessHint.size_bytes)。fatal は prose skip + 継続、all-skipped exit 1、dry-run は ledger を通らない
2. main.rs の run_hint_flow で CleanResult を受け write_history_entry_if_needed に渡す。label = "hints"（1 cleaner = 1 name）。zero-byte は batch.rs:116 が記録しないので quit タイムアウトも安全
3. 出力変更（`  [OK]` / `[SKIPPED]` → `[hints] removed: ...`）は既存利用者の期待に触れるため、文言維持派の場合は Phase B を「履歴のみ」に縮小できる。代替案: 出力文言は `  [OK]` / `[SKIPPED]` を維持し、run_hint_flow が削除済みエントリの bytes を集計して write_history_entry_if_needed に渡す（出力変更は別 PBI に分離、completion_line 契約 3 pin はそのまま生存）。CHANGELOG は本体で一括記載
4. safe_delete の private 化（PBI 77 が pub(crate) にしたもの）はこの PBI の完了後

### Phase C（参考: 1 cleaner 化 + registry 登録。別 PBI 83 として後続）
KnownAppHintsCleaner（name()="hints"、detect()=collect_hints 集約、clean()=quit → ledger → relaunch、sub_targets()=display_name + size、risk()=Moderate、interactive_gated()=false）+ define_cleaners! 登録 + tests/coverage.rs 自動対応。

### 実装順
Phase A (1) → (4) → テスト（hint/mod.rs に dry_run pin: RecordingRunner に osascript / rm / chflags / open が現れない。tests/dry_run.rs に E2E pin: --dry-run + y で実削除されない。hint の E2E pin が tests/ に存在しないため新設が必要）→ Phase B (1) → (4) → 検証ゲート

## pin（壊してはいけない契約）
- hint/mod.rs lib テスト 8 本: auto_clean_skips_if_quit_times_out（osascript あり rm なし）、auto_clean_deletes_paths_after_quit（chflags 3 回 pin + 実削除）、auto_clean_relaunches_when_configured、auto_clean_skips_relaunch_for_slack、offer_auto_clean_calls/skips_auto_clean_when_user_answers_y/n、offer_auto_clean_prompts_for_non_running_hints、quit_app_and_wait_bails_when_osascript_exits_nonzero（PBI 53）、quit_app_and_wait_escapes_quotes_in_quit_app（AppleScript インジェクション）
- completion_line 契約: skip ありで [OK] を出さない（hint/cleaner.rs:253-279 の 3 pin）。文言が変わる場合は pin を置換し CHANGELOG に明記
- KNOWN_ENTRIES 22 件の文言: display_name は hint/mod.rs tests が pin（"Claude logs"/"Zoom logs"/"VSCode caches"/"GeoServices"）。quit_app_script_leaves_known_entries_untouched は "Visual Studio Code"/"Slack"
- explorer.rs: MANAGED_PATTERNS 18 件（explorer.rs:17-90）と main.rs:2216 の PBI メタデータ参照は触らない
- main.rs 呼び出し側: tests/interactive.rs:108（非 TTY exit 1）と tests/scan.rs が無修正で green
- PBI 契約: PBI 53（osascript 失敗 bail）、65（exit-code 注入）、28（safe_delete scope tripwire「refusing to delete unsafe path」）、70（ledger 契約: fatal prose skip + 継続、all-skipped exit 1、dry-run は ledger を通らない）、71（trash_mode は引数で運ぶ、Default=Trash）、73（report_batch_failures は BatchSummary）、74（UnattendedPolicy は 3 のまま）、75（PromptPort 7 gate + NonInteractive 契約、hint は port にしない）
- pbi/README.md:81-87 設計方針（不変）: dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用

## scope fence
1. hint の app ごと 22 cleaners 化・個別 CleanTarget 登録はしない（Phase C は別 PBI 83）
2. UnattendedPolicy への hint 統合はしない
3. hint プロンプトの PromptPort 置換はしない（TtyPrompt.confirm は reporter.suspend_live_output を要求するが hint 経路に ProgressReporter がない。is_yes_answer 共有で統一が最小差分）
4. KNOWN_ENTRIES の config 化・外部ファイル化はしない
5. hint の top5 truncate（detector.rs:44）と threshold_bytes（64MB/1MB）は触れない
6. explorer.rs / MANAGED_PATTERNS 18 件は触れない
7. quit/relaunch の osascript/open 以外の実装（SIGTERM 等）はしない（PBI 53 pin）
8. hint 経路と app_support/library_caches cleaner の機能重複（同じ Slack/Cache・Code/Cache に 2 削除経路）の解消はしない（別 PBI）
9. Linux/Windows 対応はしない
10. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
