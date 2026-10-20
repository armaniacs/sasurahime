# PBI: ~/Library/Caches のアプリ別キャッシュを一覧し、既知アプリ分だけ選んで削除する (library-caches)

## ユーザーストーリー
macOS の開発者として、`~/Library/Caches` 直下のアプリ別キャッシュをサイズ順に見て、既知アプリのものだけを自分で選んで削除したい。なぜなら、このディレクトリは「システムデータ」の主要な構成要素なのに、何が大きいのか分からず、既存の `caches` ターゲット (開発ツール 7 種) では届かないから。

## ビジネス価値
- `~/Library/Caches` の内訳 (どのアプリが何 MB か) を可視化する。未知アプリは表示のみで削除できないため安全
- 既存 cleaner が触る領域は除外リストで隠し、二重計上と誤操作を防ぐ
- 測定方法: `--dry-run` の一覧と、scan の合計が Deletable のみであること

## BDD受け入れシナリオ

```gherkin
Scenario: 閾値を超えた既知アプリだけが削除可能になる
  Given Caches に typescript (65 MB)、SomeUnknownApp (10 MB)、GeoServices (1 GB) がある
  When  sasurahime clean library-caches --dry-run を実行する
  Then  typescript は "would remove" の対象になる
  And   SomeUnknownApp と GeoServices は一覧に表示されるが削除対象にならない

Scenario: 閾値ちょうどは削除対象外
  Given typescript のキャッシュがちょうど 64 MB である
  When  --dry-run を実行する
  Then  BelowThreshold になり削除対象にならない (size > threshold が条件)

Scenario: 既存 cleaner の領域は一覧に出ない
  Given Caches に Homebrew (1 GB)、ms-playwright-go (1 GB)、com.apple.Safari (1 GB) がある
  When  --dry-run を実行する
  Then  3 件とも一覧にも合計にも出ない

Scenario: 非 TTY では何も消さない
  Given 標準入力が TTY でない
  When  dry-run なしで clean library-caches を実行する
  Then  "not a terminal" が stderr に出て、何も削除されず終了コードは 0

Scenario: 実行中のアプリのキャッシュはスキップされる
  Given 対象アプリが実行中である
  When  利用者がそのキャッシュを選択して実行する
  Then  アプリは終了されず、skipped に理由付きで載り削除されない

Scenario: --yes では実行されない
  When  run_auto が対象を選ぶ
  Then  library-caches は auto_clean_allowed() が false のため対象外である

Scenario: 既存の caches と別物である
  When  sasurahime targets を実行する
  Then  "caches" と "library-caches" の両方が出る
  And   caches の挙動 (bun, go, pip, node-gyp, npm, yarn, pnpm の複合実行) は変わらない
```

## 受け入れ基準
- [ ] CLI 名は `library-caches`、enum 変種は `LibraryCaches`。既存の `caches` 変種を変更しない (別物であることを `library_caches_is_registered_and_distinct_from_caches` で固定)
- [ ] 除外リスト `EXCLUDED_NAMES` / `EXCLUDED_PREFIXES` は `library_caches.rs` の 1 か所だけで、大文字小文字を無視して照合する
- [ ] Deletable は `src/hint/apps.rs` の `BaseDir::Caches` の既知アプリで `size > threshold_bytes`。Unknown と BelowThreshold は削除経路に渡さない
- [ ] `detect()` は `scan(false)` で既知アプリのみ測り、合計は Deletable だけ。`sub_targets()` は実装しない。`--all` なし。MultiSelect の既定は全部未選択
- [ ] `auto_clean_allowed()` が `false`。削除は `chflags -R nouchg` の後 `crate::trash::delete_path` のみ
- [ ] `hint::is_running` を `pub(crate) fn` にし、`src/hint/mod.rs` に `pub(crate) use detector::is_running;` を追加する (他の挙動は不変。`mod cleaner;` と `mod detector;` は private のまま)
- [ ] `targets` が 50 行。SUPPORTED.md の件数 50、新節 `## 50.`、stats は `## 51.`、HOWTO-USE.md の表に 1 行

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `tests/library_caches.rs` の 5 件 (`--dry-run` と非 TTY のみ。`scan_includes_library_caches_with_deletable_total_only`、`non_tty_clean_deletes_nothing` を含む)。HOME は tempdir

### 統合テスト
- `scan_skips_excluded_hidden_empty_and_symlinks`、`scan_missing_dir_returns_empty`、`delete_entries_records_failures_as_skipped`、`mark_running` の pgrep の fake runner テスト

### 単体テスト
- `classify_respects_threshold`、`excluded_by_exact_name`、`known_entry_for_ignores_skip...`、`split_running`、設計書 3.4 節の入出力例の表
- main.rs の `library_caches_is_registered_and_distinct_from_caches`、cleaner の `library_caches_is_never_auto_cleaned`

## 実装アプローチ
- **Outside-In**: Task 6 の E2E の観点を念頭に、設計書の順 (Task 1 の純関数から) で内側から積む。各タスクで失敗するテストを先に
- **Red-Green-Refactor**: 1 タスク 1 コミット (計 7 コミット)
- **リファクタリング**: `library_logs.rs` からのコピーは必要部分だけにとどめる

## 見積もり
5 ストーリーポイント (根拠: 設計書のタスク 7 本。ただし対話選択や削除は `library_logs` の型を踏襲でき、判定は純関数中心で難度は中。`is_running` の可視性変更を含む)

## 技術的考慮事項
- 依存関係: PBI-04 (orphan-containers)。ロードマップの固定順 (02 -> 03)。コード上は PBI-02 の S2/S3 だけが前提だが、docs の件数 49 と節番号が PBI-04 の結果を前提にする。`is_running` の可視性変更は本 PBI の Task 1 で行い、PBI-06 が使う
- テスタビリティ: pgrep は `CommandRunner` で fake 化。ゴミ箱を使うテストは `crate::trash::set_trash_mode(false)` を先頭で呼び、末尾で戻す
- 非機能: detect は全体走査をしない (既知アプリのみ `dir_size`)。`clean` と `--dry-run` は全件を測るので数秒かかりうる

## 実装者向け注記

### 先に読む設計書
- `docs/superpowers/specs/2026-10-20-03-library-caches-design.md`: 前提、1.3 節 (名前衝突の決定)、3.3 節 (除外リスト)、3.4 節 (入出力例)、3.5 節、Task 1 から Task 7、6 章 (エッジケース)、7 章 (やりがちな間違い、禁止事項)
- `docs/superpowers/specs/2026-10-20-00-common-conventions.md` の 2.2, 7.3, 11.1

### 現状コードの確認
着手前に必ず実行すること。

```bash
cd /Users/yaar/Playground/sasurahime
cargo test 2>&1 | grep -E "test result|FAILED|failed"             # 全件 ok
grep -n "OrphanContainers :" src/main.rs                            # 1 行 (PBI-04 完了)
cargo run -q -- targets | wc -l                                     # 49
grep -n "^## 50\. \`sasurahime stats\`" docs/SUPPORTED.md            # 2 行
ls src/cleaners/library_caches.rs                                   # No such file なら未実装
grep -rn "LibraryCaches\|library-caches" src | head                 # 出力なしなら未実装
grep -n "fn is_running" src/hint/detector.rs                        # pub(crate) でなければ未実施
```
どれか期待と違えば着手せず報告する。

### 前提となる PBI
PBI-04 (orphan-containers)。

### 実装の進め方
設計書の Task 1 から Task 7 を順に、1 タスク 1 コミット。Task 1 で `is_running` の可視性を変える。設計書のコードは未コンパイル。`known_entry_for` の `path_suffixes == [name]` の比較でコンパイルエラーが出たら止まって報告する。

### 落とし穴
- `sub_targets()` を実装しない (TUI が `--sub` で子プロセスを起動して失敗する)
- `detect()` で `scan(true)` を呼ばない (遅くなる)。合計に Unknown と BelowThreshold を含めない
- `--yes` 除外を `interactive.rs` や `AUTO_CLEAN_TARGETS` で行わない (`auto_clean_allowed()` の `false` だけ)
- `is_running` を `pub use` で再エクスポートしない、`mod detector` を `pub(crate) mod` にしない
- MultiSelect の `defaults` を `true` にしない (`library_logs.rs` のコピペ事故)。`vec![false; n]`
- `library_logs.rs` の `--all` までコピーしない。特殊変種に登録しない (標準エントリ 1 つだけ。両方に書くと clap が panic)
- `Caches` 変種や `caches` の説明を触らない。除外リストを二箇所に書かない
- 未知アプリの削除経路を作らない。`osascript` やアプリの自動終了、`rm -rf` 直接実行をしない
- テストで実機の `~/Library` を触らない

### 実行すべき検証コマンド
```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo test --test library_caches
cargo build
./target/debug/sasurahime targets | grep -E "^(caches|library-caches) "   # 2 行
./target/debug/sasurahime targets | wc -l                                  # 50
HOME=$(mktemp -d) ./target/debug/sasurahime clean library-caches --dry-run # 終了コード 0
./target/debug/sasurahime clean library-caches --dry-run                   # 実機は dry-run のみ
git status --short
```
`--dry-run` なしの `clean library-caches` と `sasurahime --yes` は実機で実行しない。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] 設計書 7.3 節の完了条件チェックリストをすべて満たす
- [ ] 7 コミット、日本語 Conventional Commits、個別 add
- [ ] `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` がゼロ警告
- [ ] コードレビュー完了
- [ ] リファクタリング完了
- [ ] ドキュメント更新済み (SUPPORTED.md、HOWTO-USE.md)
