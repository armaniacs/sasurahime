# PBI-84: pre_commit テストの guard なし 1 本を TEST_LOCK + EnvGuard で隔離する（flake 衛生）

出典: 前ラウンドの深掘りで記録された持ち越し（pre_commit テストの flake 対策 = テスト衛生、known flake 候補、2026-10-30）

## ユーザーストーリー
sasurahime の開発者と CI として、cargo test の並列実行が同じ場所で同じ失敗を繰り返さないことがほしい、なぜなら flake は再実行で消えるため、赤の信頼性が失われ、本物の退行の検知が鈍り、CI の再実行コストと失敗の見逃しリスクの両方が上がるから。

## 優先度
RICE: Reach 3 × Impact 2 × Confidence 95% / Effort 0.5 人週 = スコア 11.4（本ラウンド 1 位 / 4 件）
- Reach 3: 並列実行される全テスト実行（開発者の cargo test、CI の cargo test）
- Impact 2: flake ノイズで赤の信頼が低下するが、ユーザー影響はゼロ（本番コードに触れない）
- Confidence 95%: flake 機構が実コードで確認できる（detect() が closure 経由で process env を解決時に読む、guard なし 1 本だけが TEST_LOCK の隔離から外れる）。修正は兄弟 4 本と同一の 3 行パターン
- Effort 0.5 人週: 実変更は既存 1 本への前処理追加のみ。残りは Red 再現と高並列検証

## BDD 受け入れシナリオ

Scenario: 並列実行中に兄弟テストの tempdir を拾わない
  Given テストが複数スレッドで並列実行されている
  And  detect_returns_pruneable_when_cache_exists が PRE_COMMIT_HOME に自分の tempdir キャッシュ（hook.pck 入り）を設定した guard 生存中である
  When detect_returns_not_found_when_dir_missing が detect() を呼ぶ
  Then detect() は兄弟の tempdir キャッシュを解決しない（TEST_LOCK が兄弟の guard 生存期間と重ならない）
  And  結果は NotFound でテストは green で完了する

Scenario: シェルで環境変数を設定した単独実行でも NotFound を返す
  Given PRE_COMMIT_HOME に hook.pck を含む実在ディレクトリ（macOS の tempdir）が設定されている
  When cargo test detect_returns_not_found_when_dir_missing を単独実行する
  Then テストは実行前に guard が 2 変数を unset するため PRE_COMMIT_HOME を参照しない
  And  既定パス（tempdir HOME の .cache/pre-commit）が存在しないため NotFound を返し green で完了する

Scenario: テスト後に環境変数が復元される
  Given detect_returns_not_found_when_dir_missing が TEST_LOCK を取得し EnvGuard で PRE_COMMIT_HOME と XDG_CACHE_HOME を unset した
  When テストが完了し guard の drop が走る
  Then 2 変数はテスト前の値に復元される
  And  後続の並列テストは汚染された環境変数を受け取らない

## 受け入れ基準
- [ ] detect_returns_not_found_when_dir_missing が冒頭で crate::context::TEST_LOCK を取得する（兄弟 4 本と同一の .lock().unwrap_or_else(|p| p.into_inner()) パターン）
- [ ] 同テストが EnvGuard::unset("PRE_COMMIT_HOME") と EnvGuard::unset("XDG_CACHE_HOME") を取り、guard をテスト末尾まで生存させる
- [ ] PRE_COMMIT_HOME を hook.pck 入りの実在 tempdir に設定した状態での単独実行（cargo test detect_returns_not_found_when_dir_missing）が green になる（Red 再現コマンドの反転）
- [ ] cargo test pre_commit -- --test-threads 8 を複数回実行して 1 度も落ちない
- [ ] 兄弟 4 本の guard ありテストの名前・設定値・期待値が無傷
- [ ] 本番コード（PreCommitCleaner::new、detect / clean 委譲、cli_fallback.rs、safety.rs、test_helpers.rs）の差分ゼロ
- [ ] cargo clippy --tests -- -D warnings と cargo fmt --check が通る

## テスト戦略
Outside-In TDD。ただし本 PBI はテスト衛生の修正で、変更レイヤーは単体テストのみ。
- E2E: 変更対象外。CLI 振る舞いを変えないため tests/ の統合・E2E には触れない
- 統合: 変更対象外。Cleaner trait / delete ledger / batch に接触しない
- 単体: 唯一の変更レイヤー。src/cleaners/pre_commit.rs の tests モジュール内の既存 1 本（detect_returns_not_found_when_dir_missing）に TEST_LOCK + EnvGuard を追加するのみ。新規テストは追加しない（テスト名を増やさず、既存 1 本の隔離を強化する）
- モック: 追加不要。detect() は fs と env のみを読み、CommandRunner を呼ばない
- 検証方法: Red 再現（環境変数設定下の単独実行で failed を確認）→ guard 追加 → 同一コマンドが green に反転 → 高並列・複数回実行で安定

## 見積もり
ストーリーポイント 1（0.5 人週）
内訳: Red 再現と高並列検証 0.3 / 実変更（TEST_LOCK + 2 guard の追加）0.1 / 検証ゲート（clippy、fmt、全体テスト）0.1

## 実装計画

### 問題（実コードで確認した flake 機構）
1. PreCommitCleaner::new（src/cleaners/pre_commit.rs:14-43）が cache-dir 解決 closure を持つ。closure は呼ばれるたびに std::env::var("PRE_COMMIT_HOME") を読む（:26-39）。変数があれば resolve_env_dir（src/safety.rs:99-129）に渡し、なければ XDG_CACHE_HOME 分岐に落ちる。resolve_env_dir は変数不在を default に返す（safety.rs:127）。
2. detect() は委譲先 CliFallbackCleaner::detect（src/cli_fallback.rs:70-88）で cache_dir() → closure を呼び、解決ディレクトリの存在と dir_size で NotFound / Clean / Pruneable(bytes) を決める。つまり結果は process env の瞬間値で決まる。
3. detect_returns_not_found_when_dir_missing（pre_commit.rs:146-152）だけが TEST_LOCK も EnvGuard も持たない。cargo test は既定で並列実行のため、guard あり兄弟が TEST_LOCK を握っていても本テストはロックを無視して同じ瞬間に走る。detect_returns_pruneable_when_cache_exists の guard 生存中、PRE_COMMIT_HOME は兄弟の tempdir キャッシュ（hook.pck 4096 バイト入り）を指す。macOS の tempdir（/var/folders/...）は is_safe_delete_target で許可される（safety.rs:47-48）ため resolver は兄弟のキャッシュをそのまま返し、本テストの detect() は dir_size > 0 → Pruneable を返して assert!(matches!(result.status, ScanStatus::NotFound)) が落ちる。
4. EnvGuard（src/test_helpers.rs:141-169）自体はロックを取らない。set / unset が前値を保存し Drop で復元するのみ。隔離にはテスト側が先に TEST_LOCK を取る必要がある（src/trash.rs:80-93 の set_trash_backend_override はロック込みの別実装なので混同しない）。
5. 他 cleaner の同種テストはパターン済み: huggingface.rs の cache_dir 系 4 本はすべて TEST_LOCK + EnvGuard（src/cleaners/huggingface.rs:237、:263、:291、:321）。

### guard ありテストとの差分（specific）
- detect_returns_pruneable_when_cache_exists（:155-171）: TEST_LOCK（:156-158）→ EnvGuard::set("PRE_COMMIT_HOME", cache)（:165）。guard はテスト末尾まで生存。
- cache_dir_uses_safe_default_when_pre_commit_home_is_unsafe（:265-285）: TEST_LOCK（:266-268）+ EnvGuard::set("PRE_COMMIT_HOME", "/")（:271）。
- cache_dir_uses_safe_default_when_xdg_cache_is_unsafe（:288-321）: TEST_LOCK（:289-291）+ EnvGuard::set 2 変数（:294-295、空 PRE_COMMIT_HOME + XDG_CACHE_HOME=/etc）。
- detect_uses_xdg_cache_home_unconditionally（:324-341）: TEST_LOCK（:325-327）+ EnvGuard::set 2 変数（:331-332）。
- detect_returns_not_found_when_dir_missing（:146-152）: どちらもなし。TempDir 作成 → detect() → assert のみ。プロセス全体の環境変数の瞬間値に結果が依存する。
- 同一ファイルの clean_* 5 本（:174、:193、:212、:227、:246）も TEST_LOCK + EnvGuard がない。断言の多くは汚染耐性があるが、clean_fallback_deletes_dir（:193）は解決ディレクトリが汚染で入れ替わると !cache.exists() が落ち得る（潜伏 flake 候補。観測記録は前ラウンドにないため本 PBI の範囲外、別 PBI 提案）。

### 解
detect_returns_not_found_when_dir_missing の冒頭に、兄弟 4 本と同一のロック取得と EnvGuard::unset 2 行を追加する。

```rust
#[test]
fn detect_returns_not_found_when_dir_missing() {
    let _lock = crate::context::TEST_LOCK
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let _pre_guard = EnvGuard::unset("PRE_COMMIT_HOME");
    let _xdg_guard = EnvGuard::unset("XDG_CACHE_HOME");
    // 既存の本体（TempDir 作成 → detect() → assert）は無修正
}
```

- unset を選ぶ理由: テストの意図は「上書き変数が何もない状態で既定パスが存在しない」なので、変数の完全除去（EnvGuard::unset、test_helpers.rs:155-159）が意味に最も近い。EnvGuard::set に空文字を渡す方法もあるが、resolver が空値を unsafe 扱いして default に落とす際に stderr へ WARNING を出す（safety.rs:108-112）ため unset が素直。
- XDG_CACHE_HOME も guard する理由: PRE_COMMIT_HOME を unset すると closure は XDG_CACHE_HOME 分岐に落ちる。シェルに XDG_CACHE_HOME が設定されている開発環境では、既定パスではなく実環境の pre-commit ディレクトリを拾い得るため、2 変数をともに隔離する。
- guard の生存期間: テスト関数の先頭で名前付き束縛し、末尾まで drop させない（huggingface.rs:242-244 のコメント「the guard is alive until the end of the test」と同型）。

### 実装順
1. Red 再現（コード変更なし、macOS）: PRE_COMMIT_HOME に hook.pck を置いた tempdir を設定し cargo test detect_returns_not_found_when_dir_missing → failed（Pruneable が返る）を確認
2. pre_commit.rs の tests モジュールで当該テストの冒頭に TEST_LOCK + EnvGuard::unset 2 行を追加
3. 手順 1 と同一コマンドが green に反転することを確認
4. cargo test pre_commit -- --test-threads 8 を複数回、cargo test 全体、cargo clippy --tests -- -D warnings、cargo fmt --check で検証

### 期待する Red→Green
- Red（現状、決定的に再現可能）: PRE_COMMIT_HOME を実在の hook.pck 入り tempdir に向けて単独実行 → Pruneable が返り assert が落ちる。環境変数が設定されていなければパスするが、並列実行では兄弟の guard 生存期間と重なり得る（= flake）
- Green（修正後）: 同一コマンドが ok。guard が実行前に 2 変数を除去しテスト後に復元するため、シェルの環境変数にも兄弟テストの guard にも結果が依存しなくなる
- 補足: 新規テストは追加しない。赤の再現と緑の反転は同じ既存 1 本のコマンドで確認する

### 落とし穴
- EnvGuard だけ足して TEST_LOCK を取り忘れると、guard の set / unset 自体が他テストの guard と競合し、復元順序の反転という別のレースを生む
- PRE_COMMIT_HOME だけ guard して XDG_CACHE_HOME を残すと、PRE_COMMIT_HOME を unset した瞬間に XDG 分岐が環境変数を拾い得る
- let _ = EnvGuard::unset(...) の形で書くと guard が文末で即 drop され隔離が消える。兄弟テストと同一の let _pre_guard = ... 形（名前付き束縛）で書く

## pin（壊してはいけない契約）
- 既存 10 本のテスト名は一切変更しない。テストの追加・削除・リネームなし
- 兄弟 4 本の guard ありテスト（detect_returns_pruneable_when_cache_exists :155-171、cache_dir_uses_safe_default_when_pre_commit_home_is_unsafe :265-285、cache_dir_uses_safe_default_when_xdg_cache_is_unsafe :288-321、detect_uses_xdg_cache_home_unconditionally :324-341）の TEST_LOCK + EnvGuard パターン・設定値・期待値は無傷
- guard ありテストの挙動: detect_returns_pruneable_when_cache_exists は PRE_COMMIT_HOME 設定下で Pruneable(bytes) を期待、cache_dir_uses_safe_default_when_pre_commit_home_is_unsafe は "/" で resolver の default fallback、cache_dir_uses_safe_default_when_xdg_cache_is_unsafe は空 PRE_COMMIT_HOME + XDG=/etc で default、detect_uses_xdg_cache_home_unconditionally は空 PRE_COMMIT_HOME + XDG tempdir で NotFound
- runner mock 3 本（NoToolRunner :86-103、CliToolRunner :105-124、CliToolRunnerFailing :126-144）と clean_* 5 本は無修正
- 本番コードに触れない: PreCommitCleaner::new（pre_commit.rs:13-43）、cache_dir / detect / clean 委譲（:45-72）、cli_fallback.rs の CliFallbackCleaner::detect と clean_cli_or_fallback、safety.rs の resolve_env_dir と is_safe_delete_target、test_helpers.rs の EnvGuard、context.rs の TEST_LOCK 定義は 1 行も変えない
- EnvGuard::unset の契約（test_helpers.rs:154-159）: 前値を保存し Drop で復元。この API に依存する既存テストを壊さない
- resolve_env_dir が「解決時」に env を読む設計（safety.rs:87-91 に記録、PBI 57 で意図的に scope 外とした注入案）は不変
- pbi/README.md:81-87 の設計方針（dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用）は不変

## scope fence
1. 本番コード（src/cleaners/pre_commit.rs の PreCommitCleaner::new / detect 本体、src/cli_fallback.rs、src/safety.rs、src/test_helpers.rs）は変更しない
2. detect() の環境変数注入（値の constructor への移動）はしない（PBI 57 の設計決定）
3. 同ファイルの clean_* 5 本（clean_dry_run_does_not_delete :174、clean_fallback_deletes_dir :193、clean_uses_cli_when_tool_available :212、clean_dry_run_uses_cli_when_tool_available :227、clean_returns_error_when_cli_fails :246）への guard 追加はしない（clean_fallback_deletes_dir は潜伏 flake 候補、別 PBI 提案）
4. huggingface.rs をはじめ他 cleaner のテストに波及させない
5. delete ledger / Cleaner trait / ProgressReporter / CommandRunner には接触しない
6. 新規テストの追加、test_helpers への新しい guard 型の追加はしない
7. CI ワークフロー（.github/workflows）には触れない
8. CHANGELOG.md は触らない（マージ後に本体で一括記載）
9. Linux/Windows 対応はしない

## DoD
- [ ] cargo test --no-fail-fast が green（実 Trash 操作を伴う環境依存の TCC 拒否失敗を除く）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] Red 再現コマンドが green に反転している（PRE_COMMIT_HOME 設定下の単独実行）
- [ ] cargo test pre_commit -- --test-threads 8 を複数回実行して 1 度も落ちない
- [ ] git diff が src/cleaners/pre_commit.rs の mod tests に限定されている
