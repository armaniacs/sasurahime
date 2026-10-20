# Docker 仮想ディスクの実サイズ表示と scan 網羅 設計書

共通規約書 (`docs/superpowers/specs/2026-10-20-00-common-conventions.md`) を優先して参照してください。本書の「共通ルール」の節は要約です。

## 実装順序と位置づけ

- 実装順は 04 (本書) -> 02 -> 03 -> 05 で固定です (`2026-10-20-01-roadmap-and-order.md`)。設計書の中では本書が最初です。前提は、基準コミット (2c71587) で `cargo test`、`cargo clippy -- -D warnings`、`cargo fmt --check` が通ることと、ロードマップ 7 章の前提タスク P0 (README Data access の事実化) が完了していることです。P0 の完了は `grep -n "It reads the contents of only the following files" README.md` が 1 行出ることで確認します。出なければ着手せず、先に P0 を行います。
- 02/03/05 は本書の全タスク (S1 から T5) の完了を前提にします。特に S2 の `Cleaner::auto_clean_allowed()` と S3 の `standard_cleaners()` / `special_cleaners()` / `AUTO_CLEAN_TARGETS` / `ManualOnly` による `all_cleaners()` の再構成は、02/03/05 の登録方法 (マクロ表に 1 エントリ足す) と `--yes` 除外の土台です。名前やシグネチャを変えないでください。
- 既知の既存問題: `ios-backup` は `--yes` で TTY 上だと確認プロンプトを出します。今回は触りません。
- 別 PBI に回す事項 (ロードマップ 8 章): マクロ表で `per_cleaner` 設定を無視している 29 件の factory の不整合と、無効な `per_cleaner` 設定の警告を起動時にまとめて出すこと。本書では colima の 1 行だけを直します (S3)。

## 未検証の仮定

以下は設計時に確認できていません。食い違ったら止まって報告してください。

1. 本書のコード片はコンパイル未確認です。`Config::default()` の存在と、`Config` の全フィールドが `pub` であること (`..config::Config::default()` の構造体更新構文が使えること) は実コードで確認済みです。`crate::progress::VerboseProgress::new()`、`tests/interactive.rs` の `sasurahime()` ヘルパーと `fs` / `TempDir` の import も確認済みです。
2. Docker Desktop の `Docker.raw` の 4 候補パスは、この開発機に Docker Desktop が無いため実在を確認していません (20 節)。
3. Colima の案内文の `fstrim` の効果は未検証です。
4. T4 の `FAKE_TOOLS` で、マクロ表の全 39 件が `is_available()` で true になる前提は、実行して確認していません。`scan_lists_every_standard_target` が失敗した場合、足りない fake ツール名を推測で足さずに報告してください。

## 0. 共通ルール (本書で使うもの)

- 応答、コミットメッセージは日本語。コード、識別子、コード内コメントは英語。絵文字は使いません。
- コメントは非自明な WHY だけを書きます。変更履歴やタスク ID は書きません。
- コミットは Conventional Commits (feat/fix/docs/refactor/test/chore)。`git add -A` と `git add .` は禁止で、ファイルを個別に add します。
- 各 Task の最後に `cargo fmt --check`、`cargo test`、`cargo clippy -- -D warnings` が通ることを確認します。例外として、S2 から D3 の完了までの間は次の `dead_code` 警告だけを許容します (共通規約 1.4)。D3 の完了時と、T4、T5 では警告ゼロです。
  - `ManualOnly` が本番コードから構築されない (S2 の間。S3 で消える)。
  - trait の `advisory` が呼ばれない (`method advisory is never used`)。本番コードで `advisory()` を呼ぶのは D3 の `scanner.rs` だけなので、S2 から D2 の間は出ます。
  - `disk_image` の関数や定数、`DiskImageNotice::render` など、D3 で `advisory()` が呼ばれるまで到達しない項目 (D1 から D2 の間)。
  - 上記以外の警告が出たら止まって報告します。
- ドキュメントの文字は Shift-JIS で表せる範囲にします (EM DASH、不等号の合字、波線イコールは使いません)。
- 実機で `sasurahime clean` は実行しません。`scan`、`targets`、`--dry-run` だけを使います。

## 1. 概要と非目標

### 概要

1. Docker Desktop の仮想ディスク (疎ファイル) の実使用量と最大サイズを `scan` に表示し、`docker system prune` で縮まない場合の案内を出します。Colima にも同じ仕組みを適用します。
2. `all_cleaners()` を `define_cleaners!` マクロから生成し、標準ターゲット 39 件すべてが `scan` とインタラクティブ TUI に出るようにします。
3. 新たに出るターゲットは `--yes` (run_auto) の削除対象にしません。TUI では利用者が明示的に選んだものだけ削除されます。

### 非目標

- OrbStack に仮想ディスク案内は付けません (理由は 2.6 節)。
- `docker system df` など docker デーモンへの問い合わせは detect で行いません。
- `colima prune` の確認文言の正確性見直し、`caches` と `trash` の scan 登録、30 秒タイムアウトの変更は行いません。
- TUI 側への案内表示は行いません。案内は `scan` のみです。
- 既存 15 ターゲットの `--yes` 挙動は変えません。

## 2. 調査結果 (HEAD の事実)

### 2.1 マクロと all_cleaners の乖離

`src/main.rs` の `define_cleaners!` (43-97 行) は `CleanTarget` enum、`SUPPORTED_TARGETS`、`dispatch_clean` などを生成します。doc コメント (26-27 行) は all_cleaners() も生成すると書いていますが、実際の `all_cleaners()` (426-518 行) は手書きの `vec!` で 15 件しか登録していません。

```rust
fn all_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>> {
    let logs_keep_days = config.effective_logs_keep_days();
    let mut cleaners: Vec<Box<dyn cleaner::Cleaner>> = vec![
        // Sprint 1
        Box::new(cleaners::uv::UvCleaner::new(home, Box::new(SystemCommandRunner))),
        ...
```

マクロ表は 39 件、手書きの特殊ターゲットは 9 件 (caches, logs, xcode, trash, ollama, library-logs, device-support, ios-backup, apfs-snapshot)。`sasurahime targets` は 48 行を出力します (実測)。`docs/SUPPORTED.md` の「49」は誤りです。

現在 scan に出る 15 件: uv, brew, mise, browsers, xcode, logs, act, huggingface, pre-commit, library-logs, colima, ollama, device-support, ios-backup, apfs-snapshot。

scan に出ない標準ターゲット 31 件: bun, go, pip, node-gyp, npm, yarn, pnpm, cargo, docker, orbstack, cocoapods, spm, conda, poetry, pipx, deno, dotnet, gem, bundle, rustup, simulator, gradle, jetbrains, downloads, vscode-extensions, maven, terraform, flutter, volta, sbt, tree-sitter。

### 2.2 表示名と CLI 名の差

`Cleaner::name()` (表示名) と CLI 名が異なるのは `cocoa-pods` (CLI) と `cocoapods` (name()) の 1 件だけです。`config.exclude` と `per_cleaner` は `name()` で照合します (`cleaners.retain(|c| !config.exclude.iter().any(|e| e == c.name()))`)。したがって exclude には `cocoapods` と書きます。

### 2.3 factory の引数

マクロ表の各 factory は `|home, config|` 形式のクロージャです。`home` を使うもの、`config` で `.with_config(config)` を呼ぶもの (act, node-gyp, spm, downloads, vscode-extensions, terraform, volta, sbt, tree-sitter)、どちらも使わないもの (`|_home, _config|`) があります。colima の factory は `|home, _config|` で設定を無視していますが、手書きの `all_cleaners()` は colima に `.with_config(config)` を付けています。S3 でマクロ表の colima の行に `.with_config(config)` を付け、両者を揃えます (2.7 節)。`dispatch_clean` は `($factory)(home, config)` と即時呼び出しするので、同じ形で all_cleaners 用の関数を生成できます。runner は factory 内で `Box::new(SystemCommandRunner)` を作るため、呼び出し側で渡す必要はありません。

特殊 7 件 (logs, xcode, library-logs, ollama, device-support, ios-backup, apfs-snapshot) は引数が個別 (keep_days、extra targets、保持数 2、runner なし) なのでマクロ化しません。

### 2.4 scan と run_auto の実行方法

`src/scanner.rs` の `run_scan` は `is_available()` が true のものだけを rayon の `par_iter` で `detect()` します。`is_available()` が false の cleaner は表に出ません。`src/interactive.rs` の `run_auto` と `run_interactive` も同じ方式で、`ScanStatus::Pruneable(_)` のものを対象にします。

`GenericCleaner` の detect の挙動:

- `CleanMethod::Command` (docker, npm, yarn, pnpm, deno, pipx, gem, bundle, dotnet, cocoapods, conda, poetry, orbstack, bun, go, pip): `runner.exists(program)` が true なら `Pruneable(0)`。外部コマンドは実行しません。
- `CommandWithDetectDir` (simulator, maven, flutter, colima): `dir_size` でディレクトリを走査するだけです。
- `DeleteDirs`: ディレクトリ走査のみです。

つまり docker の detect はコマンドを実行せず PATH を探すだけなので、docker デーモンが停止していても scan はハングしません。外部コマンドを detect で実行するのは `rustup toolchain list` だけです。これは `SystemCommandRunner` の 30 秒タイムアウト (`COMMAND_TIMEOUT`) で kill され、`Err` なら `NotFound` を返します。

`CargoCleaner::detect` は HOME を深さ 5 まで walkdir し、`println!` で進捗を出します。rayon 並列の scan 中に出力が混ざるため、本書で `log::info!` に置き換えます。

### 2.5 危険な副作用 (最重要)

Command 型の detect は常に `Pruneable(0)` を返します。現在の `run_auto` は Pruneable の cleaner をすべて clean するので、単純に 31 件を all_cleaners に足すと `sasurahime --yes` が次を実行してしまいます。

- `docker system prune -f`、`npm cache clean --force`、`pnpm store prune`、`conda clean --all -y`、`poetry cache clear --all`、`bundle clean` (カレントの Gemfile 依存)、`gem cleanup`
- `~/Downloads` の 30 日超ファイル、`~/.vscode/extensions` 全体、`~/.sbt`、`~/.m2/repository`、`~/.pub-cache`、`~/.volta/cache`、Cargo の `target/` 全件、rustup 未使用ツールチェイン

そこで `Cleaner::auto_clean_allowed()` (既定 true) を追加し、新規 31 件は `ManualOnly` ラッパーで false にします。run_auto だけがこれを見ます。TUI の `MultiSelect` は既定で未チェックで、"Proceed? [y/N]" もあるため、利用者が明示的に選んだものだけが実行されます。

`GenericCleaner` の `confirm_message` を持つのは colima だけで、colima は元から scan と `--yes` の対象です (挙動は変えません)。TUI と run_auto は `set_skip_confirm(true)` で二重確認を省略します。`ios-backup` と `apfs-snapshot` は cleaner 内で TTY を判定して自衛しているので、そのまま `--yes` 対象に残します。

既知の既存問題 (今回は触らない): `ios-backup` は `--yes` で TTY 上だと確認プロンプトを出します。

後続の 02/03/05 が追加する cleaner (`orphan-containers`、`library-caches`、`app-support`) は `AUTO_CLEAN_TARGETS` に入れないので、S3 の `all_cleaners()` が自動で `ManualOnly` に包みます。加えて各 cleaner 自身も `auto_clean_allowed()` で `false` を返します。

### 2.6 仮想ディスクの実情

`src/format.rs` の `dir_size` は `st_blocks * 512` (物理使用量) を合計します。ファイル単体のパスを渡しても動きます (WalkDir がそのファイルを返します)。論理サイズは `fs::metadata(p).len()` です。疎ファイルでは物理使用量が実際にディスクを占有している量、論理サイズが上限 (最大サイズ) です。

実機 (この開発機) の Colima:

- `~/.colima/_lima/colima/disk` は論理 20G、物理は約 2.7G
- `~/.colima/_lima/_disks/colima/datadisk` は論理 100G
- `du -sh ~/.colima` は 1.4G

したがって表示は「ディスク上の使用量 (物理)」を主とし、「最大サイズ (論理)」を併記します。この機には Docker Desktop も OrbStack も無いため、Docker Desktop のパスは実在確認できません。次の 4 候補を列挙し、存在するものだけを使います。

1. `~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw` (現行版の標準)
2. `~/Library/Containers/com.docker.docker/Data/vms/0/Docker.raw`
3. `~/Library/Containers/com.docker.docker/Data/com.docker.driver.amd64-linux/Docker.qcow2` (旧版)
4. `~/Library/Containers/com.docker.docker/Data/com.docker.driver.amd64-linux/Docker.raw` (旧版)

Colima は `~/.colima/_lima/*/disk`、`~/.colima/_lima/*/diffdisk`、`~/.colima/_lima/_disks/*/datadisk` を候補にします。

OrbStack は仮想ディスクを自動で縮める設計で、`orb prune` も別の目的のコマンドです。この機で実在確認ができないため、推測に基づく案内は出さず、扱いは現状維持とします (非目標)。

### 2.7 colima と per_cleaner 設定の警告

- `GenericCleaner::with_config` (`src/cleaners/generic.rs` の 355 行) は、`config.per_cleaner` に自分の `name()` のエントリがあり、`older_than_days` か `larger_than_mb` が設定されていて、`method` が `CleanMethod::Command` か `CleanMethod::CommandWithDetectDir` のとき、`log::warn!("per-cleaner filters not supported for '{}': command-based cleaner", name)` を出します。その後 `older_than_days` / `larger_than_mb` をフィールドに入れます。
- colima は `CommandWithDetectDir` です。`detect()` と `clean()` がこの 2 つのフィールドを見るのは `CleanMethod::DeleteDirs` の分岐だけなので、colima に対する `with_config` の実際の効果は警告ログ 1 行だけです。
- HEAD では、マクロ表の colima の行は `(|home, _config| cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner))),` で設定を無視しています。このため `sasurahime clean colima` (マクロの `dispatch_clean` 経由) では警告が出ず、警告が出るのは手書きの `all_cleaners()` (scan、TUI、`--yes`) の経路だけです。
- S3 で `all_cleaners()` をマクロ表から生成すると、そのままでは scan の経路の警告も消えます。
- 決定: S3 で、マクロ表の colima の行を `(|home, config| cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner)).with_config(config)),` に変えます。これで scan と clean の両方で警告が出ます。他の 29 件 (マクロ表で `_config` を受け取る行) は変えません。その不整合と、警告を起動時にまとめて出す仕組みは別 PBI です (ロードマップ 8 章)。
- 警告の出力 (`log::warn!`) をテストで捕まえる仕組みは、このプロジェクトのテストにはありません (`log::set_logger` やテスト用ロガーを使うテストが無い)。グローバルなロガーを 1 つのテストバイナリ内の並列テストで共有すると、他のテストの警告と混ざり、設定は 1 回しかできません。そこで警告を出すかどうかの判定と文面を private メソッド `unsupported_filter_warning` に切り出し、それを単体テストします。あわせて、`with_config` の適用前後で colima の `detect()` の結果が変わらないことをテストします。マクロ表の colima の行が `with_config` を呼んでいることは、S3 の 9.4 の `grep` で確認します。

## 3. ファイル一覧と責務

| ファイル | 変更 | 責務 |
|---|---|---|
| `src/cleaner.rs` | 変更 | `Cleaner` に `auto_clean_allowed()` と `advisory()` を追加 |
| `src/cleaners/manual_only.rs` | 新規 | `--yes` 対象外にするラッパー |
| `src/cleaners/mod.rs` | 変更 | `pub mod manual_only;` を追加 |
| `src/interactive.rs` | 変更 | `auto_clean_indices()` を追加し run_auto で使う |
| `src/main.rs` | 変更 | マクロに `standard_cleaners()` を生成、`special_cleaners()`、`AUTO_CLEAN_TARGETS`、`all_cleaners()` 再構成、`mod disk_image;`、テスト |
| `src/disk_image.rs` | 新規 | 疎ディスクイメージの探索、サイズ整形、案内文生成 (純関数) |
| `src/cleaners/generic.rs` | 変更 | `with_config` から警告の判定 `unsupported_filter_warning` を切り出す (S3)。`notice` フィールド、`docker(home, runner)`、colima への案内、Command 型 clean の終了コード警告と縮小判定 (D2) |
| `src/scanner.rs` | 変更 | 表の後に `advisory()` を出力 |
| `src/cleaners/cargo.rs` | 変更 | detect の `println!` を `log::info!` に変更 |
| `tests/scan_coverage.rs` | 新規 | scan 網羅、docker 案内、ハング無しの E2E |
| `tests/interactive.rs` | 変更 | `--yes` が新規ターゲットを実行しない E2E |
| `docs/SUPPORTED.md`, `README.md`, `docs/HOWTO-USE.md`, `docs/HOWTO-ADD-target.md` | 変更 | 件数、docker 節、`--yes` の説明、README の Data access |

## 4. 公開する型、関数、マクロ

```rust
// src/cleaner.rs (trait Cleaner に追加)
/// Whether `--yes` (run_auto) may clean this target without an explicit selection.
fn auto_clean_allowed(&self) -> bool { true }
/// Read-only note printed under the scan table. Must not run external commands.
fn advisory(&self) -> Option<String> { None }

// src/cleaners/manual_only.rs
pub struct ManualOnly { inner: Box<dyn Cleaner> }
impl ManualOnly { pub fn new(inner: Box<dyn Cleaner>) -> Self; }

// src/interactive.rs
pub(crate) fn auto_clean_indices(results: &[ScanResult], cleaners: &[Box<dyn Cleaner>]) -> Vec<usize>;

// src/disk_image.rs
pub const MIN_NOTICE_BYTES: u64 = 1024 * 1024;
pub const DOCKER_REMEDY: &str = /* text in section 10.3 */;
pub const COLIMA_REMEDY: &str = /* text in section 10.3 */;
pub struct DiskImage { pub path: PathBuf, pub physical: u64, pub logical: u64 }
pub fn probe(path: &Path) -> Option<DiskImage>;
pub struct DiskImageNotice { /* label, candidates, remedy */ }
impl DiskImageNotice {
    pub fn new(label: &'static str, candidates: Vec<PathBuf>, remedy: &'static str) -> Self;
    pub fn find(&self) -> Vec<DiskImage>;
    pub fn physical_total(&self) -> u64;
    pub fn render(&self) -> Option<String>;
    pub fn after_clean_message(&self, before: u64, after: u64) -> Option<String>;
}
pub fn docker_desktop_candidates(home: &Path) -> Vec<PathBuf>;
pub fn colima_candidates(home: &Path) -> Vec<PathBuf>;

// src/cleaners/generic.rs
pub fn docker(home: &Path, runner: Box<dyn CommandRunner>) -> Self;   // 引数に home を追加
pub fn with_notice(self, notice: DiskImageNotice) -> Self;
fn unsupported_filter_warning(&self, config: &Config) -> Option<String>; // private。S3 で with_config から切り出す

// src/main.rs (define_cleaners! が生成)
fn standard_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>>;
const AUTO_CLEAN_TARGETS: &[&str];
fn special_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>>;
```

## 5. 決定事項と理由

| 論点 | 決定 | 理由 |
|---|---|---|
| 表示サイズ | 物理 (st_blocks) を主、論理を「maximum size」として併記 | 物理が実際に占有している量。論理だけだと 100G と誤解させる |
| scan の Size 列 | 変更しない (docker は `Pruneable(0)` のまま) | 仮想ディスクは prune で縮む保証が無く、解放可能量として出すと嘘になる |
| 案内の出し先 | scan の表の下に Notes として出力 | `ScanResult` の構造体リテラルを変えずに済む |
| docker の問い合わせ | detect で docker を実行しない | デーモン停止、起動中のハングを構造的に避ける |
| 案内の閾値 | 物理 1 MiB 未満は出さない | 空ファイルや雛形でのノイズ防止 |
| `--yes` の範囲 | 既存 15 件のみ。新規 31 件は ManualOnly | 意図しない削除の防止 |
| 重複防止 | `standard_cleaners` と `special_cleaners` の name() 一意性テスト | 手書きと生成の二重登録を検出 |
| scan に出さないもの | `caches` (集約) と `trash` (scan 専用説明で削除は特殊) | 集約は個別と重複する。trash は --yes で消さないため登録しない |
| docker の clean | 終了コードが非 0 なら警告を出し、縮小判定は成功時のみ | デーモン停止時に「縮まなかった」と誤案内しない |
| colima の clean 後判定 | 実装しない (scan の案内のみ) | colima は CommandWithDetectDir で別経路。範囲を絞る |
| colima の `per_cleaner` 警告 | マクロ表の colima の行に `.with_config(config)` を付け、scan と clean の両方で警告を出す (S3) | HEAD の scan の経路で出ていた警告を S3 の生成切替で失わないため。`clean colima` でも同じ警告が出るようになる (2.7 節) |
| 他の 29 件の factory の `_config` | 変えない | 別 PBI (ロードマップ 8 章)。29 件を変えると dispatch と scan の挙動に広く触れるため |
| 警告のテスト方法 | 判定を `unsupported_filter_warning` に切り出して単体テストし、`detect()` が変わらないことを確かめる。ログは捕まえない | ログを捕まえる既存の仕組みが無く、グローバルなロガーは並列テストで混ざるため (2.7 節) |

## 6. タスクの依存順序

```
S1 (回帰テスト) -> S2 (ManualOnly と run_auto 保護) -> S3 (マクロ生成へ切替)
                                                          |
D1 (disk_image) -> D2 (GenericCleaner 統合) -> D3 (scan 出力と E2E)  ※ D3 は S3 の後
S3 の後: T4 (cargo の出力抑止) -> T5 (ドキュメント)
```

S2 を S3 より先に必ず完了させます。逆順にすると、S3 の時点で `--yes` が docker prune などを実行します。D1 と D2 は S 系列と独立ですが、D2 が main.rs の docker factory を変更するため S3 の後に行います。

## 7. Task S1: 現状の 15 件と一意性を固定する回帰テスト

目的: 生成への切替前に、既存の登録内容を固定します。このタスクのテストは HEAD で成功します。

### 7.1 テストを書く

`src/main.rs` 末尾の `mod tests` 内 (既存テストの後ろ) に追加します。

```rust
    const LEGACY_SCAN_TARGETS: &[&str] = &[
        "uv",
        "brew",
        "mise",
        "browsers",
        "xcode",
        "logs",
        "act",
        "huggingface",
        "pre-commit",
        "library-logs",
        "colima",
        "ollama",
        "device-support",
        "ios-backup",
        "apfs-snapshot",
    ];

    fn names_of(cleaners: &[Box<dyn cleaner::Cleaner>]) -> Vec<&'static str> {
        cleaners.iter().map(|c| c.name()).collect()
    }

    #[test]
    fn all_cleaners_keeps_legacy_targets() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        for legacy in LEGACY_SCAN_TARGETS {
            assert!(names.contains(legacy), "missing legacy target: {legacy}");
        }
    }

    #[test]
    fn all_cleaners_names_are_unique() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate cleaner names: {names:?}");
    }
```

### 7.2 実行

```
cargo test --bin sasurahime all_cleaners_ 2>&1 | tail -8
```

期待: 2 件とも ok (HEAD で成功)。

### 7.3 コミット

```
git add src/main.rs
git commit -m "test(scan): all_cleaners の既存 15 件と名前の一意性を回帰テストで固定"
```

## 8. Task S2: `--yes` 保護 (ManualOnly と auto_clean_allowed)

### 8.1 失敗するテストを書く

(a) `src/cleaners/manual_only.rs` を新規作成します。最初はテストだけを書きます。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cleaner::ScanStatus;

    struct Stub;

    impl Cleaner for Stub {
        fn name(&self) -> &'static str {
            "stub"
        }
        fn detect(&self) -> ScanResult {
            ScanResult::new("stub", ScanStatus::Pruneable(7))
        }
        fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
            Ok(CleanResult {
                name: "stub",
                bytes_freed: 7,
                uses_trash: false,
                skipped: vec![],
                deleted_paths: vec![],
            })
        }
    }

    #[test]
    fn default_cleaner_allows_auto_clean() {
        assert!(Stub.auto_clean_allowed());
    }

    #[test]
    fn manual_only_blocks_auto_clean() {
        let c = ManualOnly::new(Box::new(Stub));
        assert!(!c.auto_clean_allowed());
    }

    #[test]
    fn manual_only_delegates_name_detect_and_clean() {
        let c = ManualOnly::new(Box::new(Stub));
        assert_eq!(c.name(), "stub");
        assert!(matches!(c.detect().status, ScanStatus::Pruneable(7)));
        let reporter = crate::progress::VerboseProgress::new();
        assert_eq!(c.clean(false, &reporter).unwrap().bytes_freed, 7);
    }
}
```

(b) `src/interactive.rs` の `mod tests` 末尾に追加します。

```rust
    #[test]
    fn auto_clean_indices_skips_manual_only() {
        let cleaners: Vec<Box<dyn Cleaner>> = vec![
            Box::new(TestCleaner::new("brew", 2000, vec![])),
            Box::new(crate::cleaners::manual_only::ManualOnly::new(Box::new(
                TestCleaner::new("docker", 3000, vec![]),
            ))),
            Box::new(TestCleaner::new("empty", 0, vec![])),
        ];
        let results: Vec<ScanResult> = cleaners.iter().map(|c| c.detect()).collect();
        assert_eq!(auto_clean_indices(&results, &cleaners), vec![0]);
    }
```

(c) `tests/interactive.rs` 末尾に E2E を追加します。act は `--yes` 対象 (既存 15 件)、docker は対象外であることを同時に確かめます。

```rust
#[test]
fn yes_flag_never_runs_manual_only_targets() {
    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let calls = tmp.path().join("calls_docker.txt");
    let script = format!("#!/bin/sh\necho \"$@\" >> \"{}\"\nexit 0\n", calls.display());
    fs::write(bin_dir.join("docker"), script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(bin_dir.join("docker"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let act_cache = tmp.path().join(".cache/act");
    fs::create_dir_all(&act_cache).unwrap();
    fs::write(act_cache.join("dummy"), b"x".repeat(1024)).unwrap();

    let original_path = std::env::var("PATH").unwrap_or_default();
    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:{original_path}", bin_dir.display()))
        .env_remove("ACT_CACHE_DIR")
        .args(["--permanent", "--yes"])
        .write_stdin("y\n")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!act_cache.exists(), "act is auto-eligible and must be cleaned");
    assert!(!calls.exists(), "docker prune must never run under --yes");
}
```

### 8.2 実行と期待される失敗

```
cargo test --bin sasurahime manual_only 2>&1 | tail -15
```

期待: コンパイルエラー (`manual_only` モジュール未登録、`auto_clean_allowed` / `auto_clean_indices` 未定義)。E2E は HEAD でも成功します (docker が未登録のため)。S3 後も成功し続けることが目的です。

### 8.3 実装

1. `src/cleaner.rs` の `trait Cleaner` 内、`fn sub_targets` の直前に追加します。

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

2. `src/cleaners/mod.rs` に `pub mod manual_only;` を `log` と `mise` の間に追加します。

3. `src/cleaners/manual_only.rs` のテストの前 (ファイル先頭) に実装を書きます。

```rust
use crate::cleaner::{CleanOptions, CleanResult, Cleaner, ScanResult};
use crate::progress::ProgressReporter;
use anyhow::Result;

/// Wraps a cleaner so that `--yes` never selects it. Scan, interactive
/// selection and explicit `clean <target>` are unaffected.
pub struct ManualOnly {
    inner: Box<dyn Cleaner>,
}

impl ManualOnly {
    pub fn new(inner: Box<dyn Cleaner>) -> Self {
        Self { inner }
    }
}

impl Cleaner for ManualOnly {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn detect(&self) -> ScanResult {
        self.inner.detect()
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        self.inner.clean(dry_run, reporter)
    }

    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        opts: &CleanOptions,
    ) -> Result<CleanResult> {
        self.inner.clean_with_opts(dry_run, reporter, opts)
    }

    fn is_available(&self) -> bool {
        self.inner.is_available()
    }

    fn sub_targets(&self) -> Vec<(&'static str, u64)> {
        self.inner.sub_targets()
    }

    fn advisory(&self) -> Option<String> {
        self.inner.advisory()
    }

    fn auto_clean_allowed(&self) -> bool {
        false
    }
}
```

4. `src/interactive.rs`: `run_auto` の直前に関数を追加します。

```rust
/// Indices that `--yes` may clean: Pruneable and allowed for unattended runs.
pub(crate) fn auto_clean_indices(
    results: &[ScanResult],
    cleaners: &[Box<dyn Cleaner>],
) -> Vec<usize> {
    results
        .iter()
        .enumerate()
        .filter(|(i, r)| {
            matches!(r.status, ScanStatus::Pruneable(_)) && cleaners[*i].auto_clean_allowed()
        })
        .map(|(i, _)| i)
        .collect()
}
```

`run_auto` 内の次のブロックを置換します。`if !crate::trash::is_trash_mode() {` まで含めると `run_interactive` 側と区別できます。

HEAD:

```rust
    let pruneable_indices: Vec<usize> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| matches!(r.status, ScanStatus::Pruneable(_)))
        .map(|(i, _)| i)
        .collect();

    if pruneable_indices.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    if !crate::trash::is_trash_mode() {
```

置換後:

```rust
    let pruneable_indices = auto_clean_indices(&results, cleaners);

    if pruneable_indices.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    if !crate::trash::is_trash_mode() {
```

`run_interactive` 側 (直前に `// Collect indices of pruneable cleaners only` コメントがある方) は変更しません。

### 8.4 実行と期待出力

```
cargo test --bin sasurahime manual_only
cargo test --bin sasurahime auto_clean_indices
cargo test --test interactive yes_flag_never
cargo fmt --check
cargo clippy -- -D warnings
```

期待: manual_only 3 件、auto_clean_indices 1 件、E2E 1 件が ok。fmt は出力なし。clippy は `ManualOnly` が本番コードから構築されないことによる `dead_code` 警告 (`struct ManualOnly is never constructed` など) だけが出てもかまいません (S3 で使われて消えます)。それ以外の警告が出たら止まって報告します。

### 8.5 コミット

```
git add src/cleaner.rs src/cleaners/mod.rs src/cleaners/manual_only.rs src/interactive.rs tests/interactive.rs
git commit -m "feat(auto): --yes の対象外にできる ManualOnly と auto_clean_allowed を追加"
```

## 9. Task S3: all_cleaners をマクロ生成に切り替える

前提: S2 が完了していること。

範囲: `all_cleaners()` のマクロ生成への切替に加えて、マクロ表の colima の行に `.with_config(config)` を付け、`with_config` の警告判定を `unsupported_filter_warning` に切り出します (2.7 節)。colima の行を変えないと、切替によって scan の経路で出ていた colima の `per_cleaner` 警告が消えるためです。

### 9.1 失敗するテストを書く

`src/main.rs` の `mod tests` に追加します (S1 の `names_of` と `LEGACY_SCAN_TARGETS` を使います)。

```rust
    const NOT_SCANNED: &[&str] = &["caches", "trash"];

    // The only target whose Cleaner::name() differs from its CLI name.
    fn cleaner_name_for(cli_name: &str) -> &str {
        match cli_name {
            "cocoa-pods" => "cocoapods",
            other => other,
        }
    }

    #[test]
    fn every_clean_target_is_registered_in_all_cleaners() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        let names = names_of(&cleaners);
        let missing: Vec<&str> = SUPPORTED_TARGETS
            .iter()
            .chain(extra_targets())
            .map(|(cli, _)| *cli)
            .filter(|cli| !NOT_SCANNED.contains(cli))
            .filter(|cli| !names.iter().any(|n| *n == cleaner_name_for(cli)))
            .collect();
        assert!(missing.is_empty(), "targets missing from all_cleaners: {missing:?}");
    }

    #[test]
    fn standard_cleaners_follow_the_macro_table_order() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = standard_cleaners(tmp.path(), &config::Config::default());
        assert_eq!(cleaners.len(), SUPPORTED_TARGETS.len());
        for ((cli, _), c) in SUPPORTED_TARGETS.iter().zip(&cleaners) {
            assert_eq!(c.name(), cleaner_name_for(cli));
        }
    }

    #[test]
    fn standard_and_special_cleaners_do_not_overlap() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config::Config::default();
        let standard = names_of(&standard_cleaners(tmp.path(), &cfg));
        let special = names_of(&special_cleaners(tmp.path(), &cfg));
        for s in &special {
            assert!(!standard.contains(s), "registered twice: {s}");
        }
    }

    #[test]
    fn only_legacy_targets_are_auto_clean_allowed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        for c in &cleaners {
            assert_eq!(
                c.auto_clean_allowed(),
                AUTO_CLEAN_TARGETS.contains(&c.name()),
                "auto policy mismatch for {}",
                c.name()
            );
        }
        let names = names_of(&cleaners);
        for t in AUTO_CLEAN_TARGETS {
            assert!(names.contains(t), "AUTO_CLEAN_TARGETS has unknown name: {t}");
        }
        assert_eq!(AUTO_CLEAN_TARGETS, LEGACY_SCAN_TARGETS);
    }

    #[test]
    fn destructive_targets_are_not_auto_clean_allowed() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        for danger in [
            "docker", "orbstack", "npm", "yarn", "pnpm", "gem", "bundle", "conda", "poetry",
            "downloads", "vscode-extensions", "cargo", "rustup", "simulator", "maven", "sbt",
        ] {
            let c = cleaners.iter().find(|c| c.name() == danger).expect(danger);
            assert!(!c.auto_clean_allowed(), "{danger} must not run under --yes");
        }
    }

    #[test]
    fn config_exclude_removes_generated_targets() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cfg = config::Config {
            exclude: vec!["docker".to_string(), "cocoapods".to_string()],
            ..config::Config::default()
        };
        let names = names_of(&all_cleaners(tmp.path(), &cfg));
        assert!(!names.contains(&"docker"));
        assert!(!names.contains(&"cocoapods"));
        assert!(names.contains(&"npm"));
    }
```

`src/cleaners/generic.rs` 末尾の `mod tests` の最後 (`with_config_no_match_for_name_leaves_cleaner_unchanged` の後ろ、`mod tests` を閉じる `}` の直前) に追加します (2.7 節)。`MissingToolRunner` と `fs`、`ScanStatus` は既存の `mod tests` で使えます。

```rust
    fn colima_filter_config() -> crate::config::Config {
        let mut per_cleaner = std::collections::HashMap::new();
        per_cleaner.insert(
            "colima".to_string(),
            crate::config::PerCleanerConfig {
                older_than_days: Some(30),
                larger_than_mb: None,
            },
        );
        crate::config::Config {
            per_cleaner,
            ..crate::config::Config::default()
        }
    }

    #[test]
    fn colima_with_filter_config_takes_the_warning_branch() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaner = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let msg = cleaner
            .unsupported_filter_warning(&colima_filter_config())
            .expect("colima ignores per-cleaner filters, so a warning is expected");
        assert_eq!(
            msg,
            "per-cleaner filters not supported for 'colima': command-based cleaner"
        );
    }

    #[test]
    fn unsupported_filter_warning_is_none_without_entry_or_for_delete_dirs() {
        let tmp = tempfile::TempDir::new().unwrap();
        let colima = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        assert!(colima
            .unsupported_filter_warning(&crate::config::Config::default())
            .is_none());

        let mut per_cleaner = std::collections::HashMap::new();
        per_cleaner.insert(
            "dirs-only".to_string(),
            crate::config::PerCleanerConfig {
                older_than_days: Some(30),
                larger_than_mb: None,
            },
        );
        let config = crate::config::Config {
            per_cleaner,
            ..crate::config::Config::default()
        };
        let dirs = GenericCleaner::delete_dirs("dirs-only", tmp.path().join("data"));
        assert!(dirs.unsupported_filter_warning(&config).is_none());
    }

    #[test]
    fn colima_with_config_keeps_detect_result() {
        let tmp = tempfile::TempDir::new().unwrap();
        // Freshly written, so a 30-day age filter would hide it if it were applied.
        fs::create_dir_all(tmp.path().join(".colima/_lima/colima")).unwrap();
        fs::write(tmp.path().join(".colima/_lima/colima/dummy.img"), b"x").unwrap();

        let plain = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner)).detect();
        let configured = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner))
            .with_config(&colima_filter_config())
            .detect();

        assert!(matches!(plain.status, ScanStatus::Pruneable(_)));
        assert!(matches!(configured.status, ScanStatus::Pruneable(_)));
        assert_eq!(plain.bytes_for_test(), configured.bytes_for_test());
    }
```

`ScanStatus` は `PartialEq` を実装していないので、`assert_eq!(plain.status, configured.status)` とは書けません。`matches!` と `bytes_for_test()` で比べます。

### 9.2 実行と期待される失敗

```
cargo test --bin sasurahime 2>&1 | tail -20
```

期待: コンパイルエラー (`standard_cleaners`, `special_cleaners`, `AUTO_CLEAN_TARGETS` が未定義、`no method named unsupported_filter_warning found`)。

### 9.3 実装 (src/main.rs)

(1) マクロ直前の doc コメント (26-27 行) を次に置換します。

```rust
/// Macro that generates the CleanTarget enum, SUPPORTED_TARGETS, the dispatch
/// helpers and standard_cleaners() from a single definition table.
/// all_cleaners() composes standard_cleaners() with special_cleaners().
```

(2) マクロ本体で、`const SUPPORTED_TARGETS ... ];` の直後 (`fn dispatch_clean(` の前) に追加します。

```rust
        fn standard_cleaners(
            home: &std::path::Path,
            config: &config::Config,
        ) -> Vec<Box<dyn cleaner::Cleaner>> {
            vec![
                $( Box::new(($factory)(home, config)) as Box<dyn cleaner::Cleaner>, )*
            ]
        }
```

(3) 426-518 行の `all_cleaners` 関数全体を次で置換します。

```rust
/// Targets that `--yes` may clean. Every other registered target is wrapped in
/// ManualOnly so it runs only when selected in the TUI or via `clean <target>`.
const AUTO_CLEAN_TARGETS: &[&str] = &[
    "uv",
    "brew",
    "mise",
    "browsers",
    "xcode",
    "logs",
    "act",
    "huggingface",
    "pre-commit",
    "library-logs",
    "colima",
    "ollama",
    "device-support",
    "ios-backup",
    "apfs-snapshot",
];

fn special_cleaners(
    home: &std::path::Path,
    config: &config::Config,
) -> Vec<Box<dyn cleaner::Cleaner>> {
    // Per-cleaner `older_than_days` for logs falls back to `config.logs_keep_days`.
    let logs_keep_days = config.effective_logs_keep_days();
    vec![
        Box::new(cleaners::xcode::XcodeCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::log::LogCleaner::new_with_extra(
            home,
            logs_keep_days,
            config
                .logs_extra_targets
                .iter()
                .map(|t| cleaners::log::OwnedLogTarget {
                    name: t.name.clone(),
                    path: config::Config::expand_tilde(&t.path, home),
                    exclude: t.exclude.clone(),
                })
                .collect(),
        )),
        Box::new(cleaners::library_logs::LibraryLogsCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::ollama::OllamaCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::device_support::DeviceSupportCleaner::new(
            home,
            2,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::ios_backup::IosCleaner::new(
            home,
            Box::new(SystemCommandRunner),
        )),
        Box::new(cleaners::apfs_snapshot::ApfsSnapshotCleaner::new(Box::new(
            SystemCommandRunner,
        ))),
    ]
}

fn all_cleaners(home: &std::path::Path, config: &config::Config) -> Vec<Box<dyn cleaner::Cleaner>> {
    let mut cleaners: Vec<Box<dyn cleaner::Cleaner>> = standard_cleaners(home, config)
        .into_iter()
        .chain(special_cleaners(home, config))
        .map(|c| -> Box<dyn cleaner::Cleaner> {
            if AUTO_CLEAN_TARGETS.contains(&c.name()) {
                c
            } else {
                Box::new(cleaners::manual_only::ManualOnly::new(c))
            }
        })
        .collect();
    // Apply exclude filter
    cleaners.retain(|c| !config.exclude.iter().any(|e| e == c.name()));
    // Add custom cleaners (always included since user defined them)
    for ct in &config.custom {
        let path = config::Config::expand_tilde(&ct.path, home);
        cleaners.push(Box::new(cleaners::custom::CustomPathCleaner::new(
            ct.name.clone(),
            path,
        )));
    }
    cleaners
}
```

custom cleaner は ManualOnly で包みません。利用者が config に定義したものなので従来どおり `--yes` 対象です。

(4) マクロ表の colima の行を置換します (2.7 節)。`Colima : "colima" => ...;` の次の行です。

HEAD:

```rust
    (|home, _config| cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner))),
```

置換後:

```rust
    (|home, config| cleaners::generic::GenericCleaner::colima_prune(home, Box::new(SystemCommandRunner)).with_config(config)),
```

マクロ表の他の行 (`_config` を受け取る残り 29 件) は変えません (ロードマップ 8 章)。

### 9.3b 実装 (src/cleaners/generic.rs)

`impl GenericCleaner` の `with_config` (HEAD の 352-380 行、doc コメント 3 行を含む) を、次の 2 つのメソッドで置換します。警告の文面と、`older_than_days` / `larger_than_mb` を入れる処理は変えません。

HEAD:

```rust
    /// Apply per-cleaner config filters from the parsed config file.
    /// Looks up `self.name()` in `config.per_cleaner` and applies
    /// `older_than_days` and `larger_than_mb` if present.
    pub fn with_config(self, config: &Config) -> Self {
        let name = self.name();
        if let Some(pcc) = config.per_cleaner.get(name) {
            if (pcc.older_than_days.is_some() || pcc.larger_than_mb.is_some())
                && matches!(
                    self.method,
                    CleanMethod::Command { .. } | CleanMethod::CommandWithDetectDir { .. }
                )
            {
                log::warn!(
                    "per-cleaner filters not supported for '{}': command-based cleaner",
                    name
                );
            }
            let mut c = self;
            if let Some(days) = pcc.older_than_days {
                c = c.with_older_than(days);
            }
            if let Some(mb) = pcc.larger_than_mb {
                c = c.with_larger_than(mb);
            }
            c
        } else {
            self
        }
    }
```

置換後:

```rust
    /// Apply per-cleaner config filters from the parsed config file.
    /// Looks up `self.name()` in `config.per_cleaner` and applies
    /// `older_than_days` and `larger_than_mb` if present.
    pub fn with_config(self, config: &Config) -> Self {
        if let Some(msg) = self.unsupported_filter_warning(config) {
            log::warn!("{msg}");
        }
        let name = self.name();
        if let Some(pcc) = config.per_cleaner.get(name) {
            let mut c = self;
            if let Some(days) = pcc.older_than_days {
                c = c.with_older_than(days);
            }
            if let Some(mb) = pcc.larger_than_mb {
                c = c.with_larger_than(mb);
            }
            c
        } else {
            self
        }
    }

    /// Command-based cleaners never look at `older_than_days` /
    /// `larger_than_mb`, so a configured filter is reported instead of being
    /// silently ignored.
    fn unsupported_filter_warning(&self, config: &Config) -> Option<String> {
        let pcc = config.per_cleaner.get(self.name())?;
        let has_filter = pcc.older_than_days.is_some() || pcc.larger_than_mb.is_some();
        let command_based = matches!(
            self.method,
            CleanMethod::Command { .. } | CleanMethod::CommandWithDetectDir { .. }
        );
        if has_filter && command_based {
            Some(format!(
                "per-cleaner filters not supported for '{}': command-based cleaner",
                self.name()
            ))
        } else {
            None
        }
    }
```

`self.name()` は `&'static str` を返すので、`let name = self.name();` の後に `let mut c = self;` で `self` を移動できます (HEAD と同じ)。`unsupported_filter_warning` は private のまま (`pub` を付けない) にします。呼び出し元は `with_config` と同じファイルの `mod tests` だけです。

### 9.4 実行と期待出力

```
cargo test --bin sasurahime 2>&1 | tail -20
cargo test --bin sasurahime colima_with 2>&1 | tail -8
cargo test --bin sasurahime unsupported_filter_warning 2>&1 | tail -8
cargo test --test interactive 2>&1 | tail -8
grep -n "colima_prune(home, Box::new(SystemCommandRunner)).with_config(config)" src/main.rs
grep -c "|home, _config| cleaners::generic::GenericCleaner::colima_prune" src/main.rs
cargo fmt --check
cargo clippy -- -D warnings
```

期待: S1 の 2 件、本タスクの `src/main.rs` の 6 件と `src/cleaners/generic.rs` の 3 件が ok (`colima_with` で 2 件、`unsupported_filter_warning` で 1 件)。interactive の E2E が全件 ok (`yes_flag_never_runs_manual_only_targets` を含む)。1 つ目の `grep -n` はマクロ表の colima の行 1 行だけを出し、`grep -c` は `0` を出します。fmt は出力なし。clippy は `method advisory is never used` の警告だけが残ります (D3 で消えます。0 章の例外)。

### 9.5 コミット

```
git add src/main.rs src/cleaners/generic.rs
git commit -m "refactor(scan): all_cleaners をマクロ表から生成し、新規ターゲットは --yes 対象外にする"
```

コミットメッセージの本文 (2 行目以降) に「colima は scan と clean の両方で per_cleaner の警告を出す」と 1 行添えます。`git commit -m "<1 行目>" -m "colima は scan と clean の両方で per_cleaner 設定の警告を出す"` の形で書きます。

## 10. Task D1: disk_image モジュール (純関数)

### 10.1 失敗するテストを書く

`src/disk_image.rs` を新規作成し、まずテストだけを書きます。`src/main.rs` の `mod cleaners;` の次の行に `mod disk_image;` を追加します (モジュール宣言はアルファベット順で `command`, `config`, `context`, `disk_image`, `explorer` の位置)。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    const MIB: usize = 1024 * 1024;

    /// Writes `real` bytes of data, then extends the file to `logical` bytes (sparse tail).
    fn make_image(path: &Path, real: usize, logical: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = File::create(path).unwrap();
        f.write_all(&vec![1u8; real]).unwrap();
        f.set_len(logical).unwrap();
    }

    fn notice_for(path: PathBuf) -> DiskImageNotice {
        DiskImageNotice::new("docker", vec![path], "REMEDY TEXT")
    }

    #[test]
    fn probe_reports_physical_below_logical_for_sparse_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Docker.raw");
        make_image(&p, 2 * MIB, 64 * MIB as u64);
        let img = probe(&p).unwrap();
        assert_eq!(img.logical, 64 * MIB as u64);
        assert!(img.physical >= 2 * MIB as u64);
        assert!(img.physical < img.logical);
    }

    #[test]
    fn probe_returns_none_for_missing_or_directory() {
        let tmp = TempDir::new().unwrap();
        assert!(probe(&tmp.path().join("nope")).is_none());
        assert!(probe(tmp.path()).is_none());
    }

    #[test]
    fn find_ignores_images_below_threshold() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("tiny.raw");
        make_image(&p, 10, 10);
        assert!(notice_for(p).find().is_empty());
    }

    #[test]
    fn render_is_none_without_images() {
        let tmp = TempDir::new().unwrap();
        assert!(notice_for(tmp.path().join("nope")).render().is_none());
    }

    #[test]
    fn render_lists_path_sizes_and_remedy() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Docker.raw");
        make_image(&p, 2 * MIB, 64 * MIB as u64);
        let text = notice_for(p).render().unwrap();
        assert!(text.contains("[docker]"), "{text}");
        assert!(text.contains("Docker.raw"), "{text}");
        assert!(text.contains("on disk:"), "{text}");
        assert!(text.contains("64.0 MB"), "{text}");
        assert!(text.contains("REMEDY TEXT"), "{text}");
    }

    #[test]
    fn after_clean_message_reports_shrink() {
        let n = notice_for(PathBuf::from("/nonexistent"));
        let msg = n.after_clean_message(100 * MIB as u64, 50 * MIB as u64).unwrap();
        assert!(msg.contains("100.0 MB -> 50.0 MB"), "{msg}");
        assert!(!msg.contains("did not shrink"), "{msg}");
    }

    #[test]
    fn after_clean_message_warns_when_not_shrunk() {
        let n = notice_for(PathBuf::from("/nonexistent"));
        let msg = n.after_clean_message(100 * MIB as u64, 99 * MIB as u64).unwrap();
        assert!(msg.contains("did not shrink"), "{msg}");
        assert!(msg.contains("REMEDY TEXT"), "{msg}");
    }

    #[test]
    fn after_clean_message_is_none_when_nothing_was_measured() {
        let n = notice_for(PathBuf::from("/nonexistent"));
        assert!(n.after_clean_message(0, 0).is_none());
    }

    #[test]
    fn docker_candidates_start_with_current_layout() {
        let c = docker_desktop_candidates(Path::new("/h"));
        assert_eq!(
            c[0],
            PathBuf::from("/h/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw")
        );
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn colima_candidates_find_disk_and_datadisk() {
        let tmp = TempDir::new().unwrap();
        let lima = tmp.path().join(".colima/_lima");
        fs::create_dir_all(lima.join("colima")).unwrap();
        fs::create_dir_all(lima.join("_disks/colima")).unwrap();
        let c = colima_candidates(tmp.path());
        assert!(c.contains(&lima.join("colima/disk")), "{c:?}");
        assert!(c.contains(&lima.join("_disks/colima/datadisk")), "{c:?}");
    }

    #[test]
    fn colima_candidates_empty_without_colima_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(colima_candidates(tmp.path()).is_empty());
    }
}
```

### 10.2 実行と期待される失敗

```
cargo test --bin sasurahime disk_image 2>&1 | tail -15
```

期待: コンパイルエラー (`probe`, `DiskImageNotice` などが未定義)。

### 10.3 実装

`src/disk_image.rs` のテストの前 (ファイル先頭) に書きます。

```rust
use crate::format::format_bytes;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Images smaller than this are not worth a notice.
pub const MIN_NOTICE_BYTES: u64 = 1024 * 1024;

pub const DOCKER_REMEDY: &str = "To return space to macOS: Docker Desktop > Settings > Resources > Disk image size (reduce it), or Troubleshoot > Clean / Purge data (deletes ALL images, containers and volumes).";

pub const COLIMA_REMEDY: &str = "To return space to macOS: run `colima ssh -- sudo fstrim -av` to ask the VM to discard unused blocks; if the file still does not shrink, `colima delete` recreates the VM (it removes ALL containers, images and volumes).";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskImage {
    pub path: PathBuf,
    /// Bytes actually allocated on disk (st_blocks * 512).
    pub physical: u64,
    /// Apparent size, i.e. the maximum the sparse file can grow to.
    pub logical: u64,
}

pub fn probe(path: &Path) -> Option<DiskImage> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    Some(DiskImage {
        path: path.to_path_buf(),
        physical: meta.blocks() * 512,
        logical: meta.len(),
    })
}

#[derive(Debug, Clone)]
pub struct DiskImageNotice {
    label: &'static str,
    candidates: Vec<PathBuf>,
    remedy: &'static str,
}

impl DiskImageNotice {
    pub fn new(label: &'static str, candidates: Vec<PathBuf>, remedy: &'static str) -> Self {
        Self {
            label,
            candidates,
            remedy,
        }
    }

    pub fn find(&self) -> Vec<DiskImage> {
        self.candidates
            .iter()
            .filter_map(|p| probe(p))
            .filter(|i| i.physical >= MIN_NOTICE_BYTES)
            .collect()
    }

    pub fn physical_total(&self) -> u64 {
        self.find().iter().map(|i| i.physical).sum()
    }

    pub fn render(&self) -> Option<String> {
        let images = self.find();
        if images.is_empty() {
            return None;
        }
        let mut out = format!(
            "[{}] virtual disk image (pruning inside the VM does not always shrink it):\n",
            self.label
        );
        for i in &images {
            out.push_str(&format!(
                "  {}\n    on disk: {} / maximum size: {}\n",
                i.path.display(),
                format_bytes(i.physical),
                format_bytes(i.logical)
            ));
        }
        out.push_str(&format!("  {}", self.remedy));
        Some(out)
    }

    pub fn after_clean_message(&self, before: u64, after: u64) -> Option<String> {
        if before == 0 {
            return None;
        }
        // Shrinking by 2% or less counts as "did not shrink" to absorb block rounding.
        if after + before / 50 < before {
            Some(format!(
                "[{}] virtual disk on disk: {} -> {}",
                self.label,
                format_bytes(before),
                format_bytes(after)
            ))
        } else {
            Some(format!(
                "[{}] virtual disk did not shrink: {} -> {}\n  {}",
                self.label,
                format_bytes(before),
                format_bytes(after),
                self.remedy
            ))
        }
    }
}

pub fn docker_desktop_candidates(home: &Path) -> Vec<PathBuf> {
    let data = home.join("Library/Containers/com.docker.docker/Data");
    vec![
        data.join("vms/0/data/Docker.raw"),
        data.join("vms/0/Docker.raw"),
        data.join("com.docker.driver.amd64-linux/Docker.qcow2"),
        data.join("com.docker.driver.amd64-linux/Docker.raw"),
    ]
}

pub fn colima_candidates(home: &Path) -> Vec<PathBuf> {
    let lima = home.join(".colima/_lima");
    let mut out = vec![];
    for dir in subdirs(&lima) {
        if dir.file_name().is_some_and(|n| n == "_disks") {
            for disk in subdirs(&dir) {
                out.push(disk.join("datadisk"));
            }
        } else {
            out.push(dir.join("disk"));
            out.push(dir.join("diffdisk"));
        }
    }
    out.sort();
    out
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}
```

物理サイズに `dir_size` ではなく `MetadataExt::blocks` を直接使うのは、単一ファイルで walkdir が不要なためです。`dir_size` と同じ `blocks() * 512` 基準です。

### 10.4 実行と期待出力

```
cargo test --bin sasurahime disk_image 2>&1 | tail -15
cargo clippy -- -D warnings
```

期待: テスト 11 件が ok。この時点では公開関数が未使用のため clippy が dead_code 警告を出します (0 章の例外)。D1 から D3 は続けて実施し、clippy の警告ゼロは D3 完了後に確認してください (`cargo test` は警告があっても通ります)。

### 10.5 コミット

テストが通ったら次のとおりコミットします。

```
git add src/disk_image.rs src/main.rs
git commit -m "feat(docker): 疎ディスクイメージの実サイズと案内文を扱う disk_image を追加"
```

## 11. Task D2: GenericCleaner への統合 (docker と colima の案内)

前提: S3 と D1 が完了していること。

### 11.1 失敗するテストを書く

`src/cleaners/generic.rs` 末尾の `mod tests` に追加します。

```rust
    fn write_sparse_image(path: &Path) {
        use std::io::Write;
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![1u8; 2 * 1024 * 1024]).unwrap();
        f.set_len(64 * 1024 * 1024).unwrap();
    }

    #[test]
    fn docker_advisory_reports_virtual_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_sparse_image(
            &tmp.path()
                .join("Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw"),
        );
        let runner = crate::test_helpers::MockRunner::new().with_success("docker");
        let cleaner = GenericCleaner::docker(tmp.path(), Box::new(runner));
        let text = cleaner.advisory().expect("advisory when Docker.raw exists");
        assert!(text.contains("Docker.raw"), "{text}");
        assert!(text.contains("Disk image size"), "{text}");
    }

    #[test]
    fn docker_advisory_is_none_without_virtual_disk() {
        let tmp = tempfile::TempDir::new().unwrap();
        let runner = crate::test_helpers::MockRunner::new().with_success("docker");
        assert!(GenericCleaner::docker(tmp.path(), Box::new(runner))
            .advisory()
            .is_none());
    }

    #[test]
    fn docker_detect_does_not_run_commands_and_stays_pruneable_zero() {
        let tmp = tempfile::TempDir::new().unwrap();
        struct ExistsOnlyRunner;
        impl CommandRunner for ExistsOnlyRunner {
            fn run(&self, _: &str, _: &[&str]) -> anyhow::Result<std::process::Output> {
                unreachable!("detect must not execute external commands")
            }
            fn exists(&self, _: &str) -> bool {
                true
            }
        }
        let cleaner = GenericCleaner::docker(tmp.path(), Box::new(ExistsOnlyRunner));
        assert!(matches!(cleaner.detect().status, ScanStatus::Pruneable(0)));
    }

    #[test]
    fn colima_advisory_reports_disk_images() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_sparse_image(&tmp.path().join(".colima/_lima/colima/disk"));
        let cleaner = GenericCleaner::colima_prune(tmp.path(), Box::new(MissingToolRunner));
        let text = cleaner.advisory().expect("advisory when colima disk exists");
        assert!(text.contains("[colima]"), "{text}");
        assert!(text.contains("fstrim"), "{text}");
    }

    #[test]
    fn non_vm_cleaner_has_no_advisory() {
        let cleaner = GenericCleaner::npm(Box::new(MissingToolRunner));
        assert!(cleaner.advisory().is_none());
    }
```

### 11.2 実行と期待される失敗

```
cargo test --bin sasurahime docker_ 2>&1 | tail -15
```

期待: コンパイルエラー (`GenericCleaner::docker` の引数数が一致しない)。

### 11.3 実装 (src/cleaners/generic.rs)

(1) import に追加します。

```rust
use crate::disk_image::{
    colima_candidates, docker_desktop_candidates, DiskImageNotice, COLIMA_REMEDY, DOCKER_REMEDY,
};
```

(2) `GenericCleaner` 構造体の末尾 (`larger_than_mb` の次) にフィールドを追加します。

```rust
    /// Sparse VM disk image that pruning inside the VM may not shrink.
    notice: Option<DiskImageNotice>,
```

`base_cleaner` の `Self { ... }` の `larger_than_mb: None,` の次に `notice: None,` を追加します。

(3) `impl GenericCleaner` の `command_cleaner` の直後にメソッドを追加します。

```rust
    pub fn with_notice(mut self, notice: DiskImageNotice) -> Self {
        self.notice = Some(notice);
        self
    }
```

(4) `docker` を置換します。

HEAD:

```rust
    pub fn docker(runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("docker", "docker", &["system", "prune", "-f"], runner)
    }
```

置換後:

```rust
    pub fn docker(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self::command_cleaner("docker", "docker", &["system", "prune", "-f"], runner).with_notice(
            DiskImageNotice::new("docker", docker_desktop_candidates(home), DOCKER_REMEDY),
        )
    }
```

(5) `colima_prune` を置換します。HEAD は `Self { confirm_message: ..., fallback_delete: true, ..Self::base_cleaner(...) }` で終わっています。最後の閉じ括弧 `}` の後ろにメソッド呼び出しを付けます。

```rust
    pub fn colima_prune(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            confirm_message: Some("This will delete ALL stopped Colima VM disk data (containers, images, volumes). Continue?"),
            fallback_delete: true,
            ..Self::base_cleaner(
                "colima",
                CleanMethod::CommandWithDetectDir {
                    program: "colima",
                    args: &["prune", "--all", "--force"],
                    detect_dir: home.join(".colima"),
                },
                runner,
            )
        }
        .with_notice(DiskImageNotice::new(
            "colima",
            colima_candidates(home),
            COLIMA_REMEDY,
        ))
    }
```

(6) `impl Cleaner for GenericCleaner` に `advisory` を追加します (`fn name` の直前)。

```rust
    fn advisory(&self) -> Option<String> {
        self.notice.as_ref().and_then(|n| n.render())
    }
```

(7) Command 型の clean 後処理。`CleanMethod::Command` アームの次の 2 行 (CommandWithDetectDir アームには `let output =` が付くので、`println!` を含めて一意に指定します) を置換します。

HEAD:

```rust
                println!("$ {program} {}", args.join(" "));
                self.runner.run(program, args)?;
                Ok(CleanResult {
```

置換後:

```rust
                println!("$ {program} {}", args.join(" "));
                let image_before = self.notice.as_ref().map_or(0, |n| n.physical_total());
                let output = self.runner.run(program, args)?;
                if !output.status.success() {
                    eprintln!(
                        "[{}] warning: `{} {}` exited with code {:?}",
                        self.display_name,
                        program,
                        args.join(" "),
                        output.status.code()
                    );
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if !stderr.is_empty() {
                        eprintln!("[{}] stderr: {stderr}", self.display_name);
                    }
                } else if let Some(msg) = self
                    .notice
                    .as_ref()
                    .and_then(|n| n.after_clean_message(image_before, n.physical_total()))
                {
                    println!("{msg}");
                }
                Ok(CleanResult {
```

非 0 終了 (docker デーモン停止など) では縮小判定を出さず、警告だけを出します。戻り値は従来どおり `bytes_freed: 0` の Ok です (終了コードは変えません)。

(8) `src/main.rs` の Docker 行を置換します。

HEAD:

```rust
    (|_home, _config| cleaners::generic::GenericCleaner::docker(Box::new(SystemCommandRunner))),
```

置換後:

```rust
    (|home, _config| cleaners::generic::GenericCleaner::docker(home, Box::new(SystemCommandRunner))),
```

### 11.4 実行と期待出力

```
cargo test --bin sasurahime 2>&1 | tail -8
cargo test --test generic 2>&1 | tail -8
cargo fmt --check
cargo clippy -- -D warnings
```

期待: 追加テスト 5 件を含め全件 ok。`tests/generic.rs` の `clean_tool_not_found_skips::case_6_docker` が ok (docker 不在時の挙動は不変)。fmt は出力なし。clippy は `advisory` と、それを通してしか呼ばれない `DiskImageNotice::render` などの `dead_code` 警告だけが残ってかまいません (D3 で消えます。0 章の例外)。

### 11.5 コミット

```
git add src/cleaners/generic.rs src/main.rs
git commit -m "feat(docker): docker と colima に仮想ディスク案内を付け、prune 失敗時の警告を出す"
```

## 12. Task D3: scan に案内を出力し E2E で固定する

前提: D2 が完了していること。

### 12.1 失敗するテストを書く

`tests/scan_coverage.rs` を新規作成します。

```rust
use assert_cmd::Command;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home);
    cmd
}

fn install_script(bin_dir: &Path, name: &str, body: &str) {
    fs::create_dir_all(bin_dir).unwrap();
    let path = bin_dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn write_docker_raw(home: &Path) {
    let p = home.join("Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw");
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    let mut f = fs::File::create(&p).unwrap();
    f.write_all(&vec![1u8; 2 * 1024 * 1024]).unwrap();
    f.set_len(64 * 1024 * 1024).unwrap();
}

#[test]
fn scan_shows_docker_virtual_disk_advice() {
    let tmp = TempDir::new().unwrap();
    write_docker_raw(tmp.path());
    let bin = tmp.path().join("bin");
    install_script(&bin, "docker", "exit 0");

    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Docker.raw"), "stdout: {stdout}");
    assert!(stdout.contains("on disk:"), "stdout: {stdout}");
    assert!(stdout.contains("Disk image size"), "stdout: {stdout}");
}

#[test]
fn scan_omits_docker_advice_when_docker_is_not_installed() {
    let tmp = TempDir::new().unwrap();
    write_docker_raw(tmp.path());

    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .arg("scan")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("Docker.raw"), "stdout: {stdout}");
}

#[test]
fn scan_does_not_hang_when_docker_daemon_is_unresponsive() {
    let tmp = TempDir::new().unwrap();
    let bin = tmp.path().join("bin");
    install_script(&bin, "docker", "sleep 120");

    let output = sasurahime(tmp.path())
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .timeout(Duration::from_secs(25))
        .arg("scan")
        .output()
        .unwrap();

    assert!(output.status.success());
}
```

### 12.2 実行と期待される失敗

```
cargo test --test scan_coverage 2>&1 | tail -15
```

期待: `scan_shows_docker_virtual_disk_advice` が失敗 (scan が advisory を出力しないため)。残り 2 件は ok。

### 12.3 実装 (src/scanner.rs)

`run_scan` 末尾の次のブロックを置換します。

HEAD:

```rust
    if total > 0 {
        println!("\nTotal reclaimable: {}", format_bytes(total));
    }
}
```

置換後:

```rust
    if total > 0 {
        println!("\nTotal reclaimable: {}", format_bytes(total));
    }

    let notes: Vec<String> = available.iter().filter_map(|c| c.advisory()).collect();
    if !notes.is_empty() {
        println!("\nNotes:");
        for note in notes {
            println!("{note}");
        }
    }
}
```

### 12.4 実行と期待出力

```
cargo test --test scan_coverage 2>&1 | tail -8
cargo test --test scan 2>&1 | tail -8
cargo fmt --check
cargo clippy -- -D warnings
```

期待: scan_coverage 3 件が ok、既存の `tests/scan.rs` も全件 ok。clippy と fmt は出力なし (ここで S2 以降の `dead_code` 警告がすべて消えていること)。

### 12.5 コミット

```
git add src/scanner.rs tests/scan_coverage.rs
git commit -m "feat(scan): 表の下に仮想ディスクの案内を出力する"
```

## 13. Task T4: scan 網羅の E2E と cargo detect の出力抑止

前提: S3 と D3 が完了していること。

### 13.1 失敗するテストを書く

`tests/scan_coverage.rs` に追加します (D3 のヘルパー `install_script` を使います)。

```rust
const FAKE_TOOLS: &[&str] = &[
    "uv", "brew", "mise", "bun", "go", "pip", "npm", "yarn", "pnpm", "deno", "pipx", "docker",
    "orb", "gem", "bundle", "dotnet", "pod", "conda", "poetry", "colima", "xcrun", "mvn", "dart",
    "rustup",
];

const STANDARD_SCAN_NAMES: &[&str] = &[
    "act", "uv", "brew", "mise", "browsers", "bun", "go", "pip", "node-gyp", "npm", "yarn", "pnpm",
    "cargo", "docker", "orbstack", "cocoapods", "colima", "spm", "conda", "poetry", "pipx", "deno",
    "dotnet", "gem", "bundle", "rustup", "simulator", "gradle", "huggingface", "pre-commit",
    "jetbrains", "downloads", "vscode-extensions", "maven", "terraform", "flutter", "volta", "sbt",
    "tree-sitter",
];

fn first_cells(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter(|l| l.trim_start().starts_with('|'))
        .filter_map(|l| l.split('|').nth(1).map(|c| c.trim().to_string()))
        .collect()
}

fn scan_with_fake_tools(home: &Path) -> String {
    let bin = home.join("bin");
    for tool in FAKE_TOOLS {
        install_script(&bin, tool, "exit 0");
    }
    let output = sasurahime(home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .args(["--no-unicode", "scan"])
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn scan_lists_every_standard_target() {
    let tmp = TempDir::new().unwrap();
    let stdout = scan_with_fake_tools(tmp.path());
    let cells = first_cells(&stdout);
    let missing: Vec<&&str> = STANDARD_SCAN_NAMES
        .iter()
        .filter(|n| !cells.iter().any(|c| c == **n))
        .collect();
    assert!(missing.is_empty(), "missing from scan: {missing:?}\n{stdout}");
}

#[test]
fn scan_does_not_print_cargo_progress_lines() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("proj/target");
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("artifact"), vec![1u8; 4096]).unwrap();
    let stdout = scan_with_fake_tools(tmp.path());
    assert!(!stdout.contains("[cargo]"), "stdout: {stdout}");
}
```

注意: `rustup` の fake は `rustup toolchain list` に空出力で成功し、cleaner は Clean を返します。`first_cells` は `--no-unicode` の ASCII 表 (`|` 区切り) を前提にします。

### 13.2 実行と期待される失敗

```
cargo test --test scan_coverage 2>&1 | tail -15
```

期待: `scan_lists_every_standard_target` は S3 実装済みなら ok。`scan_does_not_print_cargo_progress_lines` が失敗 (cargo の detect が `[cargo] found 1 target/ directory(ies)` を標準出力へ出すため)。

### 13.3 実装 (src/cleaners/cargo.rs)

`detect` 内の 2 つの `println!` を置換します。

HEAD:

```rust
            println!("[cargo] registry cache: {}", crate::format::format_bytes(s));
```

置換後:

```rust
            log::info!("[cargo] registry cache: {}", crate::format::format_bytes(s));
```

HEAD:

```rust
            println!("[cargo] found {} target/ directory(ies)", targets.len());
```

置換後:

```rust
            log::info!("[cargo] found {} target/ directory(ies)", targets.len());
```

理由: scan は rayon で並列に detect するため、標準出力への println! が進捗バーと表の出力に割り込みます。`clean` 側の `println!` は変更しません。

### 13.4 実行と期待出力

```
cargo test --test scan_coverage 2>&1 | tail -8
cargo test 2>&1 | grep -E "test result|FAILED|failed" 
cargo fmt --check
cargo clippy -- -D warnings
```

期待: scan_coverage 5 件が ok。全テストの `test result` がすべて `ok`、`FAILED` なし。

### 13.5 コミット

```
git add src/cleaners/cargo.rs tests/scan_coverage.rs
git commit -m "test(scan): 標準ターゲット全件が scan に出ることを E2E で固定し、cargo detect の標準出力を抑止"
```

## 14. Task T5: ドキュメント更新

前提: T4 が完了していること。

### 14.1 docs/SUPPORTED.md

1. 10 行目 (英語) の `**49 clean targets**` を `**48 clean targets**` に、1348 行付近 (日本語) の `**49 のクリーンターゲット**` を `**48 のクリーンターゲット**` に変更します。実測値は `cargo run -q -- targets | wc -l` の出力 (48) です。後続の 02/03/05 がそれぞれ 1 ずつ増やします (共通規約 5.2 の表)。本書では新しい節を追加せず、stats 節の番号 (49) も変えません。
2. 英語の `## 20. sasurahime clean docker` 節の `**Safety:**` の直前に追加します。

```
**Virtual disk note:**
`sasurahime scan` prints the size of the Docker Desktop virtual disk
(`~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw`, plus the
older layouts) under the table: bytes actually on disk and the maximum size.
`docker system prune` frees space inside the VM but the file does not always
shrink. After `clean docker` the on-disk size is compared before and after; if
it did not shrink, shrink it from Docker Desktop > Settings > Resources > Disk
image size, or Troubleshoot > Clean / Purge data (deletes all images,
containers and volumes). Detect never talks to the Docker daemon, so scan does
not hang when the daemon is stopped.
```

3. 日本語の `## 20. sasurahime clean docker` 節の `**安全性:**` の直前に追加します。

```
**仮想ディスクの注意:**
`sasurahime scan` は表の下に Docker Desktop の仮想ディスク
(`~/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw` と旧版の配置)
の実際のディスク使用量と最大サイズを表示します。`docker system prune` は VM 内の
領域を空けますが、ファイルは必ずしも縮みません。`clean docker` の前後でディスク
使用量を比較し、縮まなかった場合は Docker Desktop の Settings > Resources > Disk
image size を小さくするか、Troubleshoot > Clean / Purge data (全イメージ、コンテナ、
ボリュームを削除) を案内します。detect は docker デーモンに問い合わせないため、
デーモン停止中でも scan はハングしません。
```

4. 英語の `## Interactive / Auto` 表の `--yes` 行を次に置換します。

```
| `sasurahime --yes` (no args, any) | Cleans every pruneable target in the unattended set (uv, brew, mise, browsers, xcode, logs, act, huggingface, pre-commit, library-logs, colima, ollama, device-support, ios-backup, apfs-snapshot) without prompting. All other targets (docker, npm, cargo, downloads, ...) appear in `scan` and the interactive list but are never run by `--yes`. Exits with 0 if nothing is found. |
```

日本語の同じ表の `--yes` 行 (「確認なしで…」) も同じ内容の日本語に置換します。

```
| `sasurahime --yes`（引数なし、任意） | 無人実行の対象 (uv, brew, mise, browsers, xcode, logs, act, huggingface, pre-commit, library-logs, colima, ollama, device-support, ios-backup, apfs-snapshot) のうち pruneable なものを確認なしで削除します。それ以外のターゲット (docker, npm, cargo, downloads など) は `scan` と対話リストには表示されますが、`--yes` では実行されません。何も見つからなければ 0 で終了します。 |
```

### 14.2 README.md

57 行の `sasurahime --yes` 行のコメントを `# non-interactive clean of the unattended target set (CI / scripting)` に、168 行の日本語行を `# 確認なしで無人実行対象のみ削除（CI・スクリプト向け）` に変更します。「40+」は実数 48 (後続の 02/03/05 の後は 51) に対して正しいので変更しません。

「Data access」節のディレクトリの一覧に 1 行ずつ追加します (共通規約 11.5)。本書は仮想ディスクイメージのメタデータ (`st_blocks` による物理サイズと `metadata().len()` による論理サイズ) だけを読み、ファイルの内容は読みません。そのため、P0 (ロードマップ 7 章) で作った「内容を読むファイル」の一覧と、その前後の文は変更しません (共通規約 1.6)。

- 英語 (90 行付近、`- \`~/Library/Application Support/MobileSync/Backup/\`` で始まる行の次):

  ```
  - `~/Library/Containers/com.docker.docker/Data/` and `~/.colima/` (size of the virtual disk image files only, shown by `sasurahime scan`)
  ```

- 日本語 (200 行付近、`- \`~/Library/Application Support/MobileSync/Backup/\`` で始まる行の次):

  ```
  - `~/Library/Containers/com.docker.docker/Data/` と `~/.colima/`（仮想ディスクイメージのファイルサイズのみ。`sasurahime scan` で表示）
  ```

### 14.3 docs/HOWTO-USE.md

`### sasurahime --yes` の節 (英語 244 行付近、日本語 808 行付近) の先頭の説明文の後に、SUPPORTED.md 14.1 の 4 と同じ趣旨の 1 文を追加します。

- 英語: `Only the unattended target set is cleaned; docker, npm, cargo, downloads and the other targets are shown by scan but never run by --yes.`
- 日本語: `削除されるのは無人実行対象のみです。docker、npm、cargo、downloads などは scan に表示されますが、--yes では実行されません。`

### 14.4 docs/HOWTO-ADD-target.md

36-55 行付近の「`all_cleaners()` への手動編集は不要」の記述に、次の 1 文を英語節と日本語節にそれぞれ追加します。新規ターゲットは自動で scan に出るが `--yes` 対象にはならない旨と、対象にしたい場合は `AUTO_CLEAN_TARGETS` に name() を追加する旨です。

- 英語: `New standard targets appear in scan automatically but are excluded from --yes; add the cleaner's name() to AUTO_CLEAN_TARGETS in src/main.rs only if it is safe to run unattended.`
- 日本語: `新しい標準ターゲットは自動で scan に出ますが --yes の対象外です。無人実行しても安全な場合だけ、src/main.rs の AUTO_CLEAN_TARGETS に name() を追加してください。`

### 14.5 確認とコミット

```
grep -n "49" docs/SUPPORTED.md | head
git diff --stat
```

期待: `49 clean targets` と `49 のクリーンターゲット` が残っていない。

```
git add docs/SUPPORTED.md README.md docs/HOWTO-USE.md docs/HOWTO-ADD-target.md
git commit -m "docs: docker 仮想ディスクの案内、--yes の対象範囲、ターゲット数を最新仕様に更新"
```

## 15. エッジケース表

| 状況 | 期待される挙動 |
|---|---|
| docker CLI が無い | `is_available()` が false で scan に出ない。案内も出ない |
| docker CLI があり Docker Desktop が無い (Colima のみ等) | docker 行は `Pruneable(0)`、候補ファイルが無いので案内なし |
| docker デーモン停止中 | scan は docker を実行しないので即時終了。`clean docker` は終了コード非 0 の警告を stderr に出し、縮小判定は出さない |
| Docker.raw が 1 MiB 未満 | 案内なし |
| Docker.raw が複数候補に存在 | 存在する全ファイルを列挙 |
| 物理サイズが論理サイズより大きい (非疎ファイル) | そのまま両方表示 |
| Colima のプロファイルが複数 | `_lima/*/disk` と `_lima/_disks/*/datadisk` をすべて列挙 |
| `config.exclude` に `docker` | all_cleaners から除外され、scan にも出ない |
| `config.exclude` に `cocoa-pods` と書いた | 効かない。`cocoapods` と書く (name() で照合) |
| `per_cleaner` の `older_than_days` を command 型に設定 | 検出と削除の挙動は変えない。警告ログ (`log::warn!`) が出るのは factory が `with_config` を呼ぶ cleaner だけです。Command 系では colima だけが該当し、他の Command 系 (docker、npm など) は factory の `_config` で設定を無視するため警告も出ません (別 PBI、ロードマップ 8 章) |
| custom cleaner | 従来どおり `--yes` 対象 (ManualOnly で包まない) |
| colima の `per_cleaner` 設定 | 警告ログを維持します。HEAD の手書き `all_cleaners()` は colima に `.with_config(config)` を付けていましたが、マクロ表の colima factory は付けていません。S3 でマクロ表の colima の行に `.with_config(config)` を付けるので、S3 後は scan (`all_cleaners()` 経由) と `clean colima` (`dispatch_clean` 経由) の両方で `per-cleaner filters not supported for 'colima': command-based cleaner` の警告が出ます。検出と削除の挙動は変わりません (`colima_with_config_keeps_detect_result` で確認) |
| TUI で `npm (0 B)` を選ぶ | 利用者の明示選択なので `npm cache clean --force` を実行 |
| rustup の detect が 30 秒でタイムアウト | `Err` を `NotFound` として扱う (既存実装) |
| 環境変数 `ACT_CACHE_DIR`、`PUB_CACHE`、`TF_PLUGIN_CACHE_DIR` が設定されている | HOME ではなく環境変数のパスを走査する (既存仕様)。E2E は `env_remove` が必要な場合がある |
| scan の並列実行中に cargo が HOME を走査 | 深さ 5 まで。大きな HOME では数秒かかりうる (許容) |

## 16. やりがちな間違い

1. S3 を S2 より先に実施する。`--yes` が docker prune や `~/Downloads` 削除を実行します。順序を守ってください。
2. `all_cleaners` を再び手書きの `vec!` に戻す、または新規ターゲットを `special_cleaners` に重複して書く。重複テストが検出します。
3. マクロ引数の factory を変えるとき、`CleanTarget` の clean ディスパッチ (`dispatch_clean`) も同じクロージャを使うことを忘れる。docker の引数変更は 1 か所 (マクロ表) だけで済みます。
4. docker の detect で `docker system df` や `docker info` を呼ぶ。デーモン停止、起動中に最大 30 秒ブロックし、並列 scan 全体が遅くなります。
5. 論理サイズ (`metadata().len()`) を「使用量」として表示する。疎ファイルでは実使用量の数倍から数十倍になります。主表示は物理 (blocks * 512) です。
6. 案内を `Pruneable` のバイト数に加算する。prune で縮まない領域を「解放可能」と表示することになります。
7. `exclude` に CLI 名を使う。照合は `name()` です (`cocoapods` のみ CLI 名と異なる)。
8. `ManualOnly` で `clean_with_opts` や `sub_targets` の転送を忘れる。xcode のサブターゲットや apfs-snapshot のオプションが壊れます (ただし新規 31 件は現状これらを使わないため、将来のための必須実装)。
9. E2E で実際の `~/` を対象に `clean` を実行する。禁止です (17 節)。
10. `tests/` の E2E で `HOME` だけを差し替えて PATH を差し替えない。実機の docker や npm が見え、結果が環境依存になります。
11. S3 でマクロ表の colima 以外の行にも `.with_config(config)` を付ける。`_config` を受け取る残り 29 件は変えません (別 PBI、ロードマップ 8 章)。
12. `unsupported_filter_warning` を `pub` にする、または警告の文面を変える。private のままにし、文面は HEAD の `log::warn!` と同じにします。
13. 警告をテストで捕まえるために `log::set_logger` やテスト用のロガーを追加する。本書では判定関数の単体テストで代えます (2.7 節)。

## 17. 禁止事項

- `sasurahime clean ...` や `sasurahime --yes` を実機の HOME で実行すること。
- `git add -A`、`git add .`。
- 本書で指定していないファイルの変更、既存 15 件の `--yes` 挙動の変更。
- 新規 31 件を `AUTO_CLEAN_TARGETS` に追加すること。
- コードコメントへの変更履歴、タスク ID、日本語の記述。
- ドキュメントに Shift-JIS で表せない文字を使うこと。

## 18. 完了条件

1. `cargo test` が全件成功し、`cargo clippy -- -D warnings` と `cargo fmt --check` が出力なしで終わる。
2. `sasurahime scan` の表に標準ターゲット 39 件 (利用可能なもの) と特殊 7 件が出る。
3. `--yes` の対象が既存 15 件のみである (S2 の E2E と S3 の単体テストで固定)。
4. Docker Desktop または Colima の仮想ディスクが存在する環境で、scan の表の下に `Notes:` と「on disk」「maximum size」「対処案」が出る。
5. docker デーモンが応答しなくても scan が 25 秒以内に終わる (D3 の E2E)。
6. `docs/SUPPORTED.md` の件数が `sasurahime targets | wc -l` の出力 (48) と一致する。
7. 各 Task が個別にコミットされている (S1, S2, S3, D1, D2, D3, T4, T5 の 8 コミット)。P0 のコミットは本書の 8 コミットに数えません。
8. マクロ表の colima の行が `.with_config(config)` を呼び、`colima_with_filter_config_takes_the_warning_branch`、`unsupported_filter_warning_is_none_without_entry_or_for_delete_dirs`、`colima_with_config_keeps_detect_result` が ok である。マクロ表の他の行の factory は HEAD と同じ (docker の D2 の変更を除く)。

## 19. 最終確認コマンド

```
cargo fmt --check
cargo clippy -- -D warnings
cargo test 2>&1 | grep -E "test result|FAILED|failed"
cargo build
./target/debug/sasurahime targets | wc -l
./target/debug/sasurahime --no-unicode scan
./target/debug/sasurahime --no-unicode scan --verbose
```

実機確認の手順 (読み取りのみ):

1. `./target/debug/sasurahime --no-unicode scan` を実行し、表に `docker`, `npm`, `cargo`, `simulator`, `spm` などが新たに出ることを確認します。インストールされていないツールの行は `is_available()` が false のため出ません。
2. この開発機には Colima があるため、表の下に `[colima] virtual disk image ...` と `~/.colima/_lima/colima/disk` の `on disk` と `maximum size (20.0 GB)`、`_disks/colima/datadisk` の最大サイズ (100.0 GB) が出ることを確認します (物理使用量が 1 MiB 以上の場合)。
3. `time ./target/debug/sasurahime --no-unicode scan` が数秒から十数秒で終わることを確認します (cargo の HOME 走査があるため、従来より長くなります)。
4. `./target/debug/sasurahime clean docker --dry-run` は `[dry-run] would run: docker system prune -f` と表示するだけで何も削除しないことを確認します (これのみ実行可)。
5. `./target/debug/sasurahime --yes` は実行しません。

## 20. 設計上のリスクと注意点

- Docker Desktop のパス 4 候補はこの機で実在確認できていません。最新版の配置が異なる場合、案内が出ないだけで害はありません。実機の Docker Desktop で `find ~/Library/Containers/com.docker.docker -name '*.raw' -o -name '*.qcow2'` を実行して候補を補正してください。
- Colima の案内文 `fstrim` は効果が VM の設定に依存し、未検証です。そのため文面を「ask the VM to discard」と断定を避けて書いています。
- scan に出る行が約 46 行に増え、cargo と各ディレクトリ走査の分だけ scan が遅くなります。並列実行のため合計は最も遅い cleaner に律速されます。
- Command 型の行は `0 B` / `pruneable` と表示されます (既存の仕様)。サイズ不明を意味します。
- `colima prune --all --force` の確認文言 (「VM のディスクデータを全て削除」) と実際のコマンドの作用の整合は未確認で、本書の範囲外です。
- docker prune は `COMMAND_TIMEOUT` (30 秒) で kill されるため、巨大な環境では完了前に打ち切られます。縮小判定は成功時のみ行います。
- `ManualOnly` は name() で `AUTO_CLEAN_TARGETS` と照合します。cleaner の name() を変更すると意図せず `--yes` から外れる (安全側) ため、単体テスト `only_legacy_targets_are_auto_clean_allowed` が検出します。
- 既知の既存問題 (今回は触らない): `ios-backup` は `--yes` で TTY 上だと確認プロンプトを出します (docs の「`--yes` では非実行」とも食い違います)。
