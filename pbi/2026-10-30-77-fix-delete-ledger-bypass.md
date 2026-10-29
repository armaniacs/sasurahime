# PBI-77: delete_ledger 契約の seam 外漏れを解消する（5 迂回サイトの移行）

出典: /improve-codebase-architecture ラウンド 2 候補 2（深掘り 2026-10-30）

## ユーザーストーリー
sasurahime の保守担当者として、PBI 70 で導入した delete_ledger の deep な契約（fatal → SkippedEntry + 継続、freed は成功のみ加算、uses_trash は成功の OR-in、SkippedEntry 記録、progress tick）が 5 つの迂回サイトでも 1 つの interface の裏で一貫することがほしい。そうすれば、同一の fatal イベントに 3 種の挙動（Err で clean 全体中断、SkippedEntry + 継続、素 warning + 記録なし）が混在する状態が解消され、将来の cleaner 追加時に帳簿契約を手で再組立せず済む。

## 優先度
RICE: Reach 3 × Impact 2 × Confidence 90% / Effort 0.75 人週 = スコア 7.2（本ラウンド 1 位）
根拠: 実害が進行形（project_artifacts.rs は reporter 引数が clean_with_mode に存在せず progress tick が構造的に出ない、uv.rs:106-125 の fatal で `?` が prune 段階まで clean 全体を巻き込む、unsafe path 事前チェック 3 サイトが skip チャネルと exit code から不可視）。同型移行の前例（generic.rs:867-877 の node-gyp 系 dir ループが既に delete_ledger + seam_delete に乗っている）が同一ファイル内に存在するため Confidence が押し上がっている。移行は機械的で、pin 群が契約を固定済み。

## BDD 受け入れシナリオ
Scenario: project-artifacts の削除で progress tick が出る
  Given project-artifacts cleaner が dry_run=false で node_modules を削除する
  When  clean を実行する
  Then  progress tick が出力される（progress_init は items.len()）
  And   reporter は clean_with_mode から delete_entries 呼び出しまで threading される

Scenario: uv の simple-vN 削除で fatal エラー時に clean 全体を中断しない
  Given uv が simple-vN 削除中に 1 エントリで fatal エラーを出す
  When  clean uv を実行する
  Then  残りエントリの削除が継続する
  And   skip が SkippedEntry として記録され、exit 1 は skip 記録経由でのみ出る

Scenario: ollama の fallback 削除で unsafe path が SkippedEntry になる
  Given ollama が未インストールで fallback 削除を実行する
  When  unsafe path が削除対象に現れる
  Then  SkippedEntry として記録される
  And   exit code に反映される

Scenario: cli_fallback の recreate が ledger 後処理で動く
  Given cli_fallback が recreate=true で削除に成功する
  When  clean_cli_or_fallback が完走する
  Then  ディレクトリが再作成される
  And   recreate 失敗が削除済みエントリの skip 記録に化けない

Scenario: 5 サイトのソースに trash::safe_delete が現れない（source pin）
  Given uv、project_artifacts、cli_fallback、generic、ollama の 5 つのソース
  When  include_str! でソースを走査する
  Then  trash::safe_delete が 1 つも現れない

## 受け入れ基準
- [ ] 5 サイト（src/cleaners/uv.rs:106-125、src/cleaners/project_artifacts.rs:163-207、src/cli_fallback.rs:159-198、src/cleaners/generic.rs:568-641、src/cleaners/ollama.rs:216-248）の削除実行部が delete_entries（seam_delete を closure として渡す 1-call 形）に置換され、手書きの分類・freed 加算・SkippedEntry 組立・tick がサイトから消える
- [ ] fatal per-item error 時に 5 サイトすべてが clean 全体を中断しない（SkippedEntry + 継続に統一、exit 1 は skip 記録経由のみ）
- [ ] project_artifacts の progress tick が復活する（reporter を clean_with_mode に threading、progress_init は items.len()）
- [ ] unsafe path 事前チェック 3 サイト（project_artifacts.rs:176-182、cli_fallback.rs:173-176、generic.rs:598-609）を削除し、seam tripwire + ledger の fatal → prose-skip 記録に一本化する（generic.rs:867-877 前例と同一文言）
- [ ] uv の freed が二相会計で二重計上されない（phase 1 = simple-vN の ledger sum、phase 2 = prune 前後の dir_size delta、before 計測は ledger 後）
- [ ] cli_fallback の recreate が ledger 後処理（skipped が空のときのみ fs::create_dir_all）で実行され、closure 内に置かれない
- [ ] generic.rs の PBI 69 TTY ゲート（:591-597）が ledger の前にそのまま維持される
- [ ] trash.rs の safe_delete が pub(crate) に絞られる（可視性 1 語のみ、ledger interface は不変）
- [ ] 既存 pin がすべて無傷で green（PBI 53/59/43/68、trash.rs 3 本、cli_fallback seam-truth、generic fallback trash_mode 3 本）

## テスト戦略
Outside-In TDD。
- E2E: tempdir HOME で `clean project-artifacts` を実行し progress tick の出力を、ollama 未インストール環境で fallback 削除の exit code 反映を、それぞれ stdout と終了コードで検証
- integration: ledger 契約を 5 サイト経由で検証（fatal 継続 + SkippedEntry 記録、freed の二相会計、recreate の ledger 後処理、empty items の zero result）
- unit: include_str! source pin（uv / project_artifacts / cli_fallback / generic / ollama の 5 ファイルに trash::safe_delete が現れないこと。trash.rs:618-642 の NsFileManager pin と同型）、detect_old_indexes 群の既存テストが無傷で通ること

## 見積もり
0.75 人週（0.6 から 1.0）
内訳: project_artifacts 0.2 / uv 0.25 / cli_fallback 0.15 / generic 0.15 / ollama 0.1 / 可視性+ピン 0.1

## 実装計画

### 問題
delete_ledger（src/trash.rs、PBI 70 実装済み）の deep な契約（fatal → SkippedEntry + 継続、freed は成功のみ加算、uses_trash は成功の OR-in、SkippedEntry 記録、progress tick）が 5 サイトで複製され挙動が分裂:
1. src/cleaners/uv.rs:106-125 ― simple-vN 手書きループ、fatal 時 `?` で clean 全体中断（prune 段階まで巻き込む）
2. src/cleaners/project_artifacts.rs:163-207 ― artifact 手書きループ、:185-189 fatal で中断、reporter 引数自体が clean_with_mode に存在しない → progress tick が構造的に出ない、:176-182 unsafe path は素 warning で記録なし
3. src/cli_fallback.rs:159-198 ― 単一アイテム fallback 削除が ledger 契約を手で再組立
4. src/cleaners/generic.rs:568-641 ― 同上（fallback branch、手組 CleanResult）
5. src/cleaners/ollama.rs:216-248 ― clean_fallback、:230 `safe_delete(...)?` で fatal 中断、手書き bookkeeping、tick なし

### 「同一イベントに 3 種の挙動」（fatal per-item error 時）
ledger 経由は SkippedEntry + 継続（trash.rs:290-303）、`safe_delete(...)?` 直呼び 5 サイトは Err で clean 全体中断、unsafe path 事前チェック 3 サイト（project_artifacts.rs:176-182 / cli_fallback.rs:173-176 / generic.rs:598-609）は素 warning + 記録なしで skip チャネルと exit code から不可視。

### 前例（同型移行が同一ファイル内に存在 = Confidence 押し上げ）
generic.rs:867-877 の node-gyp 系 dir ループは既に delete_ledger + seam_delete に乗り、設計方針をコメント記録。

### 解
5 サイトを delete_entries（seam_delete を closure として渡す 1-call 形）に機械的に乗せる。サイト固有の追加挙動は ledger の前後処理として表現:
- uv: freed は二相会計（phase 1 = simple-vN の ledger sum、phase 2 = prune 前後の dir_size delta。before 計測は ledger 後に置いて二重計上なし）。items = (PathBuf, u64) 事前収集 Vec
- project_artifacts: reporter を clean_with_mode に threading、ラベルは "project-artifacts"（self.name()）、items は detect_artifacts_inner(false) の DetectedArtifact{path,size} をそのまま map（(PathBuf, u64) と同型）。unsafe 事前チェックは削除し seam tripwire + ledger の fatal → prose-skip 記録に一本化（generic.rs:867-877 前例と同一文言）
- cli_fallback: clean_cli_or_fallback に reporter 引数追加、fallback 分岐を delete_entries 1 要素に、recreate は ledger 後処理（if config.recreate && r.skipped.is_empty() { fs::create_dir_all(dir)? }）― closure 内は却下（recreate 失敗が削除済みエントリの skip 記録に化ける）
- generic: fallback branch のみ。PBI 69 TTY ゲート（:591-597）は ledger の前にそのまま維持、TOCTOU 事前チェック（:598-609）は削除（同ファイル前例どおり seam+ledger 記録に一本化）、手組 result を delete_entries(self.display_name, ...) 1 行に
- ollama: clean_fallback に reporter 引数、delete_entries 1 要素に置換、手書き bookkeeping 削除
- 可視性: 移行後、safe_delete 直呼びは hint（別 PBI 80）のみになるため trash.rs の safe_delete を pub(crate) に絞る（PBI 80 着地後に private 化）
- ソースピン: include_str! で uv/project_artifacts/cli_fallback/generic/ollama のソースを読み trash::safe_delete が現れないことを固定するテスト（trash.rs:618-642 の NsFileManager pin と同型）

### 実装順（各ステップ独立して green で着地可能）
Step 1 project_artifacts（最重量 cleaner の progress 復活、UX 効果最大）→ Step 2 uv → Step 3 cli_fallback → Step 4 generic → Step 5 ollama → Step 6 可視性 + ソースピン → Step 7 検証ゲート。

## pin（壊してはいけない契約）
- trash.rs の ledger interface（delete_ledger / delete_entries / seam_delete / DeletionOutcome）は一切変更しない ― サイトが seam の中に入るのであり、逆ではない
- trash.rs:756-862 delete_ledger 契約（fatal → prose skip + 継続、reason 記録 + exit 1、freed sum + uses_trash OR-in、empty zero result）、:647-754 delete_entries 4 本、:618-642 NsFileManager source pin
- uv: PBI 53 pin :283-307（prune 失敗 → freed 0、Err にしない）、detect_old_indexes 群 :178-244（シグネチャ・挙動不変、変えるのは caller のみ）
- project_artifacts: :419-450（items の size は detect_artifacts_inner(false) 由来）、:563-630 trash_mode pin、:364-379 dry-run 無削除
- cli_fallback seam-truth pin: :255-304（typed TrashRefused の SkippedEntry、freed 0）、:218-252（unsafe dir 生存）、huggingface.rs:193-194 recreate pin
- generic: :1033-1136 fallback trash_mode 3 本、:1708 colima 非 TTY 拒否（PBI 69、ゲートは ledger 前）、:1739-1768 prompt-port source pin（fallback 部分編集は confirm 分岐に触れないため pin 満たし続ける）、:966-1015 unsafe → SkipCause::Reason 形状
- PBI 59（cleaner.rs:91-130 skip/fatal は変体で判定）、PBI 43（uses_trash は削除成功のみ OR-in）、PBI 68（progress_init は items.len()）
- ollama: remove_model の exit-code チェック（:205-214）と主経路 CLI closure の SkippedReason 契約

## scope fence
1. trash.rs の ledger interface 変更なし（可視性 1 語のみ）
2. detect_old_indexes の挙動変えなし（keep-highest 統合、symlink フィルタの file-as-candidate latent bug（uv.rs:39、テスト :226-236 は空で pass）は別フォローアップ PBI）
3. hint フローには触れない（PBI 80）
4. clean_cli_or_fallback の CLI-first 分岐（:126-157）と ollama 主経路（:147-195）には触れない
5. generic.rs の PBI 69 TTY ゲートの意味・順序不変
6. ProgressReporter の新 API 追加なし
7. ProjectArtifactsCleaner への runner 注入なし
8. uv の prune 失敗意味不変（warn + 非 fatal）
9. library_logs.rs:153 / library_caches.rs:223 の private delete_entries は移行しない（既に委譲済みでリークではない）
10. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] pin テストがすべて green
