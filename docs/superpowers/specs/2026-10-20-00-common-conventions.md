# sasurahime 共通規約書

対象ブランチ: feat/apfs-snapshot-enhancement (基準コミット 2c71587)。
対象機能: 2 orphan-containers / 3 library-caches / 4 Docker.raw と scan 網羅 / 5 app-support。
実装順は 04 -> 02 -> 03 -> 05 で固定です。04 の前に、ドキュメントだけの前提タスク P0 (README の Data access の事実化) を行います。順序、前提タスク、ファイルごとの衝突確認、別 PBI に回す事項は `2026-10-20-01-roadmap-and-order.md` (以下、ロードマップ) にあります。
各設計書はこの規約書を参照します。ここに書いてあることは各設計書では繰り返しません。設計書の間で共通する判断は 11 章にまとめてあります。
行番号は基準コミット時点の目安です。04 の実装後は `src/main.rs` の行番号が大きくずれます。ずれていたら関数名や引用したコードで探してください。

## 1. プロジェクト規約

### 1.1 言語と文字

- 応答、説明、コミットメッセージ、ドキュメントは日本語です。コード、識別子、コード内コメントは英語です。
- 絵文字は使いません。
- Markdown ドキュメントは Shift-JIS で表現できる文字だけで書きます。EM DASH、不等号記号の合字 (小なりイコール等)、近似記号、丸付き数字、中黒の箇条書き記号は使いません。
  - 既存コードに全角ダッシュや警告記号が残っている箇所がありますが、真似しないでください。

### 1.2 コードコメント

- 書くのは非自明な WHY だけです。隠れた制約、workaround の理由、読み手が驚く挙動などです。
- 書かないもの: WHAT (コードを読めば分かること)、変更履歴、タスク ID や issue 参照。
- docs や README にも同じ方針を適用します。履歴や経緯は書かず、最新仕様だけを書きます。

### 1.3 Git

- コミットメッセージは日本語の Conventional Commits です。type は feat / fix / docs / refactor / test / chore のいずれかです。
  - 例: `feat(library-caches): ~/Library/Caches の巨大エントリを一覧表示する cleaner を追加`
- `git add -A` と `git add .` は禁止です。対象ファイルを 1 つずつ指定して add します。
- push、マージ、ブランチ削除はしません。

### 1.4 品質ゲート

- `cargo clippy -- -D warnings` が警告ゼロで通ること。
  - 例外: 新しいモジュールや機能を複数タスクに分けて作る途中では、まだ本番コードからの呼び出し元が無い関数、型、trait メソッドへの `dead_code` 警告と、未使用の `use` (再エクスポートを含む) 警告だけを許容します。各設計書が例外の範囲を書いている場合はそれに従います (04 の 0 章など)。それ以外の警告は途中でも許容しません。
  - 各設計書の最終タスクの完了時 (登録が済んだ後) は、例外なく警告ゼロにします。
- `cargo fmt --check` が差分ゼロで通ること (差分が出たら `cargo fmt` を実行する)。各タスクのコミット前に必ず確認します。
- `cargo test` が全件通ること。各タスクのコミット前に必ず確認します。
- 対象プラットフォームは macOS のみです。Linux / Windows 対応は追加しません。
- 新しい依存クレートを `Cargo.toml` に追加しません (設計書で明示された場合を除く)。

### 1.5 セキュリティ

- `.env` や認証情報ファイルを読まず、値を出力しません。
- シークレットをリポジトリに置きません。

### 1.6 README の Privacy / Data access と同時に更新する

- README の「Privacy」節と「Data access」節 (英語 80-93 行付近、日本語 190-203 行付近) は、sasurahime が何を読み、何を送り、何を保存するかを利用者に約束する記述です。
- 規則: README の Privacy / Data access の主張を変える可能性がある変更 (新しいファイル内容の読み取り、ネットワーク、保存) では、README (英語・日本語) を同じ PBI/タスクで更新します。後続のタスクや別の PBI に回しません。
  - 「新しいファイル内容の読み取り」には、メタデータ (名前、サイズ、更新日時) 以外を読む変更がすべて入ります。例: 設定ファイルや plist を開いて値を取り出す、外部コマンドにファイルの中身を出力させる (`plutil -convert` など)。
  - 「ネットワーク」は、外部への通信を行う変更すべてです (外部コマンド経由を含む)。
  - 「保存」は、新しいファイルへの書き込み、既存の履歴や設定に新しい項目を足す変更です。
- 既に README に書かれている範囲の中だけで読む変更 (例: 列挙済みの `Info.plist` の bundle ID とアプリ名を別の cleaner も使う) は、README の更新は不要です。その場合も、設計書に「README の該当文がこの変更を包含している」ことを確認する手順を書きます。
- 判断に迷う場合は、実装を止めて報告します。

## 2. Cleaner trait と関連型 (src/cleaner.rs)

以下は基準コミットの実コードです。04 の Task S2 で trait に `auto_clean_allowed()` と `advisory()` が追加されます (2.2 節)。

```rust
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum ScanStatus {
    /// Bytes available to reclaim.
    Pruneable(u64),
    Clean,
    NotFound,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub name: &'static str,
    pub status: ScanStatus,
    pub primary_target: Option<String>,
}

impl ScanResult {
    pub fn new(name: &'static str, status: ScanStatus) -> Self;
    pub fn with_target(mut self, target: impl Into<String>) -> Self; // sets primary_target
}

#[derive(Debug)]
pub struct SkippedEntry {
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug)]
pub struct CleanResult {
    pub name: &'static str,
    pub bytes_freed: u64,
    pub uses_trash: bool,
    pub skipped: Vec<SkippedEntry>,
    pub deleted_paths: Vec<PathBuf>,
}

impl CleanResult {
    // bytes_freed == 0 かつ skipped が空でないとき 1、それ以外は 0
    pub fn exit_code(&self) -> i32;
}

#[non_exhaustive]
#[derive(Debug, Clone, Default)]
pub struct CleanOptions {
    pub all: bool,
}

pub fn is_skippable_error(e: &anyhow::Error) -> bool;

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    /// Read-only. Never deletes anything.
    fn detect(&self) -> ScanResult;
    /// Performs cleanup. When `dry_run` is true, must not delete anything.
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
    /// Default impl calls `clean()` ignoring opts.
    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.clean(dry_run, reporter)
    }
    /// Defaults to true; override to skip unavailable cleaners during scan.
    fn is_available(&self) -> bool { true }
    /// Sub-targets with display names and sizes (for TUI/CLI).
    fn sub_targets(&self) -> Vec<(&'static str, u64)> { vec![] }
}
```

### 2.1 挙動の要点

- `ScanStatus` は `Pruneable(u64)` / `Clean` / `NotFound` の 3 種類だけです。`PermissionDenied` は存在しません。追加しません。
  - 対象ディレクトリが存在しない、または外部ツールがない: `NotFound`。
  - 存在するが削除候補が 0 バイト: `Clean`。
  - 候補がある: `Pruneable(合計バイト)`。
- `ScanResult.name` は `Cleaner::name()` と同じ `&'static str` で、CLI のターゲット名 (例: `library-logs`) と一致させます。
- `with_target` は `crate::context::is_verbose()` が true のときだけ付けます (library_logs.rs の `detect` が手本)。
- `CleanOptions` は `#[non_exhaustive]` です。クレート内からは `CleanOptions { all }` と書けます (library_logs の呼び出しが手本)。フィールドを増やす指示は設計書にない限りしません。
- `CleanResult.exit_code()` は「何も解放できず、skipped がある」ときに 1 を返します。`run_and_exit` (main.rs) がこれを見て `process::exit(1)` します。したがって、削除に失敗した対象は `skipped` に載せる必要があります (eprintln だけで握りつぶすと exit 0 になります)。
- `is_skippable_error(e)` が true になる条件:
  - `e` が `std::io::Error` で、kind が `PermissionDenied` / `WouldBlock` / `AlreadyExists` のいずれか。
  - それ以外は、`format!("{e:#}")` が `trash failed` を含む、または `Permission denied` / `Operation not permitted` / `Resource busy` で始まる。
  - `trash::delete_path` の失敗は `anyhow!("trash failed: ...")` なので skippable になります。permanent モードの失敗 (`remove_dir_all ...`) は skippable になりません。
- `Cleaner` は `Send + Sync` です。フィールドは `Send + Sync` を満たす型にしてください (`Box<dyn CommandRunner>` は満たす。クロージャは `Box<dyn Fn(..) + Send + Sync>`)。
- `scan` は rayon で `detect()` を並列実行します。`detect()` はスレッドセーフかつ副作用なしにしてください。

### 2.2 04 の実装後に trait へ加わるメソッド

04 の Task S2 が `src/cleaner.rs` の `trait Cleaner` に次の 2 つを追加します (`fn sub_targets` の直前)。02/03/05 はこれが存在する前提で書かれています。

```rust
    /// Whether `--yes` (run_auto) may clean this target. Targets that run
    /// destructive external commands or delete user data return false and are
    /// cleaned only when explicitly selected.
    fn auto_clean_allowed(&self) -> bool {
        true
    }

    /// Read-only note shown under the scan table. Must not run external commands.
    fn advisory(&self) -> Option<String> {
        None
    }
```

`--yes` (引数なし起動の `run_auto`) で実行させない仕組みは、この `auto_clean_allowed()` だけです。

- 04 の S3 以降、`all_cleaners()` は `AUTO_CLEAN_TARGETS` (`src/main.rs`) に名前が無い cleaner を `ManualOnly` で包みます。`ManualOnly` は `auto_clean_allowed()` で `false` を返します。
- `run_auto` は `auto_clean_indices()` (`src/interactive.rs`) で `auto_clean_allowed()` が `false` のものを除外します。TUI (`run_interactive`) と `clean <target>` は影響を受けません。
- 02/03/05 の新しい cleaner は、自分の `impl Cleaner` で次のようにオーバーライドします (`ManualOnly` で包まれるので二重ですが、`AUTO_CLEAN_TARGETS` に誤って追加された場合の防御になります。04 の単体テスト `only_legacy_targets_are_auto_clean_allowed` がその誤りを検出します)。

  ```rust
      fn auto_clean_allowed(&self) -> bool {
          false
      }
  ```

- 除外リスト (`INTERACTIVE_ONLY` など) や独自の除外関数を `main.rs` / `interactive.rs` に作りません。
- `AUTO_CLEAN_TARGETS` に新しい名前を追加しません。

## 3. CommandRunner と fake runner

### 3.1 CommandRunner (src/command.rs)

```rust
pub trait CommandRunner: Send + Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<Output>;
    /// Returns true if `program` is available in PATH.
    fn exists(&self, program: &str) -> bool;
}
pub struct SystemCommandRunner;
```

- `SystemCommandRunner::run` は 30 秒でタイムアウトして kill し、Err を返します。プログラムが存在しない場合も Err です (終了コード非 0 は Ok(Output) で、`output.status.success()` が false になる点と区別してください)。
- 外部ツールが PATH にないときは `exists()` を先に確認し、`ScanStatus::NotFound` を返します (エラーにしません)。
- 外部コマンドは必ず注入された `runner` 経由で呼びます。`std::process::Command` を直接呼ぶのは禁止です (custom.rs と pre_commit.rs に直接呼びの例外がありますが、真似しません)。

### 3.2 MockRunner (src/test_helpers.rs、`#[cfg(test)]` の `pub(crate) mod test_helpers`)

```rust
let runner = crate::test_helpers::MockRunner::new().with_success("chflags");
let runner = crate::test_helpers::MockRunner::new().with_not_found();
```

- `with_success(program)`: そのプログラムが存在し、空 stdout / 終了コード 0 で成功します。
- `with_not_found()`: すべて not found (`exists` は false、`run` は Err)。
- 登録していないプログラムの `run` は `unexpected program` で Err になります。削除系 cleaner のテストでは必ず `with_success("chflags")` を付けます。
- MockRunner は stdout を返せません。stdout を返す必要があるテストは 3.3 の自作 runner を使います。

### 3.3 fake runner の自作 (手本は src/cleaners/apfs_snapshot.rs の tests モジュール)

```rust
use std::os::unix::process::ExitStatusExt;
use std::sync::{Arc, Mutex};

fn ok_output(stdout: &[u8]) -> std::process::Output {
    std::process::Output {
        status: std::process::ExitStatus::from_raw(0),
        stdout: stdout.to_vec(),
        stderr: vec![],
    }
}

type Calls = Arc<Mutex<Vec<Vec<String>>>>;

struct Recording {
    calls: Calls,
    delete_status: i32, // 0 = success
}
impl CommandRunner for Recording {
    fn run(&self, _: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
        self.calls
            .lock()
            .unwrap()
            .push(args.iter().map(|s| s.to_string()).collect());
        if args.contains(&"listlocalsnapshots") {
            return Ok(ok_output(b"...list output..."));
        }
        Ok(std::process::Output {
            // 非 0 の終了コードは code << 8 で作る
            status: std::process::ExitStatus::from_raw(self.delete_status << 8),
            stdout: vec![],
            stderr: b"must be root".to_vec(),
        })
    }
    fn exists(&self, _: &str) -> bool {
        true
    }
}
```

使い方:

```rust
let calls: Calls = Arc::new(Mutex::new(vec![]));
let runner = Recording { calls: Arc::clone(&calls), delete_status: 0 };
let cleaner = ApfsSnapshotCleaner::new(Box::new(runner));
// ... 実行 ...
assert_eq!(*calls.lock().unwrap(), vec![vec!["deletelocalsnapshots".to_string(), "2026-05-10-120000".to_string()]]);
```

- 呼び出しは `Arc<Mutex<Vec<..>>>` に記録し、テスト側で期待する引数列と比較します。
- 「ツール不在」は `exists` が false を返す struct (apfs_snapshot.rs の `TmutilMissing`)、「実行エラー」は `run` が `anyhow::bail!` する struct (`ErrRunner`) で作ります。

### 3.4 注入可能なクロージャ (apfs_snapshot.rs の free_space)

実機の状態に依存する値 (空き容量など) は、フィールドに `Box<dyn Fn() -> Option<u64> + Send + Sync>` として持たせ、テスト専用のビルダーで差し替えます。

```rust
pub struct ApfsSnapshotCleaner {
    pub runner: Box<dyn CommandRunner>,
    all: bool,
    free_space: Box<dyn Fn() -> Option<u64> + Send + Sync>,
}
impl ApfsSnapshotCleaner {
    pub fn new(runner: Box<dyn CommandRunner>) -> Self { /* free_space: Box::new(root_free_space) */ }
    pub fn with_all(mut self, all: bool) -> Self { self.all = all; self }
    #[cfg(test)]
    fn with_free_space(mut self, f: impl Fn() -> Option<u64> + Send + Sync + 'static) -> Self {
        self.free_space = Box::new(f);
        self
    }
}
```

- 本番用のビルダー (`with_all`) は `pub`、テスト専用の差し替え (`with_free_space`) は `#[cfg(test)]` を付けます。
- テストでは `Arc<AtomicU64>` などをクロージャに move して、呼び出しごとに値を変えます。

## 4. 削除の作法

### 4.1 削除は trash::delete_path だけを使う (src/trash.rs)

```rust
pub fn set_trash_mode(enabled: bool);
pub fn is_trash_mode() -> bool;
pub fn delete_path(path: &Path) -> Result<()>;
```

- `TRASH_MODE` は `static AtomicBool` で、既定は true (ゴミ箱へ移動)。`--permanent` または config の `trash_mode = false` で false になります (main.rs の `main()` で設定)。
- `delete_path`: trash モードなら `trash::delete`、失敗は `anyhow!("trash failed: ...")`。permanent モードならディレクトリは `remove_dir_all`、ファイルは `remove_file`。
- `std::fs::remove_*` を cleaner から直接呼ぶのは禁止です。必ず `crate::trash::delete_path(&path)` を使います。
- `CleanResult.uses_trash` は、実際に削除を試みたときは `true` (library_logs の手本どおり) にします。何もしなかった場合は `false` です。このフィールドはどこからも読まれていない (`#[allow(dead_code)]`) ため、設計書のコードが `crate::trash::is_trash_mode()` を入れている場合はそのまま写して構いません。

### 4.2 uchg の処理

削除の直前に、runner 経由で `chflags -R nouchg <path>` を実行します。失敗しても削除は続行します。

```rust
let path_str = entry.path.to_string_lossy();
let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
if let Err(e) = crate::trash::delete_path(&entry.path) {
    // 失敗の扱いは 6.3 を参照
}
```

### 4.3 dry-run の約束

- `detect()` は何も削除せず、副作用も持ちません。
- `clean(dry_run = true)` も何も削除しません。`chflags` も実行しません。削除候補を stdout に出力して、`bytes_freed: 0` の `CleanResult` を返します。
- dry-run では対話プロンプト (MultiSelect) も出しません。dry-run の判定はプロンプトより前に置きます (library_logs の `clean` が手本)。
- 書式の慣習: `[<name>] dry-run: N cleanable entries` の見出しの後に、`  would remove: <path>  [<理由>]` を 1 件ずつ出力します。

### 4.4 実機の HOME を触らないテスト

- cleaner は `home: &Path` を引数で受け取り、`std::env::var("HOME")` や `dirs::home_dir()` を cleaner 内部で呼びません。ユニットテストと統合テストは `tempfile::TempDir` を `home` として渡します。
- E2E テスト (tests/*.rs) は `assert_cmd::Command` で実バイナリを起動し、`.env("HOME", tmp.path())` で HOME を差し替えます。`PATH` の制限が必要なら `.env("PATH", "/usr/bin:/bin")` を付けます。
- `std::env::set_var("HOME", ...)` をテスト内で呼ぶのは禁止です (スレッド並列のテストを壊します)。環境変数を変える必要があるときは `crate::test_helpers::EnvGuard::set(key, val)` を使い、その間は 9 章のとおり `TEST_LOCK` を保持します。
- 削除を伴うテストでは、先頭で `crate::trash::set_trash_mode(false)` を呼ぶと、実機のゴミ箱を使わずに済みます (trash.rs の tests が手本)。ただしこれはグローバル状態です。他のテストの前提 (trash モード true) を壊さないよう、`set_trash_mode(false)` を呼んだテストは、関数の最後の行に `crate::trash::set_trash_mode(true);` を書いて戻します。設計書のテストコードにこの行が無くても、`set_trash_mode(false)` を呼ぶテストには必ず足します。
  - library_logs.rs の `clean_all_via_opts_processes_all_entries` のように trash モードを触らない既存テストもあります (実機のゴミ箱に tempdir 内のファイルが入る)。新規テストは前者 (false にして戻す) を推奨します。
- テスト用の古いファイルは `crate::test_helpers::write_aged_file(path, days_old, content)` で作れます。ディレクトリの mtime は `filetime::set_file_mtime` で変更します (library_logs.rs の tests が手本)。

## 5. 新しい cleaner を登録する手順

登録方法は 2 種類あります。

- 標準エントリ (docs/HOWTO-ADD-target.md の Path A): CLI 引数が `--dry-run` だけの cleaner。`define_cleaners!` の表に 1 エントリ足すだけで、`CleanTarget` の変種、`command_name()` / `dry_run()`、`SUPPORTED_TARGETS` (= `sasurahime targets`)、`dispatch_clean`、そして 04 の実装後は `standard_cleaners()` 経由で `all_cleaners()` (scan、TUI) への登録まで自動で行われます。
- 特殊変種 (Path B): `--all` や `--keep` など `--dry-run` 以外の引数を持つ cleaner。手本は `ApfsSnapshot` (`--all` 付き、構造体ビルダー方式) と `LibraryLogs` (`--all` 付き、`CleanOptions` 方式)。

今回の 02 (orphan-containers)、03 (library-caches)、05 (app-support) は 3 つとも `--dry-run` 以外の引数を持たないので、標準エントリで登録します (5.0 節)。特殊変種の手順 (5.1 節) は使いません。どちらで登録しても `--yes` の対象外にする方法は同じで、2.2 節の `auto_clean_allowed()` です。

### 5.0 変更箇所のチェックリスト (標準エントリ、02/03/05 が使う)

前提: 04 の Task S3 が完了していること (`standard_cleaners()` が存在すること)。

1. [ ] `src/cleaners/<module>.rs` を新規作成し、構造体と `impl Cleaner for ...` を書く (6 章)。`impl Cleaner` に `fn auto_clean_allowed(&self) -> bool { false }` を入れる (2.2 節)。テストも同じファイルの `#[cfg(test)] mod tests` に書く。
2. [ ] `src/cleaners/mod.rs` に `pub mod <module>;` をアルファベット順の位置に追加する。
3. [ ] `src/main.rs` の `define_cleaners! {` の表で、標準エントリの最後 (単独の `;` だけの行の直前) に 2 行を追加する。書式は隣の `TreeSitter` エントリと同じです。

   ```rust
       <Variant> : "<cli-name>" => "<説明 (英語、1 行)>";
       (|home, _config| cleaners::<module>::<Struct>::new(home, Box::new(SystemCommandRunner))),
   ```

   これだけで `CleanTarget::<Variant> { dry_run }`、`sasurahime clean <cli-name> [--dry-run]`、`sasurahime targets` の行、`scan` と TUI への登録が揃います。`name()` は `<cli-name>` と同じ文字列にします (04 のテスト `standard_cleaners_follow_the_macro_table_order` が検査します)。
4. [ ] 特殊変種の並び (`;` より後ろ)、`command_name()`、`dry_run()`、`extra_targets()`、`Some(Commands::Clean { target })` の `matches!` と `match target`、`special_cleaners()` には何も書かない。同じ変種を両方に書くと clap が重複で panic します。
5. [ ] `AUTO_CLEAN_TARGETS` には追加しない。
6. [ ] `src/main.rs` の `mod tests` の末尾に、`all_cleaners()` に登録されていて `auto_clean_allowed()` が `false` であることを確かめるテストを 1 件足す (各設計書にコードがあります)。
7. [ ] `tests/<module>.rs` に E2E テストを追加する (5.3)。
8. [ ] `docs/SUPPORTED.md` を更新する (5.2)。
9. [ ] `docs/HOWTO-USE.md` の対象表 (英語と日本語) に 1 行ずつ追加する (位置と文言は各設計書)。
10. [ ] `README.md` の「Data access」節は、各設計書に指示がある場合だけ追記する (11.5 節)。ただし 1.6 節の規則に当たる変更 (新しいファイル内容の読み取り、ネットワーク、保存) を設計書の指示なしに見つけた場合は、実装を止めて報告する。
11. [ ] `cargo build`、`cargo clippy -- -D warnings`、`cargo fmt --check`、`cargo test` を全て通す。

### 5.1 変更箇所のチェックリスト (特殊変種、今回は使わない)

参考として残します。04 の実装後は、手順 7 の `all_cleaners()` を `special_cleaners()` と読み替えます。

以下を上から順に行います。`<Variant>` は Rust の enum 名 (例 `OrphanContainers`)、`<cli-name>` は CLI 名 (例 `orphan-containers`)、`<module>` はファイル名 (例 `orphan_containers`) です。

1. [ ] `src/cleaners/<module>.rs` を新規作成する。構造体と `impl Cleaner for ...` を書く (6 章)。テストも同じファイルの `#[cfg(test)] mod tests` に書く。
2. [ ] `src/cleaners/mod.rs` に `pub mod <module>;` を追加する。ファイルはアルファベット順の `pub mod` 一覧です。
3. [ ] `src/main.rs` の `define_cleaners!` 呼び出し (`define_cleaners! {` は 99 行付近) の、末尾の `;` 行 (217 行付近) より後ろの「特殊 enum 変種」部に変種を追加する。マクロの標準変種の表には書かない。ApfsSnapshot の実物 (275 行付近):

   ```rust
   /// Delete APFS local Time Machine snapshots
   #[command(name = "apfs-snapshot")]
   ApfsSnapshot {
       #[arg(long)]
       dry_run: bool,
       /// Delete every local snapshot without prompting
       #[arg(long)]
       all: bool,
   },
   ```

   `#[command(name = "...")]` は、CLI 名が enum 名の kebab-case と一致しない場合に必須です。迷ったら常に付けます。
4. [ ] `impl CleanTarget` の `command_name()` (362 行付近) の `match` に `CleanTarget::<Variant> { .. } => "<cli-name>",` を追加する。
5. [ ] 同じ `impl CleanTarget` の `dry_run()` (377 行付近) の `match` に `CleanTarget::<Variant> { dry_run, .. } => *dry_run,` を追加する。
6. [ ] `extra_targets()` (394 行付近) の配列に `("<cli-name>", "説明 (英語、1 行)"),` を追加する。これが `sasurahime targets` の出力になる。
7. [ ] `all_cleaners()` (426 行付近) の `vec![ ... ]` に cleaner を追加する。`scan` と引数なし起動 (TUI) と `--yes` の対象になる。

   ```rust
   Box::new(cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
       SystemCommandRunner,
   ))),
   ```

   HOME を使う cleaner は `cleaners::<module>::<Struct>::new(home, Box::new(SystemCommandRunner))` です。
   この後の `cleaners.retain(...)` で config の `exclude` が効くため、`name()` は CLI 名と同じにすること。
8. [ ] `main()` の `Some(Commands::Clean { target })` (714 行付近) の最初の `if matches!(target, ... )` に `| CleanTarget::<Variant> { .. }` を追加する (725 行付近)。ここに足し忘れると `unreachable!` ではなく標準ディスパッチに落ち、実行時に panic します。
9. [ ] その直下の `match target { ... }` に腕を追加する (886 行付近が ApfsSnapshot)。2 つの型がある。
   - 構造体ビルダー方式 (ApfsSnapshot):

     ```rust
     CleanTarget::ApfsSnapshot { dry_run, all } => {
         let cleaner = cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
             SystemCommandRunner,
         ))
         .with_all(all);
         run_and_exit(
             "apfs-snapshot",
             move |dry, rep| cleaner.clean(dry, rep),
             dry_run,
             reporter.as_ref(),
         )?;
     }
     ```

   - `CleanOptions` 方式 (LibraryLogs、848 行付近):

     ```rust
     CleanTarget::LibraryLogs { dry_run, all } => {
         let cleaner = cleaners::library_logs::LibraryLogsCleaner::new(
             &home,
             Box::new(SystemCommandRunner),
         );
         let opts = crate::cleaner::CleanOptions { all };
         run_and_exit(
             "library-logs",
             move |dry, rep| cleaner.clean_with_opts(dry, rep, &opts),
             dry_run,
             reporter.as_ref(),
         )?;
     }
     ```

   どちらを使うかは設計書の指示に従います。指示がなければ、HOME 配下を走査する cleaner は `CleanOptions` 方式です。
   `run_and_exit` の第 1 引数のラベルは `name()` と同じ CLI 名にします (履歴 `history.json` のキーになる)。
10. [ ] `tests/<module>.rs` に E2E テストを追加する (5.3)。
11. [ ] `docs/SUPPORTED.md` を更新する (5.2)。
12. [ ] 必要なら `docs/HOWTO-USE.md` の対象表に追加する。英語の表 (102 行付近) と日本語の表 (674 行付近) の両方 (11.6 節)。
13. [ ] `README.md` は、対象表を持ちません。「Data access」節 (90 行付近の英語、200 行付近の日本語) に、cleaner が新しく読むディレクトリが含まれていなければ追記します (11.5 節)。既に列挙されているパス (`~/Library/Caches/`、`~/Library/Application Support/` など) の配下のメタデータだけを読むなら変更不要です。ファイルの内容を新しく読む場合は 1.6 節の規則に従い、同じタスクで「内容を読むファイル」の一覧を更新します。
14. [ ] `cargo build`、`cargo clippy -- -D warnings`、`cargo fmt --check`、`cargo test` を全て通す。

### 5.2 docs/SUPPORTED.md の更新

- ファイルは英語部と日本語部の 2 部構成です。英語の Quick Reference 表は 17-66 行付近、日本語の表は 1356-1404 行付近です。それぞれの表の `| \`trash\` |` で始まる行の直前に `| \`<cli-name>\` | 説明 |` の 1 行を追加します (結果として `apfs-snapshot`、`orphan-containers`、`library-caches`、`app-support`、`trash` の順になります)。
- 件数の行は英語 10 行目付近の `sasurahime provides **N clean targets**.` と、日本語 1348 行付近の `sasurahime は **N のクリーンターゲット** を提供しています。` の 2 か所です。N は `cargo run -q -- targets | wc -l` の値と一致させます。
- 各ターゲットの節は `## N. \`sasurahime clean <name>\`` の連番です。基準コミットでは clean 系が `## 1.` から `## 48. dotnet`、その次が `## 49. \`sasurahime stats\`` です。新しい clean 節は常に stats 節の直前に入れ、stats の見出し番号を 1 つ繰り下げます。挿入後に、直前の clean 節と新節の間、新節と stats 節の間がそれぞれ `---` 1 本で区切られていれば正しい状態です (`---` を新節の先頭に付けるか末尾に付けるかは各設計書の指示どおり)。日本語部にも同じ節を作ります。
- 節番号と件数は実装順に次のとおり固定です (ロードマップ 1 章)。

  | 段階 | 件数 (英日の件数行) | 追加する節 | stats の番号 |
  |------|--------------------|-----------|-------------|
  | 基準コミット | 49 (誤り。実際は 48) | なし | 49 |
  | 04 完了後 | 48 | なし (docker 節に段落追加) | 49 |
  | 02 完了後 | 49 | `## 49. orphan-containers` | 50 |
  | 03 完了後 | 50 | `## 50. library-caches` | 51 |
  | 05 完了後 | 51 | `## 51. app-support` | 52 |

- 節の形式は `## 45. apfs-snapshot` (1145 行付近) が手本です。固定の見出し: `**Category:**`、`**What it removes:**`、`**How detect works:**`、`**How clean works:**`、`**Safety:**`。今回の 3 節の Category は `Sprint 6` です (apfs-snapshot と同じ)。
- 文章は最新仕様のみ書きます。経緯や履歴は書きません。
- 既存の表には EM DASH が残っていますが、追記する行では ASCII の `-` を使います。

### 5.3 tests/ への E2E 追加

- ファイル名は `tests/<module>.rs`。`tests/library_logs.rs` と `tests/apfs_snapshot.rs` が手本です。
- 雛形:

  ```rust
  use assert_cmd::Command;
  use std::fs;
  use tempfile::TempDir;

  fn sasurahime(home: &std::path::Path) -> Command {
      let mut cmd = Command::cargo_bin("sasurahime").unwrap();
      cmd.env("HOME", home);
      cmd
  }
  ```

- 最低限入れる 4 本:
  1. `clean <cli-name> --dry-run` が、fixture の候補を stdout に出し、終了コード 0 で、fixture が残っていること。
  2. HOME に対象ディレクトリがない場合に、終了コード 0 で落ちないこと。
  3. `clean <cli-name> --all` (ある場合) で fixture が消えること。
  4. `scan` の stdout に `<cli-name>` が含まれること。
- E2E テストでは TTY がないため、対話選択 (MultiSelect) を通る経路は試験できません。`--dry-run` か `--all` を使います。
- `tests/coverage.rs` が自動で検証すること: `sasurahime targets` に出た全名前に対して `clean <name> --help` が成功する、かつ名前が重複しない。登録漏れ (`extra_targets` の追加忘れや重複) はここで検知されます。
- `sasurahime targets` の出力は `{:<12} {}` 形式で名前順にソートされます (12 文字より長い名前でも崩れません)。

## 6. HOME 配下をスキャンして選択して削除する cleaner の標準形 (src/cleaners/library_logs.rs)

新しい 3 つの cleaner (02/03/05) はこの形に従います (04 は cleaner を新設しません)。

### 6.1 構造

```rust
pub struct LibraryLogsCleaner {
    pub home: PathBuf,
    pub keep_days: u64,
    pub size_threshold: u64,
    pub runner: Box<dyn CommandRunner>,
}

impl LibraryLogsCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self { ... }
    fn logs_dir(&self) -> PathBuf { self.home.join("Library/Logs") }
    pub(crate) fn scan(&self) -> Vec<LogEntry> { scan_logs(&self.logs_dir(), self.keep_days, self.size_threshold) }
}
```

- エントリ型 (`LogEntry { path, size, last_modified, reasons }`) と理由の enum (`DeleteReason`) を持ちます。
- 走査は純粋関数 (`scan_logs(dir, keep_days, size_threshold) -> Vec<LogEntry>`、判定は `classify_entry`) に切り出し、引数で閾値を受け取ります。テストで閾値を `1` や `u64::MAX` にして境界を作ります。
- 走査の約束:
  - ディレクトリが存在しない、読めない: 空 Vec を返します (エラーにしない)。
  - 名前が `.` で始まるエントリは除外します。
  - 除外名の定数配列 (`SKIP_DIRS`) を持ちます。
  - サイズは `crate::format::dir_size(&path)` で測ります (物理ブロック数 `st_blocks * 512`)。サイズ 0 のエントリは除外します。
  - mtime は `symlink_metadata` で取得し、未来の mtime は現在時刻に丸めます (未来のファイルを stale と誤判定しない)。

### 6.2 detect

- 対象ディレクトリがない: `ScanStatus::NotFound`。
- 候補の合計が 0: `ScanStatus::Clean`。それ以外: `ScanStatus::Pruneable(合計)`。
- verbose のときだけ `.with_target(dir)` を付ける。

### 6.3 clean と clean_with_opts

`clean(dry_run, reporter)` の流れ:

1. 対象ディレクトリがない: 何も出力せず、`bytes_freed: 0` を返す。
2. `scan()` が空: `[<name>] nothing to clean` を出して `bytes_freed: 0` を返す。
3. `dry_run`: 4.3 の書式で出力して `bytes_freed: 0` を返す。
4. `interactive_select(&entries)?` で選択させる。選択が空なら `[<name>] nothing selected` を出して `bytes_freed: 0` を返す。
5. `reporter.progress_init(name, selected.len())` の後、各エントリごとに `reporter.progress_tick(&path, i + 1, size)`、`chflags`、`crate::trash::delete_path` を行い、最後に `reporter.progress_finish()`。
6. 合計解放量を `[<name>] total freed: ...` で出力して返す。

`clean_with_opts(dry_run, reporter, opts)`:

- `opts.all == false` なら `self.clean(dry_run, reporter)` に委譲する。
- `opts.all == true` なら、`interactive_select` を飛ばして全候補を対象にする。dry-run の扱いは `clean` と同じ。

失敗の扱い:

- 既存の library_logs.rs は削除失敗を `eprintln!` するだけで `skipped` に載せていません (`skipped: vec![]`)。これは新規コードでは真似しません。
- 新規コードの規約: `delete_path` が Err のとき、`crate::cleaner::is_skippable_error(&e)` が true なら `SkippedEntry { path, reason: e.to_string() }` を `skipped` に push し、false でも同様に push した上で `eprintln!` で理由を出す。全件失敗すると `bytes_freed == 0 && !skipped.is_empty()` となり、`exit_code()` が 1 を返す (apfs_snapshot.rs の失敗分を `skipped` に載せる実装が手本)。
- 失敗しても残りのエントリの処理は続行します。途中で `?` で抜けません。

### 6.4 MultiSelect

```rust
use dialoguer::MultiSelect;
let defaults: Vec<bool> = vec![true; entries.len()];
let selections = MultiSelect::new().items(&items).defaults(&defaults).interact()?;
Ok(selections.into_iter().map(|i| entries[i].clone()).collect())
```

- `items` は表示用の `Vec<String>` で、`"{:<24}  {}  [{}]"` (名前、サイズ、理由) の書式です。
- library_logs は既定を全選択 (`defaults(true)`) にしています。今回の 3 つの cleaner (02/03/05) は、どれも既定を全部未選択にします (02 は `.defaults` を呼ばない、03 と 05 は `vec![false; n]`)。library_logs からコピーするときに `true` を残さないでください。
- `interact()` は TTY がないとエラーを返します。対話選択を行う前に `std::io::stdin().is_terminal()` (`use std::io::IsTerminal;`) を確認し、TTY でなければ `[<name>] not a terminal` と案内を `eprintln!` して `bytes_freed: 0` を返します (ios_backup.rs と apfs_snapshot.rs が手本。`--all` 指定時はこのガードを外す)。今回の 3 つの cleaner にもこのガードを入れます。
- `sasurahime --yes` で実行させないことは、このガードではなく `auto_clean_allowed()` (2.2 節) で保証します。TTY 上で `--yes` を付けた場合、ガードだけではプロンプトが出てしまうためです。

### 6.5 --all

- `--all` は「プロンプトを出さず、候補全部を削除する」明示的な opt-in です。dry-run は `--all` よりも優先されます (`--all --dry-run` は削除しない)。

## 7. src/hint/ と src/explorer.rs の公開 API

### 7.1 src/hint/mod.rs

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BaseDir {
    Caches,     // <home>/Library/Caches
    AppSupport, // <home>/Library/Application Support
    Logs,       // <home>/Library/Logs
}

pub struct HintEntry {
    pub base_dir: BaseDir,
    /// One or more subdirectory paths (relative to base_dir) to sum for size.
    pub path_suffixes: &'static [&'static str],
    pub display_name: &'static str,
    pub process_name: Option<&'static str>,   // pgrep -x で実行中判定に使う
    pub commands: &'static [&'static str],    // ユーザーに表示するシェルコマンド
    pub threshold_bytes: u64,
    pub skip: bool,                           // true なら collect_hints の対象外 (OS 管理など)
    pub quit_app: Option<&'static str>,       // osascript の quit app 名。None は自動終了不可
    pub relaunch_app: Option<&'static str>,   // 削除後に open -a する名前。None は再起動しない
}

pub struct ProcessHint {
    pub entry: &'static HintEntry,
    pub size_bytes: u64, // path_suffixes の合計
    pub running: bool,
}

pub trait PromptReader { fn read_line(&self) -> Option<String>; }
pub struct StdinPrompt;

pub use cleaner::{offer_auto_clean, print_hints};
pub use detector::collect_hints;
fn base_path(home: &Path, base: BaseDir) -> PathBuf; // private
```

- `base_path` は `hint/mod.rs` と `hint/detector.rs` の両方に private で重複して存在します。外部から呼べません。必要なら設計書の指示に従って `pub(super)` 化や共有化を行います。勝手に統合しません。

### 7.2 src/hint/apps.rs のエントリ構造

`pub static KNOWN_ENTRIES: &[HintEntry]` に `HintEntry { ... }` のリテラルを並べます (全フィールド必須)。実例:

```rust
HintEntry {
    base_dir: BaseDir::AppSupport,
    path_suffixes: &["Slack/Cache"],
    display_name: "Slack",
    process_name: Some("Slack"),
    commands: &["rm -rf ~/Library/Application\\ Support/Slack/Cache"],
    threshold_bytes: 64 * MB,   // const MB: u64 = 1024 * 1024;
    skip: false,
    quit_app: Some("Slack"),
    relaunch_app: None,
},
```

- 先頭にセクションコメント (`// ── ~/Library/Caches/ ──`、`// ── ~/Library/Application Support/ ──`) があり、base_dir ごとに並べてあります。追加するときは同じセクションに入れます。
- OS 管理のため触らないものは `skip: true`、`commands: &[]` にします (例: `GeoServices`)。

### 7.3 src/hint/detector.rs

```rust
pub fn collect_hints(home: &Path, runner: &dyn CommandRunner) -> Vec<ProcessHint>;
fn dir_size_bytes(path: &Path, runner: &dyn CommandRunner) -> u64;   // private、`du -sk <path>` の 1 列目 * 1024
fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool; // private、`pgrep -x <name>` の成功可否
```

- `collect_hints` は `skip == false` のエントリのうち、`size_bytes > threshold_bytes` のものを、サイズ降順で最大 5 件返します (`truncate(5)`)。
- 実行中判定は `pgrep -x <process_name>` が成功 (終了コード 0) かどうかです。`process_name` が `None` のエントリは常に `running = false` です。
- 実行中判定の再利用は次の形に決めてあります (03 の Task 1 が行い、05 はそれを使うだけです)。
  - `src/hint/detector.rs`: `pub(crate) fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool` (本体は変えない)。
  - `src/hint/mod.rs`: `pub use detector::collect_hints;` の次の行に `pub(crate) use detector::is_running;` を追加。
  - `mod detector;` と `mod cleaner;` は private のまま (`pub(crate) mod` にしない)。
  - 呼び出し側は `crate::hint::is_running(name, runner)` と書きます。`crate::hint::detector::is_running` とは書きません (モジュールが private なので通りません)。
  - `pub use detector::{collect_hints, is_running};` のように `pub use` で再エクスポートしません。`pub(crate)` の関数は `pub use` できずコンパイルエラーになります。

### 7.4 src/hint/cleaner.rs

```rust
pub fn auto_clean_hint(/* ... */);                                  // 1 件の hint を自動削除する (テストから呼ばれる)
pub fn offer_auto_clean(/* hints, home, runner, prompt */);         // main.rs から呼ばれる
pub fn print_hints(hints: &[ProcessHint]);
```

- シグネチャの正確な引数は `src/hint/cleaner.rs` の 11 行付近 (`auto_clean_hint`)、102 行付近 (`offer_auto_clean`)、146 行付近 (`print_hints`) で確認してください。呼び出し側 (`main.rs` の `Some(Commands::Scan)`) は `hint::collect_hints(&home, &runner)` -> `hint::print_hints(&hints)` -> `hint::offer_auto_clean(&hints, &home, &runner, &hint::StdinPrompt)` の順です。
- 05 の Task 1 が `auto_clean_hint` から次の 2 関数を抽出します (挙動と出力文言は不変)。公開は `src/hint/mod.rs` の `pub use cleaner::{offer_auto_clean, print_hints};` の次の行に `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` を足す形で行い、呼び出し側は `crate::hint::quit_app_and_wait` / `crate::hint::relaunch_app` と書きます。

  ```rust
  pub(crate) fn quit_app_and_wait(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()>;
  pub(crate) fn relaunch_app(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()>;
  ```

### 7.5 src/explorer.rs

```rust
struct ManagedPattern {
    path_pattern: &'static str,   // "~/" で始まる。末尾が '*' なら前方一致 (ms-playwright* など)
    clean_target: &'static str,   // 管理している cleaner の CLI 名
}
static MANAGED_PATTERNS: &[ManagedPattern] = &[ /* 18 件 */ ];

#[derive(Debug)]
pub struct ExploreEntry {
    pub path: PathBuf,
    pub size: u64,                      // dir_size と同じ物理バイト
    pub managed: Option<&'static str>,  // 管理する cleaner 名
}

pub struct ExploreOptions {
    pub roots: Vec<PathBuf>,
    pub top: Option<usize>,   // None = 全件
    pub dry_run: bool,
}

pub fn default_roots(home: &Path) -> Vec<PathBuf>;
// 返り値: <home>/Library/Application Support, <home>/Library/Caches, <home>/.cache, <home>/.local/share
pub fn run_explore(home: &Path, opts: ExploreOptions) -> Result<()>;

fn expand_tilde(pattern: &str, home: &Path) -> PathBuf;                  // private
fn is_managed(path: &Path, home: &Path) -> Option<&'static str>;         // private
```

- 現在の `MANAGED_PATTERNS` の `clean_target` は uv / browsers / brew / mise / cargo / colima / xcode / simulator / ios-backup / huggingface / act / gradle / maven / sbt / pre-commit です。
- 新しい cleaner が既存パスを管理するようになったら、対応する `ManagedPattern` を追加します (explore で「managed by sasurahime clean X」と表示されるため)。追加は設計書が指示したときだけ行います。
- `ManagedPattern` は private なので、クレート外には出ません。`path_pattern` の `~/` は `home.join(...)` で展開されます。
- `explorer.rs` の private 関数 `open_in_finder(path)` は `std::process::Command::new("open")` を直接呼びます。`run_explore` は未管理エントリを MultiSelect で選ばせた後、1 件ごとに `Confirm` ("Open in Finder?"、既定 No) を挟んでから `open_in_finder` を呼びます。
- 05 (app-support) の第 2 段は Finder 起動を `open_in_finder` ではなく runner 経由の `open <path>` で行い、確認は 6 件以上のときだけ 1 回出します (05 の 4.3 節)。explorer と二重実装になるのは意図したものです (runner 経由でテストできること、今回 `explorer.rs` を変更しないこと)。共通化は別 PBI です (ロードマップ 8 章)。今回の 4 本では `explorer.rs` を変更しません。

### 7.6 共通ユーティリティ (src/format.rs)

```rust
pub fn dir_size(path: &std::path::Path) -> u64;   // 物理ブロック (st_blocks * 512) の合計。存在しなければ 0
pub fn format_bytes(bytes: u64) -> String;        // "1.5 GB" / "12.3 MB" / "4.0 KB" / "N B"
```

## 8. テストコマンド集

作業ディレクトリは `/Users/yaar/Playground/sasurahime` です。

| 目的 | コマンド | 期待出力 |
|------|----------|----------|
| 単体テスト 1 件 (部分一致) | `cargo test <test_name>` | `test result: ok. N passed; 0 failed` (N は 1 以上。0 件だとテスト名の誤り) |
| cleaner のテスト群 | `cargo test cleaners::<module>::` | 同上 |
| E2E テストファイル | `cargo test --test <file>` (例 `cargo test --test library_logs`) | 同上 |
| 全テスト | `cargo test` | 全ての `test result:` 行が `ok`。基準コミットでは `cargo test --bin sasurahime` の単体テストが 364 件 (`cargo test --bin sasurahime -- --list` で数えた値) |
| lint | `cargo clippy -- -D warnings` | 警告もエラーもなく `Finished` で終わる |
| フォーマット確認 | `cargo fmt --check` | 出力なしで終了コード 0 |
| フォーマット適用 | `cargo fmt` | 出力なし |
| ビルド | `cargo build` | `Finished` |

- 基準コミットで `cargo test` は全件成功、`cargo clippy -- -D warnings` と `cargo fmt --check` は無警告で通ります。作業開始時にこの 3 つを一度実行し、緑であることを確認してください。
- 既知の既存失敗: `cargo clippy --all-targets` は `src/cleaner.rs` の 60 行付近で既に失敗します。今回の作業と無関係なので直しません。`--all-targets` は使わず、`cargo clippy -- -D warnings` のみを品質ゲートとします。
- 失敗するテストを先に書く段階では、`cargo test <test_name>` が「コンパイルエラー」または「assertion failed」で失敗することを確認します。0 件実行 (`0 passed`) は失敗の確認になりません。

## 9. 禁止事項

1. 実機の `~/Library`、`~/.cache` などを読み書きするテストを書かない。必ず `TempDir` を HOME (cleaner の `home` 引数) として使う。
2. `sudo` を実行しない。root が必要な操作は、実行するコマンドを文字列として案内するだけにする (apfs_snapshot.rs の `sudo_hint` が手本)。
3. `git add -A` / `git add .` を使わない。コミットは変更ファイルを個別に指定する。
4. `std::fs::remove_dir_all` / `remove_file` を cleaner から直接呼ばない。必ず `crate::trash::delete_path` を通す。
5. `std::process::Command` を cleaner から直接呼ばない。`CommandRunner` を注入する。
6. プロダクションコード (テスト以外) で `unwrap()` を使わない。`?`、`match`、`unwrap_or`、`let ... else` を使う。やむを得ず不変条件を表明する場合のみ `expect("理由")` とする。テストコード内の `unwrap()` は許可。
7. グローバル状態を触るテストで `crate::context::TEST_LOCK` を取らずに実行しない。対象は、`crate::context::set_verbose` / `set_dry_run` を呼ぶテスト、`EnvGuard` を使うテスト、`VerboseGuard` を使うテスト (これは内部でロックを取る)。
   - 取り方: `let _guard = crate::context::TEST_LOCK.lock().unwrap();` を先頭に置く (`TEST_LOCK` は `pub(crate)` で `#[cfg(test)]` のみ)。保持中に別のロックを取らない。
   - `set_trash_mode` は `TEST_LOCK` の対象外ですが、並列テストで競合するため、使うテストは数を最小にします。
8. `ScanStatus` にバリアントを足さない。`CleanOptions` にフィールドを足さない。`Cleaner` trait を変更しない。例外は 04 の Task S2 (trait に `auto_clean_allowed()` と `advisory()` を追加) だけです。02/03/05 は trait を変更せず、`auto_clean_allowed()` を自分の `impl` でオーバーライドするだけです。
9. detect() / dry-run で削除、chflags、ファイル書き込みをしない。
10. 対話プロンプト (dialoguer) を dry-run や E2E の経路で出さない。TTY がない環境で止まるため。
11. 新しい依存クレートを追加しない。
12. 設計書にない仕様変更、リファクタリング、無関係ファイルの整形をしない。`cargo fmt` で無関係ファイルに差分が出たら、そのファイルは `git checkout` ではなく報告して止まる。
13. 絵文字と 1.1 に挙げた文字をドキュメントに入れない。コメントに WHAT / 履歴 / タスク ID を書かない。
14. `.env` や認証情報ファイルを読まない。

## 10. 実装者への進め方

1. 設計書のタスク一覧を上から 1 つずつ処理する。複数タスクを同時に進めない。
2. 各タスクで次の 5 段階を必ず守る。
   1. 失敗するテストを書く。`cargo test <name>` を実行して、期待どおりの理由 (未実装によるコンパイルエラー、または assertion の失敗) で失敗することを確認する。
   2. 最小限の実装を書く。
   3. `cargo test <name>` が通ることを確認する。続けて `cargo test` 全体、`cargo clippy -- -D warnings`、`cargo fmt --check` を通す (clippy の途中タスクでの例外は 1.4 のとおり)。
   4. 変更したファイルを個別に `git add <path>` する。`git status` で意図しないファイルが含まれていないことを確認してから、日本語の Conventional Commits でコミットする。
   5. 設計書のタスク一覧の次へ進む。
3. 実装順の目安 (HOME 配下スキャン型): 純粋関数 (判定、パース) のユニットテスト -> `detect` -> `clean(dry_run = true)` -> `clean` 本体 (`TempDir` 上で削除を検証) -> main.rs への登録 (5.0) -> E2E -> docs。
4. 詰まったとき (次のいずれか) は、それ以上進めずに止まって報告する。
   - 同じ失敗が 2 回の修正で直らない。
   - 設計書とコードベースの記述が食い違う (関数がない、シグネチャが違う等)。
   - 設計書に書かれていない判断が必要になる (名前、閾値、除外リスト、メッセージ、削除範囲など)。
   - 既存テストが自分の変更で壊れ、原因がすぐ分からない。
   - `cargo clippy` / `cargo fmt` が既存コードに起因して失敗する。
   報告には、実行したコマンド、出力の要点、どのタスクのどの手順かを含める。推測で進めない。
5. 設計書にない判断をしない。迷ったら止まって報告する。便利そうな追加機能、リネーム、周辺コードの整理も行わない。
6. 最後に、全タスク完了後に `cargo test`、`cargo clippy -- -D warnings`、`cargo fmt --check` を再実行し、結果を報告する。

## 11. 設計書の間で共通する判断

各設計書はこの章を参照します。設計書の本文と食い違って見えたら、止まって報告してください。

### 11.1 sub_targets() は実装しない

- 02/03/05 の cleaner は `sub_targets()` をオーバーライドしません (既定の空 Vec)。
- 理由: TUI (`run_interactive`) は `sub_targets()` が空でない cleaner を、子プロセス `sasurahime clean <name> --sub <sub>` で実行します。`--sub` 引数を持つのは xcode だけなので、他の cleaner で実装すると TUI から選んだときに失敗します。
- 一覧の細目は `clean <name> --dry-run` の出力で見せます。

### 11.2 plist は依存クレートを追加せずに読む

- `plist` クレートなどを `Cargo.toml` に追加しません。
- `Info.plist` を読むのは 02 の `src/cleaners/orphan_containers.rs` だけです。05 は自前で plist を読まず、02 の関数を呼びます。
- 02 は `Info.plist` から `CFBundleIdentifier`、`CFBundleName`、`CFBundleDisplayName` の 3 つの値だけを取り出し、型 `AppIdentity { bundle_id, name, display_name }` (3 項目のみ) で返します。他のキーの値を返す関数、ログに出す経路、保存する経路を作りません (README の約束。1.6 節)。
  - XML の `Info.plist` は文字列検索で 3 キーを取り出します。バイナリ (`bplist00` で始まる) は runner 経由の `plutil -convert xml1 -o - <path>` で XML を標準出力に出させ、同じ文字列検索で 3 キーを取り出します。`-o -` を省くと `plutil` が元のファイルを書き換えるので、必ず付けます。
  - 関数は `orphan_containers::read_app_identity` (1 ファイル)、`collect_app_identities` (インストール先の全 `.app`) と、それを小文字の bundle ID 集合にする `collect_installed_ids` です。
- 05 (アプリ名の取得): 02 の `collect_app_identities` が返す `AppIdentity` の `name` と `display_name`、`.app` のファイル名の stem だけを使います。`plutil -convert json` と `serde_json` による plist の解析はしません。
- `plutil` は macOS 標準で、runner 経由なのでテストで差し替えられます。

### 11.3 サイズは dir_size (物理ブロック) で測る

- サイズは `crate::format::dir_size(&path)` (物理ブロック `st_blocks * 512` の合計) で測ります。論理サイズ (`metadata().len()`) は使いません。例外は 04 の `disk_image::probe` だけで、疎ファイルの最大サイズ表示のために論理サイズを併記します。
- テストの閾値やサイズ比較は 4 KB 単位の丸めで順序が変わらない値にします。
- サイズ 0 の候補は一覧に出しません。

### 11.4 detect() は出力も対話もしない

- `detect()` は `println!` / `eprintln!` をしません (並列 scan の表示が崩れます)。案内や警告は `clean()` で出します。
- `detect()` で `dialoguer` を使いません。`pgrep` (実行中判定) や `osascript` も呼びません。読み取り専用の外部コマンド (02 の `plutil`、既存の `rustup toolchain list`) は runner 経由なら許容します。

### 11.5 実機の /Applications と README の Data access

- 単体テストは実機の `/Applications` に依存させません。インストール先は注入します (02 の `with_app_dirs`、05 の `with_app_roots`、どちらも `#[cfg(test)]`)。E2E が実機の `/Applications` を読み取りだけ行うのは許容します。
- `README.md` の「Data access」節は、ロードマップ 7 章の前提タスク P0 で「ディレクトリの一覧」「メタデータを読む旨」「内容を読むファイルの一覧」「読んだ値をログと削除履歴に書き出さない旨」の形に書き直します。P0 の時点で「内容を読むファイルの一覧」に入るのは mise の設定ファイルだけで、sasurahime 自身の設定と削除履歴は一覧の後の文で述べます。
- 以降の設計書は、1.6 節の規則に従って次だけを追記します。
  - 04: ディレクトリの一覧に 1 行 (Docker Desktop の仮想ディスクと `~/.colima/`。ファイルのサイズだけを読むので、内容を読むファイルの一覧は変えません)。
  - 02: ディレクトリの一覧に 1 行 (`~/Library/Containers/`、`~/Library/Group Containers/`) と、内容を読むファイルの一覧に 1 項目 (インストール済みアプリの `Info.plist`。bundle ID とアプリ名のみ)。
  - 03 と 05 は追記しません。03 は列挙済みの `~/Library/Caches/` のメタデータだけを読みます。05 は列挙済みの `~/Library/Application Support/` のメタデータと、02 が一覧に加えた `Info.plist` の bundle ID とアプリ名 (02 の `AppIdentity`) だけを使います。05 は最終タスクで、README の記述が 05 の挙動を包含していることを grep で確認します。
- `README.md` と `Cargo.toml` の「40+」「40 以上」は 51 件でも正しいので変更しません。

### 11.6 docs/HOWTO-USE.md の対象表

- 02/03/05 は `docs/HOWTO-USE.md` の `sasurahime clean <target>` の対象表 (英語 99 行付近、日本語 671 行付近) に 1 行ずつ足します。位置と文言は各設計書にあります。

### 11.7 既知の既存問題 (今回は触らない)

- `ios-backup` は `--yes` の対象に残っており、TTY 上で `--yes` を付けると確認プロンプトを出します。今回の 4 本では修正しません。
- このほか今回は扱わず別 PBI に回す事項 (explorer の Finder 起動の共通化、`per_cleaner` 設定の無効警告の扱い) は、ロードマップ 8 章に一覧があります。
