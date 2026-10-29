# PBI: TrashMode の concrete 化と Cleaner::clean トレイト統合

## ユーザーストーリー
sasurahime の保守担当者として、「どのモードで消すか」の解決が main.rs 入口の 1 箇所に集中することがほしい、なぜなら TrashMode::Global（static を読む）と Option::None（同じく static を読む）の 2 表現が約 30 production サイト + TrashModeGuard 48 テストサイトを強制し、テストが完全直列になっているから。

## 優先度
- 順位: 3 / 8
- RICE スコア: 2.06（Reach=3 / Impact=1.5 / Confidence=80% / Effort=1.75 人週）
- 根拠: 実測の並列テスト利得（lib 並列 15.0 秒 vs 直列 35.5 秒、8 コア機、app_support モジュールで lock-bound を実証）。PBI 07「グローバル状態の排除」の未完了部分の完了形。PBI 70（ledger）が共通 call site を先に消してから着手。

## 発見の詳細（深掘り結果）
- production の Global 構築は約 30 サイト: unwrap_or shim 7 + bare clean() の clean_with_mode(Global) 6 + raw safe_delete/delete_entries 14 + main.rs:988/1362 + trash.rs:26。test 側は TrashModeGuard::set 48 サイト（うち trash.rs:595/667 の 2 件は既に過剰）。
- 必須条件: clean_with_opts のデフォルト実装が opts を無視する（cleaner.rs:288-296）ため、CleanOptions の concrete 化だけでは bare clean() 経路（main.rs 14 + run_and_exit 12 + hint/cli_fallback/ollama）に届かない。Cleaner::clean(&self, dry_run, reporter, &CleanOptions) へのトレイト統合が必須。
- static は pin 1 回（main.rs:958）以降読み取り専用で実証済み。引数化で挙動中立。
- TEST_LOCK は消さない: 残存理由は HOME/env/verbose 直列化（約 44 サイト）。
- TrashBackendOverrideGuard が TEST_LOCK を握らない（trash.rs:643）→ guard 内部での lock 取得を同梱。

## BDD受け入れシナリオ
Scenario: mode は main で 1 度だけ解決される
  Given `--permanent` または config.trash_mode=false
  When  任意のサブコマンドを実行する
  Then  解決された TrashMode が 1 値として下ろされる
  And   `grep -rn 'TRASH_MODE|set_trash_mode|is_trash_mode|TrashMode::Global' src/` が 0 件

Scenario: opts を無視するデフォルト実装が消える
  Given Cleaner::clean が clean/clean_with_opts に統合されている
  When  任意のクリーナーを clean する
  Then  すべての impl が opts を受けて使用する（捨てる実装が存在しない）

Scenario: テストが TrashModeGuard なしで並列に走る
  Given 46 テストから TrashModeGuard が削除されている
  When  cargo test を実行する
  Then  trash モード由来の直列化が消え lib の並列実行時間が改善する

## 受け入れ基準
- [ ] main.rs:957 で TrashMode::Trash / Permanent を 1 度構築。src/ に TRASH_MODE / set_trash_mode / is_trash_mode / TrashMode::Global が 0 件
- [ ] Cleaner::clean(&self, dry_run: bool, reporter, opts: &CleanOptions) に統合（clean_with_opts と「opts を無視するデフォルト実装」cleaner.rs:288-296 の削除）
- [ ] CleanOptions.trash_mode: TrashMode（concrete）+ impl Default for TrashMode（= Trash、今日の静的既定 true と一致）
- [ ] 引数化: interactive.rs:286（run_auto/run_interactive）、main.rs:777/783（run_clean_target）、hint 3 署名、cli_fallback.rs:171、ollama.rs:268、clean_by_name
- [ ] test_helpers::TrashModeGuard 削除（48 call sites）。TEST_LOCK は HOME/env/verbose 用に残す（約 44 サイト）
- [ ] TrashBackendOverrideGuard::set() が内部で TEST_LOCK を取得（flake 窓塞ぎ）
- [ ] 外部挙動不変: tests/trash.rs の E2E（yes_permanent_requires_confirmation、config_trash_mode_false_disables_trash ほか）が green
- [ ] 計時の記録: lib 並列/直列 before/after（before 実測 15.0s / 35.5s）
- [ ] cargo test --no-fail-fast（既知 TCC 4 件のみ失敗）/ clippy -D warnings 0 / fmt --check

## テスト戦略
- E2E: 既存 tests/trash.rs の E2E が無修正で green
- 統合: pin テストは明示モードを渡す形に書き直し
- 単体: TrashMode の Default 契約（= Trash）

## 見積もり
1.75 人週（Phase 1: main 解決 + concrete 化 + トレイト統合 0.5、Phase 2: hint/cli_fallback/ollama/batch/interactive 0.5、Phase 3: テスト 48 箇所 0.5-1、検証 0.25）

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（cleaner.rs / trash.rs の doc 契約コメント）
