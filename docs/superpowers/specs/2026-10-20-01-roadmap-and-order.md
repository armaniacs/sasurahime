# 実装ロードマップと順序

対象ブランチ: `feat/apfs-snapshot-enhancement`。基準は HEAD (2c71587) です。
サブプロジェクト 1 (APFS snapshot) は実装済みです (計画は `docs/superpowers/plans/2026-10-20-apfs-snapshot-enhancement.md`)。
本書は、残り 4 本の設計書を「どの順で、何を前提に、どこを触って」実装するかを決めます。4 本の前に行うドキュメントだけの前提タスク P0 (7 章) と、今回は扱わず別 PBI に回す事項の一覧 (8 章) も本書に置きます。
共通の規約と判断は `2026-10-20-00-common-conventions.md` (以下、共通規約) にあります。設計書と共通規約が食い違って見えたら、実装を止めて報告してください。

## 1. 実装順序 (固定)

| 順 | 設計書 | タスク | 完了時の `sasurahime targets` の行数 |
|----|--------|--------|------------------------------------|
| 0 | APFS snapshot | (実装済み) | 48 |
| 1 | 本書 7 章 (前提タスク P0: README Data access の事実化) | P0 | 48 |
| 2 | `2026-10-20-04-docker-and-scan-coverage-design.md` | S1 -> S2 -> S3 -> D1 -> D2 -> D3 -> T4 -> T5 | 48 |
| 3 | `2026-10-20-02-orphan-containers-design.md` | Task 1 -> 6 | 49 |
| 4 | `2026-10-20-03-library-caches-design.md` | Task 1 -> 7 | 50 |
| 5 | `2026-10-20-05-app-support-design.md` | Task 1 -> 13 | 51 |

- 順番を入れ替えません。並行して進めません。
- 1 本の設計書が全タスク完了してから次の設計書に進みます。P0 も 04 の S1 より前に完了させます。
- 設計書どうしの順序は 04 -> 02 -> 03 -> 05 です。各設計書の本文にある「実装順は 04 -> 02 -> 03 -> 05」は、P0 を済ませた後の設計書の順序を指します。

## 2. 前提タスクと依存関係

| 設計書 | 着手の前提 | 理由 |
|--------|------------|------|
| P0 | 基準コミットで `cargo test`、`cargo clippy -- -D warnings`、`cargo fmt --check` が通ること | 出発点の確認 |
| 04 | P0 が完了していること | 04 の T5 は、P0 で書き直した README の Data access 節に 1 行足します。また共通規約 1.6 の規則 (README の約束を変える変更は同じタスクで README を直す) の前提として、README が実際の挙動と一致した状態から始めます |
| 02 | 04 の全タスク (S1 から T5) が完了していること | コード上は 04 の S2 (`Cleaner::auto_clean_allowed()` の追加) と S3 (`all_cleaners()` を `standard_cleaners()` と `special_cleaners()` から組み立て、`AUTO_CLEAN_TARGETS` 外を `ManualOnly` で包む) だけが前提です。ただし 02 の `docs/SUPPORTED.md` 更新は、04 の T5 が件数を 48 に直した後の状態を前提にしています。README の更新 (02 の Task 6) は P0 の段落と 04 の T5 の行を前提にしています。そのため 04 の全タスク完了を前提とします |
| 03 | 02 の全タスクが完了していること | `docs/SUPPORTED.md` の件数 (49 -> 50) と節番号 (50) が 02 の結果を前提にしているため。コード上の依存は 04 の S2 と S3 だけです |
| 05 | 03 の全タスクが完了していること | 05 は 03 が行う `hint::is_running` の可視性変更 (`pub(crate)` と再エクスポート) と、02 の `AppIdentity` と `collect_app_identities` を使います。`docs/SUPPORTED.md` の件数 (50 -> 51) と節番号 (51) も 03 の結果が前提です |

04 の内部の依存は 04 の 6 章のとおりです (S2 を S3 より先に行う。D2 は S3 と D1 の後。D3 は D2 の後。T4 は S3 と D3 の後。T5 は T4 の後)。

## 3. 共有する型と関数

| 項目 | 定義する設計書 (タスク) | 使う設計書 | 最終的な形 |
|------|------------------------|------------|-----------|
| `Cleaner::auto_clean_allowed(&self) -> bool` (既定 `true`) | 04 (S2) | 02、03、05 | `src/cleaner.rs` の trait メソッド。02/03/05 は自分の `impl Cleaner` で `false` を返すようにオーバーライドする |
| `Cleaner::advisory(&self) -> Option<String>` (既定 `None`) | 04 (S2) | 04 のみ | `src/cleaner.rs` の trait メソッド |
| `ManualOnly` | 04 (S2) | 04 (S3 の `all_cleaners()`) | `src/cleaners/manual_only.rs`。`AUTO_CLEAN_TARGETS` に無い cleaner を自動で包み、`auto_clean_allowed()` を `false` にする |
| `auto_clean_indices(results, cleaners) -> Vec<usize>` | 04 (S2) | `run_auto` | `src/interactive.rs`、`pub(crate)` |
| `standard_cleaners()` / `special_cleaners()` / `AUTO_CLEAN_TARGETS` | 04 (S3) | 02、03、05 (間接的に) | `src/main.rs`。02/03/05 はマクロ表に 1 エントリ足すだけで `standard_cleaners()` に入る |
| `GenericCleaner::unsupported_filter_warning(&self, config: &Config) -> Option<String>` | 04 (S3) | 04 のみ | `src/cleaners/generic.rs` の private メソッド。`with_config` が出す警告の文面を返す |
| `hint::is_running(process_name, runner) -> bool` | 03 (Task 1) | 03、05 | `src/hint/detector.rs` で `pub(crate) fn`、`src/hint/mod.rs` で `pub(crate) use detector::is_running;`。呼び出しは `crate::hint::is_running` |
| `hint::quit_app_and_wait(entry, runner)` / `hint::relaunch_app(entry, runner)` | 05 (Task 1) | 05 | `src/hint/cleaner.rs` で `pub(crate) fn`、`src/hint/mod.rs` で `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` |
| `orphan_containers::AppIdentity { bundle_id: String, name: Option<String>, display_name: Option<String> }` | 02 (Task 2) | 05 (Task 8、10) | `Info.plist` から取り出す値はこの 3 項目だけ (共通規約 11.2)。フィールドを増やさない |
| `orphan_containers::collect_app_identities(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> Vec<AppIdentity>` | 02 (Task 2) | 05 (Task 8、10) | インストール先の `.app` ごとの `AppIdentity`。bundle ID は plist に書かれた綴りのまま |
| `orphan_containers::collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String>` | 02 (Task 2) | 02 のみ | `collect_app_identities` の bundle ID を小文字化した集合を返す `pub fn` |
| `crate::format::dir_size` | 既存 | 02、03、05、04 | 物理ブロック (`st_blocks * 512`) の合計 |

`src/hint/mod.rs` の `mod cleaner;` と `mod detector;` は private のまま変えません (`pub(crate) mod` にしません)。

## 4. ファイルごとの変更の衝突確認

各設計書は前の設計書の完了後の状態に対して変更します。同じ行を 2 つの設計書が書き換える箇所は、下表の「順に適用」の箇所だけです。P0 が触るのは `README.md` だけです。

| ファイル | 04 | 02 | 03 | 05 | 衝突 |
|----------|----|----|----|----|------|
| `src/main.rs` マクロ表 (`define_cleaners!` の標準エントリ) | colima の factory に `.with_config(config)` を付ける (S3)、docker の factory を変更 (D2) | 末尾 (単独の `;` 行の直前) に `OrphanContainers` を追加 | 同じ位置に `LibraryCaches` を追加 | 同じ位置に `AppSupport` を追加 | 順に適用。各自「単独の `;` 行の直前」に 1 エントリ足すだけ |
| `src/main.rs` マクロ本体、`all_cleaners()` | `standard_cleaners()` 生成、`special_cleaners()`、`AUTO_CLEAN_TARGETS` (S3) | 変更なし | 変更なし | 変更なし | なし |
| `src/main.rs` 特殊変種、`command_name()`、`dry_run()`、`extra_targets()`、`matches!`、`match target` | 変更なし | 変更なし | 変更なし | 変更なし | なし (02/03/05 は標準エントリで登録するため触らない) |
| `src/main.rs` 先頭の `mod` 宣言 | `mod disk_image;` を追加 (D1) | なし | なし | なし | なし |
| `src/main.rs` の `mod tests` | 末尾にテスト追加 (S1、S3) | 末尾にテスト 1 件 | 末尾にテスト 1 件 | 末尾にテスト 1 件 | 順に末尾へ追記。テスト名は全て異なる |
| `src/cleaners/generic.rs` | `with_config` から `unsupported_filter_warning` を抽出し `mod tests` にテスト 3 件 (S3)、`notice` / `docker` / colima の案内 / テスト (D2) | なし | なし | なし | なし (04 の中で S3 -> D2 の順) |
| `src/interactive.rs` | `auto_clean_indices` 追加、`run_auto` 置換 (S2) | なし | なし | なし | なし |
| `src/cleaner.rs` | trait に 2 メソッド追加 (S2) | なし | なし | なし | なし |
| `src/cleaners/mod.rs` | `manual_only` | `orphan_containers` | `library_caches` | `app_support` | 別々の行。アルファベット順の位置に 1 行ずつ |
| `src/hint/detector.rs` | なし | なし | `is_running` を `pub(crate) fn` に | 変更なし (03 の結果を確認するだけ) | なし |
| `src/hint/mod.rs` | なし | なし | `pub(crate) use detector::is_running;` を追加 | `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` を追加、`mod tests` 末尾にテスト | 別々の行 |
| `src/hint/cleaner.rs` | なし | なし | なし | `auto_clean_hint` から 2 関数を抽出 | なし |
| `src/explorer.rs` | なし | なし | なし | なし | なし (05 は Finder 起動を runner 経由の `open` で行う。共通化は別 PBI、8 章) |
| `docs/SUPPORTED.md` 件数の行 (英日 2 か所) | 49 -> 48 | 48 -> 49 | 49 -> 50 | 50 -> 51 | 順に適用 |
| `docs/SUPPORTED.md` Quick Reference 表 (英日) | なし | `trash` 行の直前に 1 行 | 同じ位置に 1 行 | 同じ位置に 1 行 | 順に適用 |
| `docs/SUPPORTED.md` 節 | docker 節に段落追加、Interactive 表の `--yes` 行置換 | `## 49.` を追加、stats を `## 50.` に | `## 50.` を追加、stats を `## 51.` に | `## 51.` を追加、stats を `## 52.` に | 順に適用。新節は常に stats 節の直前 |
| `README.md` | `--yes` のコメント 2 行、Data access のディレクトリ一覧に 1 行 (英日) | Data access のディレクトリ一覧に 1 行と、内容を読むファイルの一覧に 1 項目 (英日) | なし | なし | P0 が Data access の段落を置き換えた後に順に適用。別々の行 |
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
- P0 は `README.md` だけを変えるタスクですが、上の 5 つのコマンドも実行し、基準コミットと結果が変わらないこと (全件 ok、48) を確認します。

## 6. 実装者向けの注意

1. 1 本の設計書を 1 セッションで実装します。セッションをまたぐ場合は、前のセッションの最後のコミットと `git status` を確認してから再開します。P0 は 04 と同じセッションの最初に行ってかまいません。
2. 1 章の順序を守ります。前の設計書の完了条件を満たしていなければ着手しません。着手前に 5 章のコマンドで緑を確認します。
3. 設計書に書いていない判断 (名前、閾値、除外リスト、文言、削除範囲、登録方法、ファイルの追加) をしません。
4. 設計書のコードは未コンパイルのものがあります。各設計書の冒頭の「未検証の仮定」を先に読みます。コンパイルエラーが設計書のコード由来で、2 回の修正で直らなければ止まります。
5. 詰まったら止まって報告します (共通規約 10 章の条件)。報告には、設計書名、タスク番号、実行したコマンド、出力の要点を含めます。
6. 実機の HOME で `sasurahime clean ...` (`--dry-run` なし) と `sasurahime --yes` を実行しません。
7. `git add -A` / `git add .` を使いません。`docs/superpowers/` はコミットに含めません。
8. README の Privacy / Data access の主張を変える可能性がある変更 (新しいファイル内容の読み取り、ネットワーク、保存) は、同じタスクで README (英語・日本語) を更新します (共通規約 1.6)。設計書に README の指示が無いのにこれに当たる変更が必要になったら、止まって報告します。
9. 8 章に挙げた事項は、気づいても今回は直しません。

## 7. 前提タスク P0: README Data access の事実化

### 7.1 目的と事実

README の Data access 節は「ディレクトリのメタデータだけを読み、ファイルの内容は読まない」と約束していますが、基準コミットの時点で既に正しくありません。

- `src/cleaners/mise.rs` の 113 行 (`fs::read_to_string(global_config)`) と 125 行 (`fs::read_to_string(entry.path())`) が、mise の `~/.config/mise/config.toml` と、HOME 配下 (walkdir の `max_depth(5)`) の `.mise.toml` の内容を読みます。使用中の runtime のバージョンを削除しないためです (CLAUDE.md の安全規則)。
- `src/config.rs` の 86 行が sasurahime 自身の設定ファイル (`~/.config/sasurahime/config.toml`、または `--config` で指定したファイル) を読みます。
- `src/history.rs` の 132 行が sasurahime 自身の削除履歴 (`~/.local/share/sasurahime/history.json`) を読みます。

このタスクで README を実際の挙動に合わせます。以降の 04、02 は、ここで作った形 (「内容を読むファイルの一覧」) に 1 行ずつ足すだけにします (共通規約 11.5)。

コードは変更しません。テストの追加もありません (ドキュメントだけのタスクです)。

### 7.2 英語の変更 (README.md 93 行付近)

置き換える行 (この 1 行全体):

```markdown
The tool reads directory metadata (names, sizes, modification times) to determine what can be cleaned. It does **not** read the contents of your files.
```

置き換え後 (空行を含めて 5 行):

```markdown
The tool reads file and directory metadata (names, sizes, modification times) to determine what can be cleaned. It reads the contents of only the following files, and only to decide what can be cleaned:

- mise configuration files (`~/.config/mise/config.toml` and every `.mise.toml` within 5 directory levels of your home directory), to find runtime versions that are still in use so that they are not deleted

Values read from these files are not written to logs or to the deletion history. sasurahime also reads its own configuration file (`~/.config/sasurahime/config.toml`, or the file given with `--config`) and its deletion history (`~/.local/share/sasurahime/history.json`). It does **not** read the contents of any other file.
```

### 7.3 日本語の変更 (README.md 203 行付近)

置き換える行 (この 1 行全体):

```markdown
ツールはディレクトリのメタデータ（名前、サイズ、更新日時）のみを読み取り、ファイルの内容を読み取ることはありません。
```

置き換え後 (空行を含めて 5 行):

```markdown
ツールはファイルとディレクトリのメタデータ（名前、サイズ、更新日時）を読み取り、削除できるものを判断します。ファイルの内容を読み取るのは次のファイルだけで、目的は削除できるものの判断に限ります:

- mise の設定ファイル（`~/.config/mise/config.toml` と、ホームディレクトリから 5 階層以内にあるすべての `.mise.toml`）。使用中の runtime のバージョンを特定し、削除しないようにするため

これらのファイルから読み取った値は、ログにも削除履歴にも書き出しません。このほか、sasurahime 自身の設定ファイル（`~/.config/sasurahime/config.toml`、または `--config` で指定したファイル）と削除履歴（`~/.local/share/sasurahime/history.json`）を読み取ります。それ以外のファイルの内容は読み取りません。
```

- 「読み取った値をログにも削除履歴にも書き出さない」の対象は、一覧に載ったファイル (mise の設定ファイルと、02 が加える `Info.plist`) です。sasurahime 自身の設定と削除履歴は一覧の外の文で述べます。削除履歴は読み込んだ内容に 1 件足して書き戻すため、一覧に入れると「履歴に書き出さない」と矛盾するからです。
- 削除履歴に記録される項目 (`HistoryEntry` の `timestamp`、`cleaner`、`freed_bytes`、`skipped_count`) は、ファイルから読んだ値を入れる欄を持ちません。
- 1 行目の `:` は英語、日本語とも半角です (既存の「以下のディレクトリを読み取り、...を特定します:」と同じ)。

### 7.4 後続タスクが加える行 (P0 では書かない)

P0 の時点では、02 の `Info.plist` の読み取りはまだ実装されていません。README は実際の挙動だけを書くので、`Info.plist` の項目は 02 の Task 6 で足します (共通規約 1.6)。後続の追記は次のとおりです。

- 04 の T5: ディレクトリの一覧 (`MobileSync/Backup/` の行の次) に Docker Desktop と `~/.colima/` の 1 行 (英日)。
- 02 の Task 6: ディレクトリの一覧 (04 の行の次) に `~/Library/Containers/` と `~/Library/Group Containers/` の 1 行と、内容を読むファイルの一覧 (mise の項目の次) に `Info.plist` の 1 項目 (英日)。

02 の完了後の「内容を読むファイル」の部分は、英語が次の形になります (参考。P0 ではこのとおりには書きません)。

```markdown
The tool reads file and directory metadata (names, sizes, modification times) to determine what can be cleaned. It reads the contents of only the following files, and only to decide what can be cleaned:

- mise configuration files (`~/.config/mise/config.toml` and every `.mise.toml` within 5 directory levels of your home directory), to find runtime versions that are still in use so that they are not deleted
- the `Info.plist` of installed apps (`.app` bundles in `/Applications`, `~/Applications` and `/System/Applications`, and one folder level below them); only the bundle ID and the app name are taken from it, to tell leftovers of uninstalled apps from the data of installed apps

Values read from these files are not written to logs or to the deletion history. sasurahime also reads its own configuration file (`~/.config/sasurahime/config.toml`, or the file given with `--config`) and its deletion history (`~/.local/share/sasurahime/history.json`). It does **not** read the contents of any other file.
```

日本語は次の形になります (参考)。

```markdown
ツールはファイルとディレクトリのメタデータ（名前、サイズ、更新日時）を読み取り、削除できるものを判断します。ファイルの内容を読み取るのは次のファイルだけで、目的は削除できるものの判断に限ります:

- mise の設定ファイル（`~/.config/mise/config.toml` と、ホームディレクトリから 5 階層以内にあるすべての `.mise.toml`）。使用中の runtime のバージョンを特定し、削除しないようにするため
- インストール済みアプリの `Info.plist`（`/Applications`、`~/Applications`、`/System/Applications` の直下と 1 階層下にある `.app`）。取り出すのは bundle ID とアプリ名だけで、アンインストール済みアプリの残骸とインストール済みアプリのデータを見分けるため

これらのファイルから読み取った値は、ログにも削除履歴にも書き出しません。このほか、sasurahime 自身の設定ファイル（`~/.config/sasurahime/config.toml`、または `--config` で指定したファイル）と削除履歴（`~/.local/share/sasurahime/history.json`）を読み取ります。それ以外のファイルの内容は読み取りません。
```

### 7.5 確認

```bash
grep -n "It does \*\*not\*\* read the contents of your files" README.md   # 出力なし
grep -n "ファイルの内容を読み取ることはありません" README.md              # 出力なし
grep -n "It reads the contents of only the following files" README.md     # 1 行
grep -n "ファイルの内容を読み取るのは次のファイルだけ" README.md          # 1 行
grep -n "^- mise configuration files\|^- mise の設定ファイル" README.md    # 2 行
git diff --stat                                                            # README.md だけ
```

続けて 5 章の 5 つのコマンドを実行し、全件 ok と `48` を確認します。

### 7.6 コミット

```bash
git add README.md
git commit -m "docs(readme): Data access を実際に内容を読むファイルに合わせて書き直す"
```

## 8. 別 PBI にする事項 (今回は触らない)

次の事項は今回の 4 本と P0 では直しません。気づいても手を付けず、必要なら報告に書くだけにします。それぞれ別の PBI としてバックログに積みます。

| 事項 | 現状 | 今回扱わない理由 | 別 PBI でやること |
|------|------|------------------|-------------------|
| explorer の Finder 起動の共通化 | `src/explorer.rs` の private 関数 `open_in_finder` は `std::process::Command` を直接呼び、`run_explore` は 1 件ごとに `Confirm` を挟みます。05 の第 2 段は runner 経由の `open` を自前で呼び、6 件以上のときだけ 1 回確認します (05 の 4.3 節、Task 12)。同じ「Finder で開く」が 2 か所にあります | 05 は fake runner で `open` の呼び出し回数をテストしたいこと、今回 `explorer.rs` を変更しないこと | runner を注入できる共通関数 (開く処理と、確認が必要かの判定) に寄せ、explorer と app-support の確認方針を揃えるかを決める |
| `per_cleaner` 設定の無効警告を起動時にまとめて出す | `GenericCleaner::with_config` は Command 系の cleaner に `older_than_days` / `larger_than_mb` が設定されていると `log::warn!` を出します。警告が出るかどうかは factory が `with_config` を呼ぶかで決まります。マクロ表では colima (04 の S3 で `with_config` を付ける) と 9 件 (act, node-gyp, spm, downloads, vscode-extensions, terraform, volta, sbt, tree-sitter) 以外の 29 件 (CLI 名で uv, brew, mise, browsers, bun, go, pip, npm, yarn, pnpm, cargo, docker, orbstack, cocoa-pods, conda, poetry, pipx, deno, dotnet, gem, bundle, rustup, simulator, gradle, huggingface, pre-commit, jetbrains, maven, flutter) が、factory の第 2 引数を `_config` にして設定を無視し、警告も出しません。scan では cleaner ごとに構築するため、同じ警告が scan と clean の経路で別々に出ます | 29 件の factory を変えると dispatch と scan の両方の挙動に触れ、04 の範囲を超えるため | 設定の読み込み直後に、`per_cleaner` のキーを全 cleaner の `name()` と対応可否に照合し、未知の名前や効かない設定を 1 回だけまとめて警告する。29 件の factory の不整合もそこで解消する |
| `ios-backup` の `--yes` プロンプト問題 | `ios-backup` は `--yes` の対象 (`AUTO_CLEAN_TARGETS`) に残っており、TTY 上で `--yes` を付けると確認プロンプトを出します。`docs/SUPPORTED.md` の「`--yes` では非実行」とも食い違います | 4 本のどれの範囲でもなく、`--yes` の対象を変えると既存 15 件の挙動変更になるため (04 の非目標) | `ios-backup` を `--yes` の対象から外すか、`--yes` 時はプロンプトを出さずにスキップするかを決め、ドキュメントと揃える |
