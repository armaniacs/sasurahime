# 実装ロードマップと順序

対象ブランチ: `feat/apfs-snapshot-enhancement`。基準は HEAD (2c71587) です。
サブプロジェクト 1 (APFS snapshot) は実装済みです (計画は `docs/superpowers/plans/2026-10-20-apfs-snapshot-enhancement.md`)。
本書は、残り 4 本の設計書を「どの順で、何を前提に、どこを触って」実装するかを決めます。
共通の規約と判断は `2026-10-20-00-common-conventions.md` (以下、共通規約) にあります。設計書と共通規約が食い違って見えたら、実装を止めて報告してください。

## 1. 実装順序 (固定)

| 順 | 設計書 | タスク | 完了時の `sasurahime targets` の行数 |
|----|--------|--------|------------------------------------|
| 0 | APFS snapshot | (実装済み) | 48 |
| 1 | `2026-10-20-04-docker-and-scan-coverage-design.md` | S1 -> S2 -> S3 -> D1 -> D2 -> D3 -> T4 -> T5 | 48 |
| 2 | `2026-10-20-02-orphan-containers-design.md` | Task 1 -> 6 | 49 |
| 3 | `2026-10-20-03-library-caches-design.md` | Task 1 -> 7 | 50 |
| 4 | `2026-10-20-05-app-support-design.md` | Task 1 -> 13 | 51 |

- 順番を入れ替えません。並行して進めません。
- 1 本の設計書が全タスク完了してから次の設計書に進みます。

## 2. 前提タスクと依存関係

| 設計書 | 着手の前提 | 理由 |
|--------|------------|------|
| 04 | 基準コミットで `cargo test`、`cargo clippy -- -D warnings`、`cargo fmt --check` が通ること | 出発点の確認 |
| 02 | 04 の全タスク (S1 から T5) が完了していること | コード上は 04 の S2 (`Cleaner::auto_clean_allowed()` の追加) と S3 (`all_cleaners()` を `standard_cleaners()` と `special_cleaners()` から組み立て、`AUTO_CLEAN_TARGETS` 外を `ManualOnly` で包む) だけが前提です。ただし 02 の `docs/SUPPORTED.md` 更新は、04 の T5 が件数を 48 に直した後の状態を前提にしています。そのため 04 の全タスク完了を前提とします |
| 03 | 02 の全タスクが完了していること | `docs/SUPPORTED.md` の件数 (49 -> 50) と節番号 (50) が 02 の結果を前提にしているため。コード上の依存は 04 の S2 と S3 だけです |
| 05 | 03 の全タスクが完了していること | 05 は 03 が行う `hint::is_running` の可視性変更 (`pub(crate)` と再エクスポート) と、02 の `collect_installed_ids` を使います。`docs/SUPPORTED.md` の件数 (50 -> 51) と節番号 (51) も 03 の結果が前提です |

04 の内部の依存は 04 の 6 章のとおりです (S2 を S3 より先に行う。D2 は S3 と D1 の後。D3 は D2 の後。T4 は S3 と D3 の後。T5 は T4 の後)。

## 3. 共有する型と関数

| 項目 | 定義する設計書 (タスク) | 使う設計書 | 最終的な形 |
|------|------------------------|------------|-----------|
| `Cleaner::auto_clean_allowed(&self) -> bool` (既定 `true`) | 04 (S2) | 02、03、05 | `src/cleaner.rs` の trait メソッド。02/03/05 は自分の `impl Cleaner` で `false` を返すようにオーバーライドする |
| `Cleaner::advisory(&self) -> Option<String>` (既定 `None`) | 04 (S2) | 04 のみ | `src/cleaner.rs` の trait メソッド |
| `ManualOnly` | 04 (S2) | 04 (S3 の `all_cleaners()`) | `src/cleaners/manual_only.rs`。`AUTO_CLEAN_TARGETS` に無い cleaner を自動で包み、`auto_clean_allowed()` を `false` にする |
| `auto_clean_indices(results, cleaners) -> Vec<usize>` | 04 (S2) | `run_auto` | `src/interactive.rs`、`pub(crate)` |
| `standard_cleaners()` / `special_cleaners()` / `AUTO_CLEAN_TARGETS` | 04 (S3) | 02、03、05 (間接的に) | `src/main.rs`。02/03/05 はマクロ表に 1 エントリ足すだけで `standard_cleaners()` に入る |
| `hint::is_running(process_name, runner) -> bool` | 03 (Task 1) | 03、05 | `src/hint/detector.rs` で `pub(crate) fn`、`src/hint/mod.rs` で `pub(crate) use detector::is_running;`。呼び出しは `crate::hint::is_running` |
| `hint::quit_app_and_wait(entry, runner)` / `hint::relaunch_app(entry, runner)` | 05 (Task 1) | 05 | `src/hint/cleaner.rs` で `pub(crate) fn`、`src/hint/mod.rs` で `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` |
| `orphan_containers::collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String>` | 02 (Task 2) | 05 (Task 8、10) | 小文字化済みの bundle ID 集合を返す `pub fn` |
| `crate::format::dir_size` | 既存 | 02、03、05、04 | 物理ブロック (`st_blocks * 512`) の合計 |

`src/hint/mod.rs` の `mod cleaner;` と `mod detector;` は private のまま変えません (`pub(crate) mod` にしません)。

## 4. ファイルごとの変更の衝突確認

各設計書は前の設計書の完了後の状態に対して変更します。同じ行を 2 つの設計書が書き換える箇所は、下表の「順に適用」の箇所だけです。

| ファイル | 04 | 02 | 03 | 05 | 衝突 |
|----------|----|----|----|----|------|
| `src/main.rs` マクロ表 (`define_cleaners!` の標準エントリ) | docker の factory を変更 (D2) | 末尾 (単独の `;` 行の直前) に `OrphanContainers` を追加 | 同じ位置に `LibraryCaches` を追加 | 同じ位置に `AppSupport` を追加 | 順に適用。各自「単独の `;` 行の直前」に 1 エントリ足すだけ |
| `src/main.rs` マクロ本体、`all_cleaners()` | `standard_cleaners()` 生成、`special_cleaners()`、`AUTO_CLEAN_TARGETS` (S3) | 変更なし | 変更なし | 変更なし | なし |
| `src/main.rs` 特殊変種、`command_name()`、`dry_run()`、`extra_targets()`、`matches!`、`match target` | 変更なし | 変更なし | 変更なし | 変更なし | なし (02/03/05 は標準エントリで登録するため触らない) |
| `src/main.rs` 先頭の `mod` 宣言 | `mod disk_image;` を追加 (D1) | なし | なし | なし | なし |
| `src/main.rs` の `mod tests` | 末尾にテスト追加 (S1、S3) | 末尾にテスト 1 件 | 末尾にテスト 1 件 | 末尾にテスト 1 件 | 順に末尾へ追記。テスト名は全て異なる |
| `src/interactive.rs` | `auto_clean_indices` 追加、`run_auto` 置換 (S2) | なし | なし | なし | なし |
| `src/cleaner.rs` | trait に 2 メソッド追加 (S2) | なし | なし | なし | なし |
| `src/cleaners/mod.rs` | `manual_only` | `orphan_containers` | `library_caches` | `app_support` | 別々の行。アルファベット順の位置に 1 行ずつ |
| `src/hint/detector.rs` | なし | なし | `is_running` を `pub(crate) fn` に | 変更なし (03 の結果を確認するだけ) | なし |
| `src/hint/mod.rs` | なし | なし | `pub(crate) use detector::is_running;` を追加 | `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` を追加、`mod tests` 末尾にテスト | 別々の行 |
| `src/hint/cleaner.rs` | なし | なし | なし | `auto_clean_hint` から 2 関数を抽出 | なし |
| `src/explorer.rs` | なし | なし | なし | なし | なし (05 は Finder 起動を runner 経由の `open` で行う) |
| `docs/SUPPORTED.md` 件数の行 (英日 2 か所) | 49 -> 48 | 48 -> 49 | 49 -> 50 | 50 -> 51 | 順に適用 |
| `docs/SUPPORTED.md` Quick Reference 表 (英日) | なし | `trash` 行の直前に 1 行 | 同じ位置に 1 行 | 同じ位置に 1 行 | 順に適用 |
| `docs/SUPPORTED.md` 節 | docker 節に段落追加、Interactive 表の `--yes` 行置換 | `## 49.` を追加、stats を `## 50.` に | `## 50.` を追加、stats を `## 51.` に | `## 51.` を追加、stats を `## 52.` に | 順に適用。新節は常に stats 節の直前 |
| `README.md` | `--yes` のコメント 2 行、Data access に 1 行 | Data access に 1 行 | なし | なし | 別々の行 |
| `docs/HOWTO-USE.md` | `--yes` 節に 1 文 | 対象表 (英日) に 1 行 | 対象表 (英日) に 1 行 | 対象表 (英日) に 1 行 | 別々の行 |
| `docs/HOWTO-ADD-target.md` | 1 文追加 | なし | なし | なし | なし |
| `Cargo.toml` | なし | なし | なし | なし | なし (`description` の「40+」は 51 件でも正しいので変えない) |

`README.md` の「40+」「40 以上」と `Cargo.toml` の「40+」は、どの段階でも正しいので変更しません。

## 5. 各ステップ後に必ず通すコマンド

作業ディレクトリは `/Users/yaar/Playground/sasurahime` です。

各タスクのコミット前:

```bash
cargo fmt --check
cargo test
```

各設計書の最終タスクの完了時 (と、設計書が指定したタスクの完了時):

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run -q -- targets | wc -l
git status --short
```

- `cargo fmt --check` と `cargo clippy -- -D warnings` は出力なし、`cargo test` は全 `test result:` が `ok`。
- `targets | wc -l` は 1 章の表の値。
- `git status --short` に意図しないファイルが無いこと (`docs/superpowers/` の未追跡は除く)。
- 途中タスクの clippy の扱いは共通規約 1.4 のとおりです (呼び出し元がまだ無い関数への `dead_code` / 未使用警告だけは途中で許容)。

## 6. 実装者向けの注意

1. 1 本の設計書を 1 セッションで実装します。セッションをまたぐ場合は、前のセッションの最後のコミットと `git status` を確認してから再開します。
2. 1 章の順序を守ります。前の設計書の完了条件を満たしていなければ着手しません。着手前に 5 章のコマンドで緑を確認します。
3. 設計書に書いていない判断 (名前、閾値、除外リスト、文言、削除範囲、登録方法、ファイルの追加) をしません。
4. 設計書のコードは未コンパイルのものがあります。各設計書の冒頭の「未検証の仮定」を先に読みます。コンパイルエラーが設計書のコード由来で、2 回の修正で直らなければ止まります。
5. 詰まったら止まって報告します (共通規約 10 章の条件)。報告には、設計書名、タスク番号、実行したコマンド、出力の要点を含めます。
6. 実機の HOME で `sasurahime clean ...` (`--dry-run` なし) と `sasurahime --yes` を実行しません。
7. `git add -A` / `git add .` を使いません。`docs/superpowers/` はコミットに含めません。

## 7. 既知の既存問題 (今回は触らない)

- `ios-backup` は `--yes` で実行された場合、TTY 上では確認プロンプトを出します (`--yes` の対象から外れていない)。今回の 4 本では修正しません。
