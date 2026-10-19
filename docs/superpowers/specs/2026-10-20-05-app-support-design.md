# app-support 実装設計書

対象: `sasurahime clean app-support`（`~/Library/Application Support` の整理）。
ブランチ: `feat/apfs-snapshot-enhancement` で、04、02、03 の完了後に続けて実装します。
共通規約は `docs/superpowers/specs/2026-10-20-00-common-conventions.md`、順序と衝突確認は
`2026-10-20-01-roadmap-and-order.md`、関連設計は `2026-10-20-02-orphan-containers-design.md` と
`2026-10-20-03-library-caches-design.md` を参照します。コードは写経できる形で載せています。

## 前提 (着手前に確認)

- 実装順は 04 -> 02 -> 03 -> 05 (本書) で固定です。本書が最後です。
- 前提タスク: 03 (library-caches) の全タスクが完了していること (その前提として 04 と 02 も完了済み)。本書が使う前提は次の 4 つです。
  - 04 の S2: `Cleaner::auto_clean_allowed()` (本書の cleaner は `false` を返す)。
  - 04 の S3: `standard_cleaners()` と `ManualOnly`。本書はマクロ表に標準エントリを 1 つ足すだけで登録します。
  - 03 の Task 1: `src/hint/detector.rs` の `pub(crate) fn is_running` と、`src/hint/mod.rs` の `pub(crate) use detector::is_running;`。本書は `is_running` の可視性を変更しません。
  - 02 の Task 2: `crate::cleaners::orphan_containers::collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String>`。
- `docs/SUPPORTED.md` は 03 の完了後の状態 (件数 50、`## 50. library-caches`、stats は `## 51.`) を前提にします。
- 確認コマンド (どれか 1 つでも期待と違えば着手せず報告):

  ```bash
  grep -n "fn auto_clean_allowed" src/cleaner.rs                          # 1 行出る
  grep -n "fn standard_cleaners" src/main.rs                             # 1 行出る
  grep -n "LibraryCaches :" src/main.rs                                  # 1 行出る
  grep -n "pub(crate) fn is_running" src/hint/detector.rs                # 1 行出る
  grep -n "pub(crate) use detector::is_running;" src/hint/mod.rs         # 1 行出る
  grep -n "pub fn collect_installed_ids" src/cleaners/orphan_containers.rs   # 1 行出る
  cargo run -q -- targets | wc -l                                        # 50
  ```

## 未検証の仮定

- 本書のコード片はコンパイル未確認です。
- Task 1 の抽出後も `auto_clean_hint` の呼び出し順と出力文言が変わらないこと (既存の hint テストが回帰テストになる前提)。`QUIT_TIMEOUT_SECS` と `QUIT_POLL_INTERVAL` が `src/hint/cleaner.rs` の既存定数であることは実コードで確認済みです。hint の `mod tests` に `RecordingRunner`、`VSCODE_ENTRY` (`relaunch_app: Some("Visual Studio Code")`)、`SLACK_ENTRY` (`relaunch_app: None`) があることも確認済みです。
- `plutil -convert json -o - <Info.plist>` が実機の `.app` で JSON を返すこと (第 2 段)。
- 第 2 段の E2E (`dry_run_shows_leftovers_without_deleting` など) は実機の `/Applications` を読み取りだけ行い、`ZzzNoSuchApp12345` に一致するアプリが無い前提です。
- `serde_json` は既存の依存 (`[dependencies]`)、`filetime` は既存の dev-dependency で、どちらも追加不要なことは `Cargo.toml` で確認済みです。
- 02 の `collect_installed_ids` が返す ID は小文字化済みです (02 の Task 2 のコードで確認済み)。本書の `InstalledApps::from_parts` は再度小文字化するので、どちらでも動きます。

## 1. 概要

### 1.1 目的

`~/Library/Application Support` 下の次の2つを扱います。

- 第1段(必須): 既知アプリ(Slack、Claude、Obsidian、VS Code、Chrome)のキャッシュ系サブディレクトリだけを、許可リスト方式で削除します。
- 第2段: アンインストール済みアプリの残骸らしきフォルダを表示だけします。削除はしません。

### 1.2 非目標

- ユーザーデータ(設定、ログイン情報、IndexedDB、Local Storage、Partitions、ワークスペース、Backups、User)の削除。許可リストに無いパスは一切削除できません。
- Chrome の Default 以外のプロファイル(Profile 1 など)。許可リストに入れません。
- 第2段の残骸の削除。`open <path>` で Finder を開くだけです。
- `--yes` での削除実行。`--yes` では絶対に何も消えません(`auto_clean_allowed()` が `false`。4.3 節)。
- hint 機能(`sasurahime scan` 後のヒント表示と `rm -rf` 自動実行)の挙動変更。
- Linux / Windows 対応。

### 1.3 第1段と第2段の境界

| 項目 | 第1段 | 第2段 |
|------|-------|-------|
| 対象 | 許可リストのキャッシュ系サブパス | Application Support 直下のフォルダ |
| 削除 | する(ゴミ箱が既定、ユーザーが選んだものだけ) | しない |
| detect() の合計 | 含める | 含めない |
| dry-run | 一覧のみ | 一覧のみ(表示は同じ) |
| 対話 | MultiSelect(既定は全部未選択) | MultiSelect で Finder を開く(既定は全部未選択) |
| 前提 | hint/apps.rs | orphan-containers の bundle ID 収集関数 `collect_installed_ids` |

第2段のタスク群(Task 8 以降)は第1段の完了後に、同じセッションで続けて実装します。

### 1.4 既存 hint 機能との関係

- `src/hint/apps.rs` の `KNOWN_ENTRIES` のうち `base_dir == BaseDir::AppSupport` の5件(表示名 "Slack"、"Claude (desktop)"、"Obsidian"、"VSCode caches"、"Google Chrome")を、本機能の許可リストのアプリ対応表の根拠にします。
- 実行中判定は 03 が公開した `crate::hint::is_running`、終了と再起動は本書の Task 1 で `hint::cleaner` から取り出して `crate::hint` から公開する関数を再利用します(3.3 節と4.1 節)。
- hint の `auto_clean_hint` は `rm -rf` で全 path_suffixes を直接消し、許可リスト検査も無く、ゴミ箱も使いません。本機能はこれを呼びません。終了と再起動の部分だけを関数に抽出して共有します。
- `sasurahime scan` は従来どおり hint を表示します。同じディレクトリが app-support の Pruneable 合計にも現れますが、二重計上にはなりません(hint は stderr のヒント、app-support は表の1行)。

## 2. ファイル一覧

| 区分 | パス | 責務 |
|------|------|------|
| 新規 | `src/cleaners/app_support.rs` | 許可リスト、パス検査、候補走査、Cleaner 実装、第2段の残骸検出 |
| 新規 | `tests/app_support.rs` | E2E(`--dry-run` のみ) |
| 変更 | `src/cleaners/mod.rs` | `pub mod app_support;` を1行追加 |
| 変更 | `src/hint/mod.rs` | `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` を1行追加、`mod tests` にテスト追加。`mod cleaner;` と `mod detector;` は private のまま |
| 変更 | `src/hint/cleaner.rs` | `quit_app_and_wait` と `relaunch_app` を抽出(挙動は不変) |
| 確認のみ | `src/hint/detector.rs` | 03 で `pub(crate) fn is_running` になっていることを確認する。本書では変更しない |
| 変更 | `src/main.rs` | `define_cleaners!` の標準エントリを1つ追加、単体テスト1件(6 章) |
| 変更 | `docs/SUPPORTED.md` | 件数 50 -> 51、表の行、第 51 節の追加と stats の 51 -> 52(英語節と日本語節) |
| 変更 | `docs/HOWTO-USE.md` | 対象表(英語と日本語)に1行ずつ |

`src/explorer.rs`、`src/interactive.rs`、`src/cleaner.rs`、`README.md` は変更しません。Finder 起動は `explorer::open_in_finder` (中で `std::process::Command` を直接呼ぶ) を使わず、runner 経由の `open <path>` で行います(共通規約 3.1)。

## 3. 許可リスト

### 3.1 定数表

Application Support を `AS` と書きます。サブパスは `AS/<root>/<sub>` を指します。この表に無いパスは削除できません。

| アプリ(hint の display_name) | root | 許可サブパス | 閾値 | pgrep -x | quit_app | relaunch_app | バンドル ID(参考) |
|---|---|---|---|---|---|---|---|
| Slack | `Slack` | `Cache`、`Code Cache`、`GPUCache`、`Service Worker/CacheStorage`、`Service Worker/ScriptCache`、`logs` | 64 MiB | Slack | Slack | なし | com.tinyspeck.slackmacgap |
| Claude (desktop) | `Claude` | `Cache`、`Code Cache`、`GPUCache`、`DawnGraphiteCache`、`DawnWebGPUCache`、`Service Worker/CacheStorage`、`Service Worker/ScriptCache` | 64 MiB | Claude | Claude | なし | com.anthropic.claudefordesktop |
| Obsidian | `obsidian` | `Cache`、`Code Cache`、`GPUCache`、`DawnGraphiteCache`、`DawnWebGPUCache`、`Service Worker/CacheStorage`、`Service Worker/ScriptCache` | 64 MiB | Obsidian | Obsidian | Obsidian | md.obsidian |
| VSCode caches | `Code` | `Cache`、`CachedData`、`CachedExtensionVSIXs`、`CachedProfilesData`、`Code Cache`、`GPUCache`、`DawnGraphiteCache`、`DawnWebGPUCache`、`Service Worker/CacheStorage`、`Service Worker/ScriptCache`、`logs` | 64 MiB | Code | Visual Studio Code | Visual Studio Code | com.microsoft.VSCode |
| Google Chrome | `Google/Chrome` | `ShaderCache`、`GrShaderCache`、`GraphiteDawnCache`、`Default/Cache`、`Default/Cache_Data`、`Default/Code Cache`、`Default/GPUCache`、`Default/DawnGraphiteCache`、`Default/DawnWebGPUCache`、`Default/Service Worker/CacheStorage`、`Default/Service Worker/ScriptCache` | 64 MiB | Google Chrome | Google Chrome | Google Chrome | com.google.Chrome |

- 閾値、pgrep 名、quit_app、relaunch_app は hint の `HintEntry` から実行時に引きます(表と一致することをテストで保証します)。バンドル ID は説明用で、コードでは使いません。
- 閾値はアプリ単位です。そのアプリの許可サブパスの合計サイズが閾値を超えたときだけ、そのアプリの全候補を一覧に出します。
- 存在しないサブパスは黙って無視します。
- 許可サブパス同士に祖先関係を持たせません(二重計上と二重削除の防止)。テストで保証します。
- 禁止(許可リストに絶対入れない)例: `Local Storage`、`IndexedDB`、`Partitions`、`Cookies`、`Session Storage`、`User`、`Backups`、`Workspaces`、`Default/Login Data`、`Default/Local Storage`、`Default/IndexedDB`、`Default/Extensions`、`Default/Service Worker/Database`、`Default/Service Worker` 全体。

### 3.2 追加する場合の規則

新しいアプリを足すときは、先に `hint/apps.rs` に `BaseDir::AppSupport` のエントリを足し、同じ表示名で `TARGETS` に足します。hint の path_suffixes は必ず許可サブパスに含めます(テスト `targets_cover_every_hint_suffix` が検査します)。

### 3.3 実行中判定と終了・再起動の再利用

本書の着手時点(03 の完了後)のシグネチャは次のとおりです。

```rust
// src/hint/detector.rs (03 の Task 1 で pub(crate) 済み。src/hint/mod.rs で pub(crate) use 済み)
pub(crate) fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool
// 呼び出し: crate::hint::is_running(name, runner)

// src/hint/cleaner.rs (mod cleaner が private なので外から呼べない)
pub fn auto_clean_hint(hint: &ProcessHint, home: &Path, runner: &dyn CommandRunner) -> anyhow::Result<()>
```

決定: `auto_clean_hint` は呼びません(`rm -rf` と全 path_suffixes 削除を含むため)。代わりに、その前半(osascript で終了、1秒間隔で最大10秒 pgrep、最終確認)と後半(`open -a` で再起動)を関数として抽出し、`auto_clean_hint` 自身もそれを使うよう書き換えます。呼び出しの順序と出力文言は変えないので、既存の hint テストがそのまま回帰テストになります。

```rust
// src/hint/cleaner.rs に追加する pub(crate) 関数
pub(crate) fn quit_app_and_wait(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()>
pub(crate) fn relaunch_app(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()>
// src/hint/mod.rs で `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` して公開する。
// 呼び出し: crate::hint::quit_app_and_wait(entry, runner) / crate::hint::relaunch_app(entry, runner)
```

公開範囲の決定(03 と共通。共通規約 7.3、7.4): 関数は `pub(crate) fn`、公開は `src/hint/mod.rs` の `pub(crate) use` で行い、`mod cleaner;` と `mod detector;` は private のまま変えません。`pub use` で再エクスポートしません(`pub(crate)` の関数は `pub use` できません)。

## 4. 公開する型と関数

すべて `src/cleaners/app_support.rs` に置きます。

### 4.1 シグネチャ

```rust
pub const DEFAULT_MIN_APP_BYTES: u64 = 64 * 1024 * 1024;

pub struct AppSupportTarget {
    pub hint_name: &'static str, // HintEntry.display_name と一致させる
    pub root: &'static str,      // Application Support からの相対。'/' を含んでよい
    pub subpaths: &'static [&'static str],
}
pub const TARGETS: &[AppSupportTarget] = &[/* 3.1 節の表 */];
pub fn hint_entry(target: &AppSupportTarget) -> Option<&'static HintEntry>;

#[derive(Debug, PartialEq, Eq)]
pub enum GuardError { NotAllowed, NotFound, Symlink, NotDirectory, Unreadable, OutsideBase }
pub fn resolve_allowed(base: &Path, target: &AppSupportTarget, sub: &str)
    -> Result<PathBuf, GuardError>;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub target: &'static AppSupportTarget,
    pub sub: &'static str,
    pub path: PathBuf, // 表示用。削除では使わない(resolve_allowed を再実行する)
    pub size: u64,
}
pub fn scan_candidates(base: &Path, min_app_bytes: Option<u64>) -> Vec<Candidate>;
pub fn format_candidate_label(c: &Candidate, running: bool) -> String;

pub struct AppSupportCleaner { /* home, runner, min_app_bytes, app_roots */ }
impl AppSupportCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self;
    pub(crate) fn apply_selection(&self, selected: &[Candidate],
        quit_approved: &HashSet<&'static str>, reporter: &dyn ProgressReporter) -> CleanResult;
}
impl Cleaner for AppSupportCleaner { /* name() == "app-support" */ }

// 第2段
pub struct InstalledApps { /* 正規化済みの names / bundle_ids / bundle_tails */ }
impl InstalledApps {
    pub fn from_parts(names: impl IntoIterator<Item = String>,
                      bundle_ids: impl IntoIterator<Item = String>) -> Self;
}
pub fn normalize_name(s: &str) -> String;
pub fn is_excluded_dir_name(name: &str) -> bool;
pub fn matches_installed(dir_name: &str, installed: &InstalledApps) -> bool;
pub fn parse_plist_names(json: &str) -> Vec<String>;
pub fn collect_installed_apps(roots: &[PathBuf], runner: &dyn CommandRunner) -> InstalledApps;

#[derive(Debug, Clone)]
pub struct Leftover { pub path: PathBuf, pub size: u64 }
pub fn find_leftovers(base: &Path, installed: &InstalledApps, now: SystemTime,
                      min_bytes: u64, recent_days: u64) -> Vec<Leftover>;
```

hint 側で使うもの:

```rust
// src/hint/detector.rs (03 で変更済み。本書では変更しない)
pub(crate) fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool
// src/hint/cleaner.rs (本書の Task 1 で抽出)
pub(crate) fn quit_app_and_wait(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()>
pub(crate) fn relaunch_app(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()>
// いずれも crate::hint:: から呼ぶ
```

orphan-containers (02) が提供する関数(第2段の前提。Task 8 で存在を確認):

```rust
// src/cleaners/orphan_containers.rs (02 の Task 2 で実装済みの実シグネチャ)
pub fn collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String>;
// app_dirs の直下と 1 段下のサブフォルダにある *.app の Info.plist から CFBundleIdentifier を読み、小文字化して返す
```

シグネチャが上と違う場合は、推測で合わせずに止まって報告します。

### 4.2 入出力例

| 入力 | 出力 |
|------|------|
| `resolve_allowed(base, slack, "Cache")`、`AS/Slack/Cache` が実ディレクトリ | `Ok(AS/Slack/Cache)` |
| `resolve_allowed(base, slack, "Local Storage")` | `Err(NotAllowed)`(実在しても) |
| `resolve_allowed(base, slack, "../Code/Cache")` | `Err(NotAllowed)` |
| `resolve_allowed(base, slack, "/etc")` | `Err(NotAllowed)` |
| `AS/Slack` が外部へのシンボリックリンク | `Err(Symlink)` |
| `AS/Slack/Cache` がファイル | `Err(NotDirectory)` |
| `AS/Slack/Cache` が無い | `Err(NotFound)` |
| `AS/Slack` が権限 000 | `Err(Unreadable)` |
| `normalize_name("Google Chrome")` | `"googlechrome"` |
| `normalize_name("zoom.us")` | `"zoomus"` |
| `is_excluded_dir_name("com.apple.sharedfilelist")` | `true` |
| `is_excluded_dir_name("AddressBook")` | `true` |
| `is_excluded_dir_name(".DS_Store")` | `true` |
| `is_excluded_dir_name("Spotify")` | `false` |
| `matches_installed("Spotify", names={"spotify"})` | `true` |
| `matches_installed("slackmacgap", bundle_ids={"com.tinyspeck.slackmacgap"})` | `true`(末尾要素一致) |
| `matches_installed("com.foo.bar", bundle_ids={"com.foo.bar"})` | `true`(完全一致) |
| `parse_plist_names({"CFBundleName":"Code","CFBundleDisplayName":"Visual Studio Code"})` | `["Code","Visual Studio Code"]` |

### 4.3 ふるまいの決定事項

- `detect()`: Application Support が無ければ `NotFound`。許可リストの候補合計が 0 なら `Clean`、それ以外は `Pruneable(合計)`。第2段の残骸は含めません。pgrep も plutil も呼びません(軽量に保つため)。
- `clean(dry_run = true)`: 候補一覧(と第2段の残骸一覧)を stdout に出すだけです。osascript、chflags、削除、Finder 起動は一切しません。TTY でなくても動きます。
- `clean(dry_run = false)`: 標準入力が TTY でなければ、メッセージを出して何もせず `Ok`(ios-backup と同じ方式)。TTY なら MultiSelect(既定は全部未選択)。選択ゼロなら何もしません。
- `--yes`: `AppSupportCleaner` は `auto_clean_allowed()` で `false` を返します(共通規約 2.2)。`AUTO_CLEAN_TARGETS` にも入れないので `ManualOnly` に包まれ、`run_auto` の対象から外れます。`clean()` が呼ばれないので、TTY 上でもプロンプトは出ず、何も削除されません。TUI(引数なし起動)では選択すれば実行できます。
- 実行中のアプリ: 候補の選択後、実行中のアプリごとに `Confirm`(既定 No)で終了の許可を取ります。許可が無ければそのアプリの候補は skipped(理由 "X is running")になり削除しません。許可があれば `quit_app_and_wait` を呼び、失敗(10秒で終了しない等)なら skipped。削除後、`relaunch_app` を、こちらが終了させたアプリにだけ呼びます。
- 削除: 各候補で `resolve_allowed` を再実行して得たパスだけを使います(`Candidate.path` は使いません)。`chflags -R nouchg <path>` を runner 経由で実行(結果は無視)してから `crate::trash::delete_path`。失敗は skipped に積みます。
- 終了コード: `CleanResult::exit_code()` に従います(解放 0 かつ skipped があれば 1)。
- 第2段の Finder 起動: `explorer` の既存実装は未管理エントリを MultiSelect で選び、さらに Confirm で `open <path>` します。本機能は残骸一覧の MultiSelect(既定は未選択)で選んだものを、確認なしで `open <path>` します。理由: 開くだけで副作用が無く、選択自体が意思表示だからです。`open` は `explorer::open_in_finder` ではなく `self.runner.run("open", &[path])` で呼びます(cleaner から `std::process::Command` を直接呼ばない規約のため)。

## 5. タスク分割

共通ルール:

- 各タスクの最後でコミットします。`git add` は列挙したファイルだけを個別に指定します。
- 第1段の途中のコミットでは、未使用の公開項目による `dead_code` 警告と未使用 `use` の警告が出てかまいません(共通規約 1.4)。`cargo clippy -- -D warnings` は Task 6 の登録後と Task 7、Task 13 の完了時に必ず通します。
- テストでファイルを消すものは `crate::trash::set_trash_mode(false);` を先頭で呼びます(実機のゴミ箱を使わないため)。そのテストの最後の行には `crate::trash::set_trash_mode(true);` を足して戻します(共通規約 4.4)。本書のテストコードにはこの行が書かれていないので、写すときに足してください。
- 実機の `~/Library` は触りません。必ず `tempfile::TempDir` の疑似 HOME を使います。

### Task 1: hint の終了・再起動処理を関数に抽出する(第1段)

目的: `auto_clean_hint` の挙動を変えずに、終了待ちと再起動を再利用可能にします。

**1-1. 失敗するテストを書く。** `src/hint/mod.rs` の `mod tests` の末尾(最後の `}` の直前)に追加します。

```rust
    static NO_QUIT_ENTRY: HintEntry = HintEntry {
        base_dir: BaseDir::Caches,
        path_suffixes: &["x"],
        display_name: "NoQuit",
        process_name: None,
        commands: &[],
        threshold_bytes: 0,
        skip: false,
        quit_app: None,
        relaunch_app: None,
    };

    #[test]
    fn quit_app_and_wait_errors_when_entry_has_no_quit_app() {
        let runner = RecordingRunner::new(&[], 0);
        let err = cleaner::quit_app_and_wait(&NO_QUIT_ENTRY, &runner).unwrap_err();
        assert!(err.to_string().contains("cannot be auto-quit"));
        assert!(runner.recorded_calls().is_empty());
    }

    #[test]
    fn relaunch_app_opens_configured_app() {
        let runner = RecordingRunner::new(&[], 0);
        cleaner::relaunch_app(&VSCODE_ENTRY, &runner).unwrap();
        let calls = runner.recorded_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "open");
        assert_eq!(calls[0].1, vec!["-a".to_string(), "Visual Studio Code".to_string()]);
    }

    #[test]
    fn relaunch_app_does_nothing_without_relaunch_app() {
        let runner = RecordingRunner::new(&[], 0);
        cleaner::relaunch_app(&SLACK_ENTRY, &runner).unwrap();
        assert!(runner.recorded_calls().is_empty());
    }
```

**1-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime hint:: 2>&1 | tail -20
```

期待: `cannot find function quit_app_and_wait in module cleaner` でコンパイルエラー。

**1-3. 実装する。**

(a) `src/hint/mod.rs` の次の行の直後に 1 行追加します。`mod cleaner;` と `mod detector;` は private のまま変えません(`pub(crate) mod` にしません)。

```rust
pub use cleaner::{offer_auto_clean, print_hints};
```

の次の行に

```rust
pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};
```

を追加します。結果として先頭付近は次のようになります(`pub(crate) use detector::is_running;` は 03 が追加済みの行です)。

```rust
pub mod apps;
mod cleaner;
mod detector;

pub use cleaner::{offer_auto_clean, print_hints};
pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};
pub use detector::collect_hints;
pub(crate) use detector::is_running;
```

(b) `src/hint/detector.rs` は変更しません。03 の Task 1 で `is_running` は既に `pub(crate) fn` になっています。次のコマンドで確認だけします。

```bash
grep -n "pub(crate) fn is_running" src/hint/detector.rs
grep -n "pub(crate) use detector::is_running;" src/hint/mod.rs
```

期待: それぞれ 1 行出る。出ない場合は 03 が未完了なので、作業を止めて報告します(ここで自分で可視性を変えません)。

(c) `src/hint/cleaner.rs` の `use super::{base_path, ProcessHint, PromptReader};` を次にし、`auto_clean_hint` 全体を下の3関数に置き換えます(`clean_hint_dirs` 以降は変更しません)。

```rust
use super::{base_path, HintEntry, ProcessHint, PromptReader};
```

```rust
/// Quit the app via osascript and wait until its process is gone.
pub(crate) fn quit_app_and_wait(
    entry: &HintEntry,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let quit_app = match entry.quit_app {
        Some(a) => a,
        None => anyhow::bail!("{} cannot be auto-quit", entry.display_name),
    };

    eprintln!("  Quitting {}...", entry.display_name);
    runner.run("osascript", &["-e", &format!("quit app \"{quit_app}\"")])?;

    let process_name = entry.process_name.unwrap_or(quit_app);
    for _ in 0..QUIT_TIMEOUT_SECS {
        std::thread::sleep(QUIT_POLL_INTERVAL);
        let still_running = runner
            .run("pgrep", &["-x", process_name])
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !still_running {
            break;
        }
    }
    let still_running = runner
        .run("pgrep", &["-x", process_name])
        .map(|o| o.status.success())
        .unwrap_or(false);
    if still_running {
        anyhow::bail!(
            "{} did not quit within {}s \u{2014} skipping cache deletion",
            entry.display_name,
            QUIT_TIMEOUT_SECS
        );
    }
    Ok(())
}

/// Relaunch the app if the entry configures `relaunch_app`.
pub(crate) fn relaunch_app(entry: &HintEntry, runner: &dyn CommandRunner) -> anyhow::Result<()> {
    if let Some(app) = entry.relaunch_app {
        eprintln!("  Restarting {}...", entry.display_name);
        runner.run("open", &["-a", app])?;
        eprintln!("  [OK]");
    } else {
        eprintln!(
            "  ({} will not be restarted \u{2014} relaunch manually if needed)",
            entry.display_name
        );
    }
    Ok(())
}

/// Quit the app, delete caches, optionally relaunch. Returns Err if quit timed out.
pub fn auto_clean_hint(
    hint: &ProcessHint,
    home: &Path,
    runner: &dyn CommandRunner,
) -> anyhow::Result<()> {
    let entry = hint.entry;
    quit_app_and_wait(entry, runner)?;

    let base = base_path(home, entry.base_dir);
    eprintln!("  Clearing cache...");
    for suffix in entry.path_suffixes {
        let path = base.join(suffix);
        if path.exists() {
            let path_str = path.to_string_lossy();
            runner.run("rm", &["-rf", &path_str])?;
        }
    }
    eprintln!("  [OK]");

    relaunch_app(entry, runner)
}
```

**1-4. 実行して成功を確認(既存 hint テストも回帰になります)。**

```bash
cargo test --bin sasurahime hint:: 2>&1 | tail -20
```

期待: `test result: ok.` で失敗 0。`auto_clean_*` と `offer_auto_clean_*` の既存テストも通ります(各 1 秒程度かかります)。`cargo fmt --check` も通します。

`pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` は Task 5 まで使われないため、この時点の `cargo clippy -- -D warnings` は `unused import` で失敗します。共通規約 1.4 の例外として許容し、Task 6 で警告ゼロを確認します。

**1-5. コミット。**

```bash
git add src/hint/mod.rs src/hint/cleaner.rs
git commit -m "refactor(hint): アプリ終了待ちと再起動を再利用できる関数に抽出"
```

### Task 2: モジュールの骨格と許可リスト定数(第1段)

**2-1. 失敗するテストを書く。** `src/cleaners/app_support.rs` を新規作成し、まずテストだけを書きます(実装は 2-3 で足します。この時点では `TARGETS` が未定義でコンパイルエラーになります)。

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::Component;

    #[test]
    fn every_target_has_matching_hint_entry() {
        for t in TARGETS {
            let e = hint_entry(t).unwrap_or_else(|| panic!("no hint entry for {}", t.hint_name));
            assert!(e.process_name.is_some(), "{} needs process_name", t.hint_name);
            assert!(e.quit_app.is_some(), "{} needs quit_app", t.hint_name);
            assert!(e.threshold_bytes >= DEFAULT_MIN_APP_BYTES);
        }
    }

    #[test]
    fn every_app_support_hint_entry_has_a_target() {
        for e in KNOWN_ENTRIES.iter().filter(|e| e.base_dir == BaseDir::AppSupport) {
            assert!(
                TARGETS.iter().any(|t| t.hint_name == e.display_name),
                "hint entry {} has no TARGETS row",
                e.display_name
            );
        }
    }

    #[test]
    fn targets_cover_every_hint_suffix() {
        for e in KNOWN_ENTRIES.iter().filter(|e| e.base_dir == BaseDir::AppSupport) {
            let t = TARGETS.iter().find(|t| t.hint_name == e.display_name).unwrap();
            for s in e.path_suffixes {
                let rel = s
                    .strip_prefix(&format!("{}/", t.root))
                    .unwrap_or_else(|| panic!("{s} is not under root {}", t.root));
                assert!(t.subpaths.contains(&rel), "{s} missing from allowlist");
            }
        }
    }

    #[test]
    fn allowlist_paths_are_relative_normal_components() {
        for t in TARGETS {
            for p in std::iter::once(&t.root).chain(t.subpaths.iter()) {
                assert!(!p.is_empty());
                for c in Path::new(p).components() {
                    assert!(matches!(c, Component::Normal(_)), "bad path {p}");
                }
            }
        }
    }

    #[test]
    fn allowlist_has_no_duplicate_or_nested_paths() {
        for t in TARGETS {
            let mut seen = HashSet::new();
            for s in t.subpaths {
                assert!(seen.insert(*s), "duplicate {s}");
            }
            for a in t.subpaths {
                for b in t.subpaths {
                    if a != b {
                        assert!(
                            !Path::new(a).starts_with(Path::new(b)),
                            "{a} is nested under {b} in {}",
                            t.hint_name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn allowlist_never_contains_user_data_names() {
        const FORBIDDEN: &[&str] = &[
            "Local Storage",
            "IndexedDB",
            "Partitions",
            "Cookies",
            "Session Storage",
            "User",
            "Backups",
            "Workspaces",
            "Login Data",
            "Extensions",
            "Database",
        ];
        for t in TARGETS {
            for s in t.subpaths {
                assert_ne!(*s, "Service Worker", "whole Service Worker dir holds user data");
                for c in Path::new(s).components() {
                    let c = c.as_os_str().to_string_lossy();
                    assert!(!FORBIDDEN.contains(&c.as_ref()), "forbidden component {c} in {s}");
                }
            }
        }
    }
}
```

**2-2. 実行して失敗を確認。** 先に `src/cleaners/mod.rs` に `pub mod app_support;` を `pub mod apfs_snapshot;` の次の行へ追加します。

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find value TARGETS in this scope` などのコンパイルエラー。

**2-3. 実装する。** `src/cleaners/app_support.rs` の先頭(テストの前)に書きます。

```rust
use crate::hint::apps::KNOWN_ENTRIES;
use crate::hint::{BaseDir, HintEntry};
use std::path::Path;

pub const DEFAULT_MIN_APP_BYTES: u64 = 64 * 1024 * 1024;

/// One application whose cache subdirectories may be removed.
/// Only paths listed in `subpaths` are ever deletable; everything else under
/// `root` (settings, logins, IndexedDB, Local Storage, workspaces) is user data.
pub struct AppSupportTarget {
    pub hint_name: &'static str,
    pub root: &'static str,
    pub subpaths: &'static [&'static str],
}

pub const TARGETS: &[AppSupportTarget] = &[
    AppSupportTarget {
        hint_name: "Slack",
        root: "Slack",
        subpaths: &[
            "Cache",
            "Code Cache",
            "GPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
            "logs",
        ],
    },
    AppSupportTarget {
        hint_name: "Claude (desktop)",
        root: "Claude",
        subpaths: &[
            "Cache",
            "Code Cache",
            "GPUCache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
        ],
    },
    AppSupportTarget {
        hint_name: "Obsidian",
        root: "obsidian",
        subpaths: &[
            "Cache",
            "Code Cache",
            "GPUCache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
        ],
    },
    AppSupportTarget {
        hint_name: "VSCode caches",
        root: "Code",
        subpaths: &[
            "Cache",
            "CachedData",
            "CachedExtensionVSIXs",
            "CachedProfilesData",
            "Code Cache",
            "GPUCache",
            "DawnGraphiteCache",
            "DawnWebGPUCache",
            "Service Worker/CacheStorage",
            "Service Worker/ScriptCache",
            "logs",
        ],
    },
    AppSupportTarget {
        hint_name: "Google Chrome",
        root: "Google/Chrome",
        subpaths: &[
            "ShaderCache",
            "GrShaderCache",
            "GraphiteDawnCache",
            "Default/Cache",
            "Default/Cache_Data",
            "Default/Code Cache",
            "Default/GPUCache",
            "Default/DawnGraphiteCache",
            "Default/DawnWebGPUCache",
            "Default/Service Worker/CacheStorage",
            "Default/Service Worker/ScriptCache",
        ],
    },
];

/// Looks up the hint entry that carries process name, quit/relaunch app names
/// and the size threshold for this target.
pub fn hint_entry(target: &AppSupportTarget) -> Option<&'static HintEntry> {
    KNOWN_ENTRIES
        .iter()
        .find(|e| e.base_dir == BaseDir::AppSupport && e.display_name == target.hint_name)
}
```

**2-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 6 件 passed。

**2-5. コミット。**

```bash
git add src/cleaners/app_support.rs src/cleaners/mod.rs
git commit -m "feat(app-support): 既知アプリのキャッシュ許可リストを追加"
```

### Task 3: パス検査 resolve_allowed(第1段)

許可リスト方式がパストラバーサルとシンボリックリンクで迂回されないことを、このタスクのテストで保証します。

**3-1. 失敗するテストを書く。** `mod tests` の末尾に追加します。`use` 行も `mod tests` 内に足します。

```rust
    use std::fs;
    use tempfile::TempDir;

    fn target(name: &str) -> &'static AppSupportTarget {
        TARGETS.iter().find(|t| t.hint_name == name).unwrap()
    }

    fn base_of(tmp: &TempDir) -> PathBuf {
        let b = tmp.path().join("Library/Application Support");
        fs::create_dir_all(&b).unwrap();
        b
    }

    #[test]
    fn resolve_allows_listed_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        let p = resolve_allowed(&base, target("Slack"), "Cache").unwrap();
        assert_eq!(p, base.join("Slack/Cache"));
    }

    #[test]
    fn resolve_allows_nested_chrome_path() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Google/Chrome/Default/Service Worker/CacheStorage")).unwrap();
        let p = resolve_allowed(&base, target("Google Chrome"), "Default/Service Worker/CacheStorage");
        assert!(p.is_ok());
    }

    #[test]
    fn resolve_rejects_unlisted_user_data_even_if_it_exists() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        for d in ["Local Storage", "IndexedDB", "Partitions", "Service Worker"] {
            fs::create_dir_all(base.join("Slack").join(d)).unwrap();
            assert_eq!(
                resolve_allowed(&base, target("Slack"), d),
                Err(GuardError::NotAllowed),
                "{d}"
            );
        }
    }

    #[test]
    fn resolve_rejects_path_traversal_and_absolute() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        fs::create_dir_all(base.join("Code/Cache")).unwrap();
        for sub in [
            "../Code/Cache",
            "Cache/..",
            "Cache/../Local Storage",
            "../../..",
            "/etc",
            "",
            ".",
            "./Cache",
            "Cache/",
        ] {
            assert_eq!(
                resolve_allowed(&base, target("Slack"), sub),
                Err(GuardError::NotAllowed),
                "sub = {sub:?}"
            );
        }
    }

    #[test]
    fn resolve_rejects_symlinked_root() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("outside");
        fs::create_dir_all(outside.join("Cache")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack")).unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::Symlink)
        );
    }

    #[test]
    fn resolve_rejects_symlinked_leaf() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("Documents");
        fs::create_dir_all(&outside).unwrap();
        fs::create_dir_all(base.join("Slack")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack/Cache")).unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::Symlink)
        );
    }

    #[test]
    fn resolve_rejects_symlinked_middle_component() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("elsewhere");
        fs::create_dir_all(outside.join("CacheStorage")).unwrap();
        fs::create_dir_all(base.join("Slack")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack/Service Worker")).unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Service Worker/CacheStorage"),
            Err(GuardError::Symlink)
        );
    }

    #[test]
    fn resolve_rejects_file_where_dir_expected() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack")).unwrap();
        fs::write(base.join("Slack/Cache"), b"x").unwrap();
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::NotDirectory)
        );
    }

    #[test]
    fn resolve_reports_not_found_for_missing() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        assert_eq!(
            resolve_allowed(&base, target("Slack"), "Cache"),
            Err(GuardError::NotFound)
        );
        let missing_base = tmp.path().join("nope");
        assert_eq!(
            resolve_allowed(&missing_base, target("Slack"), "Cache"),
            Err(GuardError::NotFound)
        );
    }

    #[test]
    fn resolve_reports_unreadable_parent() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        fs::set_permissions(base.join("Slack"), fs::Permissions::from_mode(0o000)).unwrap();
        let r = resolve_allowed(&base, target("Slack"), "Cache");
        fs::set_permissions(base.join("Slack"), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(r, Err(GuardError::Unreadable));
    }
```

**3-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find function resolve_allowed` などのコンパイルエラー。

**3-3. 実装する。** ファイル先頭の `use std::path::Path;` を次の `use` 群に置き換え、`hint_entry` の後に追記します。

```rust
use std::fs;
use std::path::{Component, Path, PathBuf};
```

```rust
#[derive(Debug, PartialEq, Eq)]
pub enum GuardError {
    NotAllowed,
    NotFound,
    Symlink,
    NotDirectory,
    Unreadable,
    OutsideBase,
}

impl std::fmt::Display for GuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            GuardError::NotAllowed => "path is not in the allowlist",
            GuardError::NotFound => "path does not exist",
            GuardError::Symlink => "path contains a symbolic link",
            GuardError::NotDirectory => "path is not a directory",
            GuardError::Unreadable => "path is not readable",
            GuardError::OutsideBase => "path resolves outside Application Support",
        };
        f.write_str(s)
    }
}

/// The only way to obtain a deletable path. `sub` must equal one of
/// `target.subpaths` verbatim, and no component below `base` may be a symlink.
pub fn resolve_allowed(
    base: &Path,
    target: &AppSupportTarget,
    sub: &str,
) -> Result<PathBuf, GuardError> {
    if !target.subpaths.iter().any(|s| *s == sub) {
        return Err(GuardError::NotAllowed);
    }
    let rel = Path::new(target.root).join(sub);
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(GuardError::NotAllowed);
    }
    let mut cur = base.to_path_buf();
    for comp in rel.components() {
        cur.push(comp);
        let meta = match fs::symlink_metadata(&cur) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(GuardError::NotFound)
            }
            Err(_) => return Err(GuardError::Unreadable),
        };
        if meta.file_type().is_symlink() {
            return Err(GuardError::Symlink);
        }
        if !meta.is_dir() {
            return Err(GuardError::NotDirectory);
        }
    }
    let canonical_base = base.canonicalize().map_err(|_| GuardError::Unreadable)?;
    let canonical = cur.canonicalize().map_err(|_| GuardError::Unreadable)?;
    if !canonical.starts_with(&canonical_base) {
        return Err(GuardError::OutsideBase);
    }
    Ok(cur)
}
```

**3-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 16 件 passed。`resolve_rejects_unreadable_parent` は一般ユーザーで実行してください(root だと権限 000 でも読めて失敗します)。

**3-5. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "feat(app-support): 許可リスト外・.. ・シンボリックリンクを拒否するパス検査を追加"
```

### Task 4: 候補の走査 scan_candidates(第1段)

**4-1. 失敗するテストを書く。** `mod tests` の末尾に追加します。

```rust
    fn put(base: &Path, rel: &str, bytes: usize) {
        let p = base.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, vec![1u8; bytes]).unwrap();
    }

    #[test]
    fn scan_returns_only_allowlisted_dirs() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/Local Storage/b.bin", 10_000);
        put(&base, "Slack/IndexedDB/c.bin", 10_000);
        put(&base, "Slack/Partitions/d.bin", 10_000);
        let c = scan_candidates(&base, Some(0));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].path, base.join("Slack/Cache"));
        assert_eq!(c[0].sub, "Cache");
        assert!(c[0].size > 0);
    }

    #[test]
    fn scan_skips_app_below_threshold() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        assert!(scan_candidates(&base, Some(u64::MAX)).is_empty());
    }

    #[test]
    fn scan_uses_hint_threshold_by_default() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        assert!(scan_candidates(&base, None).is_empty());
    }

    #[test]
    fn scan_excludes_symlinked_cache_dir() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("Documents");
        put(&outside, "Cache/big.bin", 50_000);
        fs::create_dir_all(base.join("Slack")).unwrap();
        std::os::unix::fs::symlink(outside.join("Cache"), base.join("Slack/Cache")).unwrap();
        assert!(scan_candidates(&base, Some(0)).is_empty());
    }

    #[test]
    fn scan_skips_empty_dirs_and_missing_base() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        fs::create_dir_all(base.join("Slack/Cache")).unwrap();
        assert!(scan_candidates(&base, Some(0)).is_empty());
        assert!(scan_candidates(&tmp.path().join("nope"), Some(0)).is_empty());
    }

    #[test]
    fn scan_sorts_by_size_descending_across_apps() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 8_192);
        put(&base, "Code/CachedData/b.bin", 81_920);
        put(&base, "Code/logs/c.log", 4_096);
        let c = scan_candidates(&base, Some(0));
        let sizes: Vec<u64> = c.iter().map(|x| x.size).collect();
        let mut sorted = sizes.clone();
        sorted.sort_by(|a, b| b.cmp(a));
        assert_eq!(sizes, sorted);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn scan_threshold_is_per_app_total() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Code/Cache/a.bin", 40_000);
        put(&base, "Code/CachedData/b.bin", 40_000);
        put(&base, "Slack/Cache/c.bin", 40_000);
        let one = crate::format::dir_size(&base.join("Slack/Cache"));
        let c = scan_candidates(&base, Some(one + 1));
        assert!(c.iter().all(|x| x.target.hint_name == "VSCode caches"));
        assert_eq!(c.len(), 2);
    }
```

**4-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find function scan_candidates` のコンパイルエラー。

**4-3. 実装する。** `use` に `use crate::format::dir_size;` を足し、`resolve_allowed` の後に追記します。

```rust
#[derive(Debug, Clone)]
pub struct Candidate {
    pub target: &'static AppSupportTarget,
    pub sub: &'static str,
    /// For display only. Deletion re-resolves via `resolve_allowed`.
    pub path: PathBuf,
    pub size: u64,
}

pub fn scan_candidates(base: &Path, min_app_bytes: Option<u64>) -> Vec<Candidate> {
    let mut all = Vec::new();
    for target in TARGETS {
        let mut found = Vec::new();
        for sub in target.subpaths {
            let Ok(path) = resolve_allowed(base, target, sub) else {
                continue;
            };
            let size = dir_size(&path);
            if size == 0 {
                continue;
            }
            found.push(Candidate {
                target,
                sub: *sub,
                path,
                size,
            });
        }
        let total: u64 = found.iter().map(|c| c.size).sum();
        let threshold = min_app_bytes
            .or_else(|| hint_entry(target).map(|e| e.threshold_bytes))
            .unwrap_or(DEFAULT_MIN_APP_BYTES);
        if total > threshold {
            all.extend(found);
        }
    }
    all.sort_by_key(|c| std::cmp::Reverse(c.size));
    all
}
```

**4-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 23 件 passed。

**4-5. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "feat(app-support): 許可リストに沿った削除候補の走査を追加"
```

### Task 5: Cleaner 本体(detect、dry-run、対話、削除)(第1段)

**5-1. 失敗するテストを書く。** `mod tests` の末尾に追加します。`use` 行も `mod tests` 内に足します。

```rust
    use crate::command::CommandRunner;
    use crate::progress::DeepSuppressReporter;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use std::sync::{Arc, Mutex};

    fn out(code: i32) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code),
            stdout: vec![],
            stderr: vec![],
        }
    }

    #[derive(Default)]
    struct Inner {
        running: Mutex<HashSet<String>>,
        calls: Mutex<Vec<String>>,
    }

    #[derive(Clone, Default)]
    struct Fake(Arc<Inner>);

    impl Fake {
        fn with_running(names: &[&str]) -> Self {
            let f = Fake::default();
            for n in names {
                f.0.running.lock().unwrap().insert(n.to_string());
            }
            f
        }
        fn calls(&self) -> Vec<String> {
            self.0.calls.lock().unwrap().clone()
        }
    }

    impl CommandRunner for Fake {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<Output> {
            self.0
                .calls
                .lock()
                .unwrap()
                .push(format!("{program} {}", args.join(" ")));
            match program {
                "pgrep" => {
                    let n = args.last().copied().unwrap_or("");
                    let up = self.0.running.lock().unwrap().contains(n);
                    Ok(out(if up { 0 } else { 256 }))
                }
                "osascript" => {
                    self.0.running.lock().unwrap().clear();
                    Ok(out(0))
                }
                "open" | "chflags" => Ok(out(0)),
                _ => anyhow::bail!("unexpected command {program}"),
            }
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    fn cleaner_for(tmp: &TempDir, fake: &Fake) -> AppSupportCleaner {
        AppSupportCleaner::new(tmp.path(), Box::new(fake.clone())).with_min_app_bytes(0)
    }

    #[test]
    fn detect_is_not_found_without_application_support() {
        let tmp = TempDir::new().unwrap();
        let c = cleaner_for(&tmp, &Fake::default());
        assert!(matches!(c.detect().status, ScanStatus::NotFound));
        assert_eq!(c.name(), "app-support");
    }

    #[test]
    fn detect_sums_only_allowlisted_dirs() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/Local Storage/b.bin", 99_999);
        let c = cleaner_for(&tmp, &Fake::default());
        let r = c.detect();
        assert_eq!(r.bytes_for_test(), dir_size(&base.join("Slack/Cache")));
    }

    #[test]
    fn detect_is_clean_when_only_user_data_exists() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Local Storage/b.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        assert!(matches!(c.detect().status, ScanStatus::Clean));
    }

    #[test]
    fn dry_run_touches_nothing_and_runs_no_commands() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Slack"]);
        let c = cleaner_for(&tmp, &fake);
        let r = c.clean(true, &DeepSuppressReporter).unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert!(base.join("Slack/Cache/a.bin").exists());
        assert!(fake.calls().is_empty(), "calls: {:?}", fake.calls());
    }

    #[test]
    fn apply_deletes_only_selected_candidates() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/logs/l.log", 10_000);
        put(&base, "Slack/Local Storage/u.bin", 10_000);
        let fake = Fake::default();
        let c = cleaner_for(&tmp, &fake);
        let all = scan_candidates(&base, Some(0));
        let sel: Vec<Candidate> = all.into_iter().filter(|x| x.sub == "Cache").collect();
        let r = c.apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter);
        assert!(!base.join("Slack/Cache").exists());
        assert!(base.join("Slack/logs/l.log").exists());
        assert!(base.join("Slack/Local Storage/u.bin").exists());
        assert_eq!(r.bytes_freed, sel[0].size);
        assert!(r.skipped.is_empty());
        assert_eq!(r.exit_code(), 0);
    }

    #[test]
    fn apply_blocks_running_app_without_approval() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Slack"]);
        let c = cleaner_for(&tmp, &fake);
        let sel = scan_candidates(&base, Some(0));
        let r = c.apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter);
        assert!(base.join("Slack/Cache/a.bin").exists());
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert!(r.skipped[0].reason.contains("running"));
        assert!(!fake.calls().iter().any(|c| c.starts_with("osascript")));
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn apply_quits_approved_running_app_then_deletes_and_relaunches() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "obsidian/Cache/a.bin", 10_000);
        let fake = Fake::with_running(&["Obsidian"]);
        let c = cleaner_for(&tmp, &fake);
        let sel = scan_candidates(&base, Some(0));
        let approved: HashSet<&'static str> = HashSet::from(["Obsidian"]);
        let r = c.apply_selection(&sel, &approved, &DeepSuppressReporter);
        assert!(!base.join("obsidian/Cache").exists());
        assert!(r.bytes_freed > 0);
        let calls = fake.calls();
        let q = calls
            .iter()
            .position(|c| c == "osascript -e quit app \"Obsidian\"")
            .expect("quit call");
        let o = calls
            .iter()
            .position(|c| c == "open -a Obsidian")
            .expect("relaunch call");
        assert!(q < o);
    }

    #[test]
    fn apply_does_not_relaunch_app_that_was_not_running() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "obsidian/Cache/a.bin", 10_000);
        let fake = Fake::default();
        let c = cleaner_for(&tmp, &fake);
        let sel = scan_candidates(&base, Some(0));
        c.apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter);
        assert!(!fake.calls().iter().any(|c| c.starts_with("open")));
    }

    #[test]
    fn apply_never_deletes_forged_unlisted_sub() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Local Storage/u.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        let forged = Candidate {
            target: target("Slack"),
            sub: "Local Storage",
            path: base.join("Slack/Local Storage"),
            size: 1,
        };
        let r = c.apply_selection(&[forged], &HashSet::new(), &DeepSuppressReporter);
        assert!(base.join("Slack/Local Storage/u.bin").exists());
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.exit_code(), 1);
    }

    #[test]
    fn apply_ignores_forged_path_field() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        put(&base, "Slack/Local Storage/u.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        let forged = Candidate {
            target: target("Slack"),
            sub: "Cache",
            path: base.join("Slack/Local Storage"),
            size: 5,
        };
        c.apply_selection(&[forged], &HashSet::new(), &DeepSuppressReporter);
        assert!(!base.join("Slack/Cache").exists());
        assert!(base.join("Slack/Local Storage/u.bin").exists());
    }

    #[test]
    fn apply_does_not_follow_symlinks_inside_cache() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let outside = tmp.path().join("Documents");
        put(&outside, "keep.txt", 100);
        put(&outside, "dir/keep2.txt", 100);
        put(&base, "Slack/Cache/a.bin", 10_000);
        std::os::unix::fs::symlink(outside.join("keep.txt"), base.join("Slack/Cache/f")).unwrap();
        std::os::unix::fs::symlink(outside.join("dir"), base.join("Slack/Cache/d")).unwrap();
        let c = cleaner_for(&tmp, &Fake::default());
        let sel = scan_candidates(&base, Some(0));
        c.apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter);
        assert!(!base.join("Slack/Cache").exists());
        assert!(outside.join("keep.txt").exists());
        assert!(outside.join("dir/keep2.txt").exists());
    }

    #[test]
    fn apply_rejects_dir_replaced_by_symlink_after_scan() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let c = cleaner_for(&tmp, &Fake::default());
        let sel = scan_candidates(&base, Some(0));
        let outside = tmp.path().join("Documents");
        put(&outside, "keep.txt", 100);
        fs::remove_dir_all(base.join("Slack/Cache")).unwrap();
        std::os::unix::fs::symlink(&outside, base.join("Slack/Cache")).unwrap();
        let r = c.apply_selection(&sel, &HashSet::new(), &DeepSuppressReporter);
        assert!(outside.join("keep.txt").exists());
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
    }

    #[test]
    fn label_marks_running_apps() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        put(&base, "Slack/Cache/a.bin", 10_000);
        let sel = scan_candidates(&base, Some(0));
        assert!(format_candidate_label(&sel[0], true).contains("[running]"));
        assert!(!format_candidate_label(&sel[0], false).contains("[running]"));
        assert!(format_candidate_label(&sel[0], false).contains("Slack/Cache"));
    }

    #[test]
    fn app_support_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!cleaner_for(&tmp, &Fake::default()).auto_clean_allowed());
    }
```

**5-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find type AppSupportCleaner` などのコンパイルエラー。

**5-3. 実装する。** ファイル先頭の `use` 群を次に置き換え、`scan_candidates` の後ろ(`#[cfg(test)]` の前)に追記します。

```rust
use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
use crate::hint::apps::KNOWN_ENTRIES;
use crate::hint::{is_running, quit_app_and_wait, relaunch_app, BaseDir, HintEntry};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::io::IsTerminal;
use std::path::{Component, Path, PathBuf};
```

```rust
const NAME: &str = "app-support";

pub fn format_candidate_label(c: &Candidate, running: bool) -> String {
    format!(
        "{:<18} {:<44} {:>10}{}",
        c.target.hint_name,
        format!("{}/{}", c.target.root, c.sub),
        format_bytes(c.size),
        if running { "  [running]" } else { "" }
    )
}

enum AppState {
    Ready,
    Quit,
    Blocked(String),
}

pub struct AppSupportCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    min_app_bytes: Option<u64>,
}

impl AppSupportCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            min_app_bytes: None,
        }
    }

    #[cfg(test)]
    fn with_min_app_bytes(mut self, n: u64) -> Self {
        self.min_app_bytes = Some(n);
        self
    }

    fn base(&self) -> PathBuf {
        self.home.join("Library/Application Support")
    }

    fn empty_result() -> CleanResult {
        CleanResult {
            name: NAME,
            bytes_freed: 0,
            uses_trash: false,
            skipped: vec![],
            deleted_paths: vec![],
        }
    }

    fn running_apps(&self, cands: &[Candidate]) -> HashSet<&'static str> {
        let mut running = HashSet::new();
        for c in cands {
            if running.contains(c.target.hint_name) {
                continue;
            }
            let up = hint_entry(c.target)
                .and_then(|e| e.process_name)
                .map(|n| is_running(n, self.runner.as_ref()))
                .unwrap_or(false);
            if up {
                running.insert(c.target.hint_name);
            }
        }
        running
    }

    fn print_dry_run(&self, cands: &[Candidate]) {
        if cands.is_empty() {
            println!("[{NAME}] nothing to clean");
            return;
        }
        println!("[{NAME}] dry-run: {} cleanable cache directories", cands.len());
        for c in cands {
            println!(
                "  would remove: {}  ({}, {})",
                c.path.display(),
                c.target.hint_name,
                format_bytes(c.size)
            );
        }
    }

    fn interactive_select(
        &self,
        cands: &[Candidate],
        running: &HashSet<&'static str>,
    ) -> Result<Vec<Candidate>> {
        use dialoguer::MultiSelect;
        let items: Vec<String> = cands
            .iter()
            .map(|c| format_candidate_label(c, running.contains(c.target.hint_name)))
            .collect();
        let defaults = vec![false; items.len()];
        println!("\nCache directories under ~/Library/Application Support/ (none selected by default):\n");
        let picked = MultiSelect::new().items(&items).defaults(&defaults).interact()?;
        Ok(picked.into_iter().map(|i| cands[i].clone()).collect())
    }

    fn confirm_quits(
        &self,
        selected: &[Candidate],
        running: &HashSet<&'static str>,
    ) -> Result<HashSet<&'static str>> {
        use dialoguer::Confirm;
        let mut approved = HashSet::new();
        for c in selected {
            let name = c.target.hint_name;
            if !running.contains(name) || approved.contains(name) {
                continue;
            }
            let ok = Confirm::new()
                .with_prompt(format!(
                    "{name} is running. Quit it now? (its own save dialogs apply)"
                ))
                .default(false)
                .interact()?;
            if ok {
                approved.insert(name);
            }
        }
        Ok(approved)
    }

    fn prepare_app(
        &self,
        target: &'static AppSupportTarget,
        approved: &HashSet<&'static str>,
    ) -> AppState {
        let Some(entry) = hint_entry(target) else {
            return AppState::Blocked("no hint entry".to_string());
        };
        let up = entry
            .process_name
            .map(|n| is_running(n, self.runner.as_ref()))
            .unwrap_or(false);
        if !up {
            return AppState::Ready;
        }
        if !approved.contains(target.hint_name) {
            return AppState::Blocked(format!("{} is running", target.hint_name));
        }
        match quit_app_and_wait(entry, self.runner.as_ref()) {
            Ok(()) => AppState::Quit,
            Err(e) => AppState::Blocked(format!("{e}")),
        }
    }

    /// Deletes the selected candidates. The paths come from `resolve_allowed`,
    /// never from `Candidate::path`, so a forged or stale candidate cannot
    /// reach outside the allowlist.
    pub(crate) fn apply_selection(
        &self,
        selected: &[Candidate],
        quit_approved: &HashSet<&'static str>,
        reporter: &dyn ProgressReporter,
    ) -> CleanResult {
        let base = self.base();
        let mut states: Vec<(&'static str, AppState)> = Vec::new();
        let mut freed = 0u64;
        let mut skipped = Vec::new();
        let mut deleted = Vec::new();

        reporter.progress_init(NAME, selected.len());
        for (i, cand) in selected.iter().enumerate() {
            reporter.progress_tick(&cand.path, i + 1, cand.size);
            let name = cand.target.hint_name;
            let idx = match states.iter().position(|(n, _)| *n == name) {
                Some(i) => i,
                None => {
                    let st = self.prepare_app(cand.target, quit_approved);
                    states.push((name, st));
                    states.len() - 1
                }
            };
            if let AppState::Blocked(reason) = &states[idx].1 {
                skipped.push(SkippedEntry {
                    path: cand.path.clone(),
                    reason: reason.clone(),
                });
                continue;
            }
            let path = match resolve_allowed(&base, cand.target, cand.sub) {
                Ok(p) => p,
                Err(e) => {
                    skipped.push(SkippedEntry {
                        path: cand.path.clone(),
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            let _ = self
                .runner
                .run("chflags", &["-R", "nouchg", &path.to_string_lossy()]);
            match crate::trash::delete_path(&path) {
                Ok(()) => {
                    freed += cand.size;
                    println!(
                        "[{NAME}] removed: {} (freed {})",
                        path.display(),
                        format_bytes(cand.size)
                    );
                    deleted.push(path);
                }
                Err(e) => {
                    eprintln!("[{NAME}] error removing {}: {e}", path.display());
                    skipped.push(SkippedEntry {
                        path: cand.path.clone(),
                        reason: format!("{e}"),
                    });
                }
            }
        }
        reporter.progress_finish();

        for (name, state) in &states {
            if !matches!(state, AppState::Quit) {
                continue;
            }
            let entry = TARGETS
                .iter()
                .find(|t| t.hint_name == *name)
                .and_then(hint_entry);
            if let Some(entry) = entry {
                if let Err(e) = relaunch_app(entry, self.runner.as_ref()) {
                    eprintln!("[{NAME}] failed to relaunch {name}: {e}");
                }
            }
        }

        CleanResult {
            name: NAME,
            bytes_freed: freed,
            uses_trash: true,
            skipped,
            deleted_paths: deleted,
        }
    }
}

impl Cleaner for AppSupportCleaner {
    fn name(&self) -> &'static str {
        NAME
    }

    fn detect(&self) -> ScanResult {
        let base = self.base();
        if !base.exists() {
            return ScanResult::new(NAME, ScanStatus::NotFound);
        }
        let total: u64 = scan_candidates(&base, self.min_app_bytes)
            .iter()
            .map(|c| c.size)
            .sum();
        let mut r = ScanResult::new(
            NAME,
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(base.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let base = self.base();
        if !base.exists() {
            return Ok(Self::empty_result());
        }
        let cands = scan_candidates(&base, self.min_app_bytes);
        if dry_run {
            self.print_dry_run(&cands);
            return Ok(Self::empty_result());
        }
        if cands.is_empty() {
            println!("[{NAME}] nothing to clean");
            return Ok(Self::empty_result());
        }
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "[{NAME}] not a terminal - skipping. Run `sasurahime clean app-support` \
                 interactively to choose what to remove."
            );
            return Ok(Self::empty_result());
        }
        let running = self.running_apps(&cands);
        let selected = self.interactive_select(&cands, &running)?;
        if selected.is_empty() {
            println!("[{NAME}] nothing selected");
            return Ok(Self::empty_result());
        }
        let approved = self.confirm_quits(&selected, &running)?;
        let result = self.apply_selection(&selected, &approved, reporter);
        println!("[{NAME}] total freed: {}", format_bytes(result.bytes_freed));
        Ok(result)
    }

    // Deleting app caches may require quitting running apps, so it is never
    // done unattended; `--yes` skips this cleaner even on a terminal.
    fn auto_clean_allowed(&self) -> bool {
        false
    }
}
```

`auto_clean_allowed()` は 04 の Task S2 で `Cleaner` trait に追加済みのメソッドのオーバーライドです(共通規約 2.2)。Task 12 で `clean` を分割するときも、このメソッドは残します。

**5-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 37 件 passed、failed 0(`apply_quits_approved_running_app_then_deletes_and_relaunches` は終了待ちの 1 秒スリープを含みます)。この時点で `pub(crate) use cleaner::{quit_app_and_wait, relaunch_app};` と `crate::hint::is_running` が使われるので、hint 側の未使用警告は消えます。

**5-5. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "feat(app-support): AppSupportCleaner を追加(未選択既定の対話削除と実行中アプリ対応)"
```

### Task 6: main.rs への登録と E2E(第1段)

登録は `define_cleaners!` の標準エントリ 1 つだけです(6 章に全文があります)。特殊変種としては登録しません。

**6-1. 失敗するテストを書く。** `tests/app_support.rs` を新規作成します。

```rust
use assert_cmd::Command;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn put(home: &Path, rel: &str, bytes: usize) {
    let p = home.join("Library/Application Support").join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, vec![1u8; bytes]).unwrap();
}

const OVER_THRESHOLD: usize = 65 * 1024 * 1024;

#[test]
fn dry_run_lists_cache_dirs_and_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    put(tmp.path(), "Slack/Local Storage/leveldb/keep.bin", 4096);
    put(tmp.path(), "Slack/IndexedDB/keep.bin", 4096);

    let output = sasurahime(tmp.path())
        .args(["clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Slack/Cache"), "stdout:\n{stdout}");
    assert!(!stdout.contains("Local Storage"), "stdout:\n{stdout}");
    assert!(!stdout.contains("IndexedDB"), "stdout:\n{stdout}");
    let base = tmp.path().join("Library/Application Support/Slack");
    assert!(base.join("Cache/big.bin").exists());
    assert!(base.join("Local Storage/leveldb/keep.bin").exists());
    assert!(base.join("IndexedDB/keep.bin").exists());
}

#[test]
fn dry_run_without_application_support_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "app-support", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn non_interactive_clean_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);

    let output = sasurahime(tmp.path())
        .args(["clean", "app-support"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not a terminal"), "stderr:\n{stderr}");
    assert!(tmp
        .path()
        .join("Library/Application Support/Slack/Cache/big.bin")
        .exists());
}

#[test]
fn targets_lists_app_support_exactly_once() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let n = stdout
        .lines()
        .filter(|l| l.split_whitespace().next() == Some("app-support"))
        .count();
    assert_eq!(n, 1, "stdout:\n{stdout}");
}

#[test]
fn scan_includes_app_support() {
    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    let output = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("app-support"), "stdout:\n{stdout}");
}
```

**6-2. 実行して失敗を確認。**

```bash
cargo test --test app_support 2>&1 | tail -20
```

期待: 5 件すべてが失敗(3 件は `error: unrecognized subcommand 'app-support'`、`targets` と `scan` のテストは `app-support` が出ないことによる assertion の失敗)。

続けて `src/main.rs` の `mod tests` の末尾(最後のテスト関数の後ろ、`mod tests` を閉じる `}` の直前)に次を追加します。04 の S1 で追加された `names_of` を使います。

```rust
    #[test]
    fn app_support_is_registered_but_not_auto_cleaned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        assert!(names_of(&cleaners).contains(&"app-support"));
        let c = cleaners.iter().find(|c| c.name() == "app-support").unwrap();
        assert!(!c.auto_clean_allowed());
    }
```

```bash
cargo test --bin sasurahime app_support_is_registered 2>&1 | tail -10
```

期待: `assertion failed` で 1 件失敗(`all_cleaners()` にまだ無いため)。

**6-3. 実装する。** 6 章の手順を適用します。

**6-4. 実行して成功を確認。**

```bash
cargo test --test app_support 2>&1 | tail -20
cargo test --bin sasurahime app_support_is_registered 2>&1 | tail -10
cargo test 2>&1 | tail -5
cargo clippy -- -D warnings
cargo fmt --check
```

期待: `app_support` の E2E 5 件 passed、`app_support_is_registered_but_not_auto_cleaned` が ok、全体 `test result: ok.`(04 の `only_legacy_targets_are_auto_clean_allowed`、`standard_cleaners_follow_the_macro_table_order`、`tests/coverage.rs` を含む)。clippy は警告 0、fmt は出力なし。

**6-5. コミット。**

```bash
git add src/main.rs tests/app_support.rs
git commit -m "feat(app-support): clean app-support を main.rs に登録しE2Eを追加"
```

### Task 7: ドキュメントと第1段の最終確認

**7-1. テスト。** ドキュメントのみの変更なので新規テストはありません。代わりに、記載内容が実装と一致することを次のコマンドで確認します。

```bash
cargo run -q -- targets 2>/dev/null | grep app-support
```

期待: `app-support` を含む行が1行出る。

**7-2. 実装する。** `docs/SUPPORTED.md` を 6 か所、`docs/HOWTO-USE.md` を 2 か所編集します。番号と件数は共通規約 5.2 の表のとおり固定です(05 完了後: 件数 51、新節 `## 51.`、stats は `## 52.`)。着手時点では 03 の結果で件数 50、`## 50. library-caches`、`## 51. stats` になっています。「最大番号 + 1」で決めないでください(stats 節が最大番号です)。

(a) 英語の Quick Reference 表の `| \`trash\` |` で始まる行の直前(03 が追加した `library-caches` 行の次)に追加:

```markdown
| `app-support` | Cache dirs of known apps in `~/Library/Application Support/` - interactive only, never in `--yes` |
```

(b) 日本語のクイックリファレンス表の `| \`trash\` |` で始まる行の直前(03 が追加した `library-caches` 行の次)に追加:

```markdown
| `app-support` | `~/Library/Application Support/` 内の既知アプリのキャッシュ、対話的のみ、`--yes` では非実行 |
```

(c) 英語節の `## 51. \`sasurahime stats\`` の見出し行の直前(その前の `---` 行の後ろ)に次の新節を挿入し、stats の見出しを `## 52. \`sasurahime stats\`` に変更します。新節の末尾の `---` が stats 節との区切りになります:

```markdown
## 51. `sasurahime clean app-support`

**Category:** Sprint 6

**What it removes:** Cache subdirectories of known apps (Slack, Claude desktop,
Obsidian, VS Code, Google Chrome) under `~/Library/Application Support/`.
Only paths on a fixed allowlist can be removed (`Cache`, `Code Cache`,
`GPUCache`, `Service Worker/CacheStorage`, `logs`, and similar). Settings,
logins, IndexedDB, Local Storage and workspaces are never touched.

**How detect works:** Sums the allowlisted directories per app. An app is
reported only when its total exceeds 64 MiB. The leftover list described below
is not included in the total.

**How clean works:**
1. `--dry-run` prints the candidate directories and exits.
2. Otherwise a `dialoguer::MultiSelect` opens with nothing selected.
3. For a running app, a confirmation asks whether to quit it. Without
   approval the app's directories are skipped. After deletion, an app that
   sasurahime quit is relaunched if its hint entry configures a relaunch app.
4. Selected directories are moved to the Trash (`--permanent` deletes).

**Leftovers (display only):** Folders directly under `Application Support`
that match no installed `.app` by name or bundle ID tail are listed. Nothing
is deleted; selecting an entry opens it in Finder.

**Safety:**
- Allowlist only; `..`, absolute paths and symbolic links are rejected.
- Never runs under `--yes`; a non-TTY invocation does nothing.
- `--dry-run` has no side effects.

---
```

(d) 日本語節の `## 51. \`sasurahime stats\`` の見出し行の直前(その前の `---` 行の後ろ)に次の新節を挿入し、stats の見出しを `## 52. \`sasurahime stats\`` に変更します:

```markdown
## 51. `sasurahime clean app-support`

**カテゴリ:** Sprint 6

**削除対象:** `~/Library/Application Support/` 下の既知アプリ(Slack、Claude
デスクトップ、Obsidian、VS Code、Google Chrome)のキャッシュ用サブディレクトリ。
固定の許可リスト(`Cache`、`Code Cache`、`GPUCache`、`Service Worker/CacheStorage`、
`logs` など)にあるパスだけが削除できます。設定、ログイン情報、IndexedDB、
Local Storage、ワークスペースには一切触れません。

**detect の動作:** アプリごとに許可リストのディレクトリ合計を測ります。合計が
64 MiB を超えたアプリだけを対象にします。後述の残骸一覧は合計に含めません。

**clean の動作:**
1. `--dry-run` は候補ディレクトリを表示して終了します。
2. それ以外は `dialoguer::MultiSelect` を開きます。既定は全部未選択です。
3. 実行中のアプリは、終了してよいか確認します。承認がなければそのアプリの
   ディレクトリはスキップします。sasurahime が終了させたアプリは、hint 機能で
   再起動が設定されているものに限り、削除後に再起動します。
4. 選択したディレクトリはゴミ箱へ移動します(`--permanent` では完全削除)。

**残骸の表示(表示のみ):** `Application Support` 直下のフォルダのうち、インストール済み
`.app` の名前にも bundle ID の末尾要素にも一致しないものを一覧にします。削除は
しません。選択すると Finder で開きます。

**安全性:**
- 許可リストのみ。`..`、絶対パス、シンボリックリンクは拒否します。
- `--yes` では決して実行しません。非 TTY 呼び出しは何もしません。
- `--dry-run` は副作用ゼロです。

---
```

(e) 英語節の件数 `sasurahime provides **50 clean targets**.` を `sasurahime provides **51 clean targets**.` に変更します。

(f) 日本語節の件数 `sasurahime は **50 のクリーンターゲット** を提供しています。` を `sasurahime は **51 のクリーンターゲット** を提供しています。` に変更します。

(g) `docs/HOWTO-USE.md` の英語の対象表(99 行付近)の `| \`apfs-snapshot\`` で始まる行の次に追加:

```markdown
| `app-support`   | Cache dirs of known apps in `~/Library/Application Support/` (interactive only) |
```

(h) `docs/HOWTO-USE.md` の日本語の対象表(671 行付近)の `| \`apfs-snapshot\`` で始まる行の次に追加:

```markdown
| `app-support`   | `~/Library/Application Support/` 内の既知アプリのキャッシュ(対話的のみ) |
```

確認:

```bash
grep -n "^## 51\. \|^## 52\. " docs/SUPPORTED.md     # app-support 2 行、stats 2 行
grep -n "51 clean targets\|51 のクリーンターゲット" docs/SUPPORTED.md   # 2 行
cargo run -q -- targets | wc -l                     # 51
```

**7-3. 最終確認(第1段の完了ゲート)。**

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test 2>&1 | tail -5
```

期待: fmt と clippy が無出力で通り、`test result: ok.`。この時点で第2段の記述(残骸)はドキュメントに先行していますが、第2段は同じセッションで続けて実装します。

**7-4. コミット。**

```bash
git add docs/SUPPORTED.md docs/HOWTO-USE.md
git commit -m "docs(app-support): SUPPORTED.md と HOWTO-USE.md に app-support を追記"
```

### Task 8: 第2段の前提確認(orphan-containers の関数)

第2段は 02 が実装した `src/cleaners/orphan_containers.rs` の `collect_installed_ids` を再利用します。実装順(04 -> 02 -> 03 -> 05)により存在しているはずですが、実装前に確認します。

**8-1. 確認コマンド。**

```bash
grep -n "pub fn collect_installed_ids" src/cleaners/orphan_containers.rs
grep -n "pub mod orphan_containers" src/cleaners/mod.rs
```

期待: それぞれ1行ずつ出力され、1 行目が `pub fn collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String> {` であること。出ない場合や引数・戻り値の型が違う場合は、推測で合わせずに作業を止めて報告します。

**8-2. テストを書く。** 存在とシグネチャをテストとして固定します。`mod tests` の末尾に追加します。

```rust
    #[test]
    fn orphan_containers_bundle_id_collector_is_available() {
        let ids: HashSet<String> =
            crate::cleaners::orphan_containers::collect_installed_ids(&[], &Fake::default());
        assert!(ids.is_empty());
    }
```

**8-3. 実行。**

```bash
cargo test --bin sasurahime orphan_containers_bundle_id_collector 2>&1 | tail -10
```

期待: passed(02 が完了済みなので最初から通ります。このタスクは赤の確認をしません)。コンパイルエラーになった場合は 02 の実装と食い違っているので、止めて報告します。

**8-4. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "test(app-support): orphan-containers の bundle ID 収集関数の存在を固定"
```

### Task 9: 名前の正規化・除外・照合(第2段、純関数)

照合ルール(確定):

1. 比較は正規化名で行います。`normalize_name` は英数字(Unicode の alphanumeric)だけを残して小文字にします。`"Google Chrome"` は `"googlechrome"`、`"zoom.us"` は `"zoomus"` です。
2. フォルダ名の正規化名が、インストール済みアプリの名前集合(.app のファイル名の stem、`CFBundleName`、`CFBundleDisplayName`)に含まれれば一致です。
3. フォルダ名(小文字)がインストール済み bundle ID と完全一致すれば一致です。
4. フォルダ名の正規化名が、インストール済み bundle ID の末尾要素(正規化済み、3文字以上)に含まれれば一致です。例: フォルダ `slackmacgap` と `com.tinyspeck.slackmacgap`。
5. フォルダ名に `.` が2つ以上あれば(bundle ID 形式)、最後の要素を正規化して 2 と 4 も試します。例: フォルダ `com.example.Foo` と `Foo.app`。
6. 正規化すると空になる名前は「一致」扱いにします(判断できないものを残骸と言わない)。
7. 一致は存在判定です。同名の .app が複数あっても、1つでも一致すればインストール済みで、残骸にしません。

除外(残骸の候補にしない):

- 先頭が `.` のもの。
- 完全一致(大文字小文字無視): `AddressBook`、`CloudDocs`、`MobileSync`、`CallHistoryDB`、`CallHistoryTransactions`、`CrashReporter`、`Dock`、`FileProvider`、`Knowledge`、`SyncServices`、`iCloud`、`DifferentialPrivacy`、`AskPermission`、`Animoji`、`Siri`、`App Store`、`DiskImages`、`Spotlight`、`Quick Look`、`FaceTime`、`Notes`、`networkserviceproxy`、`icdd`、`Homebrew`、`pypoetry`、`rustup`。
- 前方一致: `com.apple.`、`Apple`。
- 複数アプリが共有するベンダー親フォルダ: `Google`、`Microsoft`、`Adobe`、`JetBrains`、`BraveSoftware`、`Mozilla`、`Autodesk`、`Oracle`、`Blackmagic Design`。名前では判定できないためです。
- 第1段の許可リストの root の先頭要素(`Slack`、`Claude`、`obsidian`、`Code`、`Google`)。既知アプリは別扱いで、`Code` は .app 名 `Visual Studio Code` と照合できないためです。

**9-1. 失敗するテストを書く。**

```rust
    fn installed(names: &[&str], ids: &[&str]) -> InstalledApps {
        InstalledApps::from_parts(
            names.iter().map(|s| s.to_string()),
            ids.iter().map(|s| s.to_string()),
        )
    }

    #[test]
    fn normalize_name_cases() {
        assert_eq!(normalize_name("Google Chrome"), "googlechrome");
        assert_eq!(normalize_name("zoom.us"), "zoomus");
        assert_eq!(normalize_name("1Password 7"), "1password7");
        assert_eq!(normalize_name(" - . "), "");
    }

    #[test]
    fn excluded_names() {
        for n in [
            ".DS_Store",
            "AddressBook",
            "addressbook",
            "CloudDocs",
            "MobileSync",
            "com.apple.sharedfilelist",
            "AppleIntelligence",
            "Google",
            "Microsoft",
            "JetBrains",
            "Slack",
            "obsidian",
            "Code",
            "Claude",
        ] {
            assert!(is_excluded_dir_name(n), "{n}");
        }
        for n in ["Spotify", "Zoom", "Figma", "com.foo.bar"] {
            assert!(!is_excluded_dir_name(n), "{n}");
        }
    }

    #[test]
    fn match_by_app_name() {
        let i = installed(&["Spotify", "Google Chrome"], &[]);
        assert!(matches_installed("Spotify", &i));
        assert!(matches_installed("spotify", &i));
        assert!(matches_installed("Google-Chrome", &i));
        assert!(!matches_installed("Figma", &i));
    }

    #[test]
    fn match_by_full_bundle_id_case_insensitive() {
        let i = installed(&[], &["com.foo.Bar"]);
        assert!(matches_installed("com.foo.bar", &i));
        assert!(matches_installed("COM.FOO.BAR", &i));
    }

    #[test]
    fn match_by_bundle_id_tail() {
        let i = installed(&[], &["com.tinyspeck.slackmacgap"]);
        assert!(matches_installed("slackmacgap", &i));
        assert!(!matches_installed("slack", &i));
    }

    #[test]
    fn match_bundle_id_shaped_folder_by_its_last_component() {
        let i = installed(&["Foo"], &[]);
        assert!(matches_installed("com.example.Foo", &i));
        let j = installed(&[], &["org.other.widget"]);
        assert!(matches_installed("com.example.widget", &j));
    }

    #[test]
    fn unmatchable_empty_normalized_name_is_treated_as_installed() {
        let i = installed(&[], &[]);
        assert!(matches_installed("---", &i));
    }

    #[test]
    fn short_bundle_tail_is_not_used() {
        let i = installed(&[], &["com.foo.ab"]);
        assert!(!matches_installed("ab", &i));
    }

    #[test]
    fn duplicate_app_names_still_match_once_or_more() {
        let i = installed(&["Foo", "Foo"], &["com.a.foo", "com.b.foo"]);
        assert!(matches_installed("Foo", &i));
    }
```

**9-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find type InstalledApps` などのコンパイルエラー。

**9-3. 実装する。** `scan_candidates` の後ろに追記します(`impl AppSupportCleaner` の前でも後でもかまいません)。

```rust
// ── Stage 2: leftovers of uninstalled apps (display only) ──

const EXCLUDED_EXACT: &[&str] = &[
    "AddressBook",
    "CloudDocs",
    "MobileSync",
    "CallHistoryDB",
    "CallHistoryTransactions",
    "CrashReporter",
    "Dock",
    "FileProvider",
    "Knowledge",
    "SyncServices",
    "iCloud",
    "DifferentialPrivacy",
    "AskPermission",
    "Animoji",
    "Siri",
    "App Store",
    "DiskImages",
    "Spotlight",
    "Quick Look",
    "FaceTime",
    "Notes",
    "networkserviceproxy",
    "icdd",
    "Homebrew",
    "pypoetry",
    "rustup",
];

const EXCLUDED_PREFIXES: &[&str] = &["com.apple.", "Apple"];

/// Folders shared by several apps; the name alone cannot tell if one is installed.
const VENDOR_PARENT_DIRS: &[&str] = &[
    "Google",
    "Microsoft",
    "Adobe",
    "JetBrains",
    "BraveSoftware",
    "Mozilla",
    "Autodesk",
    "Oracle",
    "Blackmagic Design",
];

pub fn normalize_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

pub fn is_excluded_dir_name(name: &str) -> bool {
    if name.starts_with('.') {
        return true;
    }
    let lower = name.to_lowercase();
    if EXCLUDED_EXACT
        .iter()
        .chain(VENDOR_PARENT_DIRS.iter())
        .any(|e| e.to_lowercase() == lower)
    {
        return true;
    }
    if EXCLUDED_PREFIXES
        .iter()
        .any(|p| lower.starts_with(&p.to_lowercase()))
    {
        return true;
    }
    // Known apps are handled by the allowlist, not by name matching.
    TARGETS.iter().any(|t| {
        t.root
            .split('/')
            .next()
            .is_some_and(|first| first.to_lowercase() == lower)
    })
}

pub struct InstalledApps {
    names: HashSet<String>,
    bundle_ids: HashSet<String>,
    bundle_tails: HashSet<String>,
}

impl InstalledApps {
    pub fn from_parts(
        names: impl IntoIterator<Item = String>,
        bundle_ids: impl IntoIterator<Item = String>,
    ) -> Self {
        let names = names
            .into_iter()
            .map(|n| normalize_name(&n))
            .filter(|n| !n.is_empty())
            .collect();
        let bundle_ids: HashSet<String> =
            bundle_ids.into_iter().map(|b| b.to_lowercase()).collect();
        let bundle_tails = bundle_ids
            .iter()
            .filter_map(|b| b.rsplit('.').next())
            .map(normalize_name)
            .filter(|t| t.len() >= 3)
            .collect();
        Self {
            names,
            bundle_ids,
            bundle_tails,
        }
    }
}

pub fn matches_installed(dir_name: &str, installed: &InstalledApps) -> bool {
    let n = normalize_name(dir_name);
    if n.is_empty() {
        return true;
    }
    if installed.names.contains(&n)
        || installed.bundle_ids.contains(&dir_name.to_lowercase())
        || (n.len() >= 3 && installed.bundle_tails.contains(&n))
    {
        return true;
    }
    if dir_name.matches('.').count() >= 2 {
        if let Some(last) = dir_name.rsplit('.').next() {
            let l = normalize_name(last);
            if !l.is_empty()
                && (installed.names.contains(&l)
                    || (l.len() >= 3 && installed.bundle_tails.contains(&l)))
            {
                return true;
            }
        }
    }
    false
}
```

**9-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 全件 passed(Task 9 で 9 件増える)。

**9-5. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "feat(app-support): アンインストール済みアプリ判定の名前照合と除外ルールを追加"
```

### Task 10: インストール済みアプリの収集(第2段)

**10-1. 失敗するテストを書く。**

```rust
    struct PlistRunner(std::collections::HashMap<String, String>);

    impl CommandRunner for PlistRunner {
        fn run(&self, program: &str, args: &[&str]) -> anyhow::Result<Output> {
            if program != "plutil" {
                anyhow::bail!("unexpected {program}");
            }
            let path = args.last().copied().unwrap_or("");
            match self.0.iter().find(|(k, _)| path.contains(k.as_str())) {
                Some((_, json)) => Ok(Output {
                    status: std::process::ExitStatus::from_raw(0),
                    stdout: json.clone().into_bytes(),
                    stderr: vec![],
                }),
                None => Ok(out(256)),
            }
        }
        fn exists(&self, _: &str) -> bool {
            true
        }
    }

    #[test]
    fn parse_plist_names_reads_both_keys() {
        let j = r#"{"CFBundleName":"Code","CFBundleDisplayName":"Visual Studio Code","CFBundleIdentifier":"com.microsoft.VSCode"}"#;
        assert_eq!(parse_plist_names(j), vec!["Code", "Visual Studio Code"]);
    }

    #[test]
    fn parse_plist_names_tolerates_missing_keys_and_garbage() {
        assert_eq!(parse_plist_names(r#"{"CFBundleName":"A"}"#), vec!["A"]);
        assert!(parse_plist_names("{}").is_empty());
        assert!(parse_plist_names("not json").is_empty());
        assert!(parse_plist_names(r#"{"CFBundleName":5}"#).is_empty());
    }

    #[test]
    fn list_app_bundles_finds_top_level_and_one_level_nested() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Foo Bar.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("Setapp/Baz.app")).unwrap();
        fs::create_dir_all(apps.join("Setapp/deep/Qux.app")).unwrap();
        fs::create_dir_all(apps.join("NotAnApp")).unwrap();
        let mut found: Vec<String> = list_app_bundles(&[apps.clone(), tmp.path().join("missing")])
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        found.sort();
        assert_eq!(found, vec!["Baz.app", "Foo Bar.app"]);
    }

    #[test]
    fn collect_installed_names_uses_file_stem_and_plist_names() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Visual Studio Code.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("Plain.app/Contents")).unwrap();
        let runner = PlistRunner(
            [(
                "Visual Studio Code.app".to_string(),
                r#"{"CFBundleName":"Code"}"#.to_string(),
            )]
            .into_iter()
            .collect(),
        );
        let mut names = collect_installed_names(&[apps], &runner);
        names.sort();
        assert_eq!(names, vec!["Code", "Plain", "Visual Studio Code"]);
    }
```

**10-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find function parse_plist_names` などのコンパイルエラー。

**10-3. 実装する。** Task 9 のコードの後ろに追記します。`use` に `use std::time::SystemTime;` は Task 11 で足します。

```rust
pub fn parse_plist_names(json: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        return vec![];
    };
    ["CFBundleName", "CFBundleDisplayName"]
        .iter()
        .filter_map(|k| v.get(*k).and_then(|x| x.as_str()))
        .map(|s| s.to_string())
        .collect()
}

fn is_app_bundle(p: &Path) -> bool {
    p.extension().is_some_and(|e| e == "app")
}

/// `.app` bundles directly under each root, plus one level of subfolders
/// (for example `/Applications/Setapp/*.app`). Symlinked folders are not followed.
fn list_app_bundles(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for root in roots {
        let Ok(rd) = fs::read_dir(root) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if is_app_bundle(&path) {
                found.push(path);
                continue;
            }
            let is_real_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if !is_real_dir {
                continue;
            }
            if let Ok(sub) = fs::read_dir(&path) {
                for s in sub.flatten() {
                    if is_app_bundle(&s.path()) {
                        found.push(s.path());
                    }
                }
            }
        }
    }
    found
}

fn collect_installed_names(roots: &[PathBuf], runner: &dyn CommandRunner) -> Vec<String> {
    let mut names = Vec::new();
    for app in list_app_bundles(roots) {
        if let Some(stem) = app.file_stem().and_then(|s| s.to_str()) {
            names.push(stem.to_string());
        }
        let plist = app.join("Contents/Info.plist");
        if let Ok(o) = runner.run(
            "plutil",
            &["-convert", "json", "-o", "-", &plist.to_string_lossy()],
        ) {
            if o.status.success() {
                names.extend(parse_plist_names(&String::from_utf8_lossy(&o.stdout)));
            }
        }
    }
    names
}

/// Names come from this module; bundle IDs come from the orphan-containers
/// collector so both features agree on what is installed.
pub fn collect_installed_apps(roots: &[PathBuf], runner: &dyn CommandRunner) -> InstalledApps {
    let names = collect_installed_names(roots, runner);
    let ids = crate::cleaners::orphan_containers::collect_installed_ids(roots, runner);
    InstalledApps::from_parts(names, ids)
}
```

**10-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 全件 passed。

**10-5. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "feat(app-support): インストール済みアプリ名と bundle ID の収集を追加"
```

### Task 11: 残骸の検出 find_leftovers(第2段)

追加ルール(確定):

- 直下のディレクトリだけが対象です。ファイルとシンボリックリンクは対象外です(`DirEntry::file_type` はリンクをたどりません)。
- サイズ(`dir_size`)が 1 MiB 未満のものは出しません(ノイズ削減)。
- フォルダ自身と直下の子の最終更新のうち最新が 30 日以内のものは出しません。`/Volumes` や Setapp など、走査しない場所にあるアプリを今も使っている可能性が高いためです。未来の日時も「最近」として除外します。
- 並びはサイズの降順です。

**11-1. 失敗するテストを書く。**

```rust
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    fn age(path: &Path, days: u64) {
        let t = SystemTime::now() - Duration::from_secs(days * 86_400);
        set_file_mtime(path, FileTime::from_system_time(t)).unwrap();
    }

    /// Creates `<base>/<name>/data.bin` of `bytes` bytes, all aged `days` days.
    fn make_folder(base: &Path, name: &str, bytes: usize, days: u64) -> PathBuf {
        let d = base.join(name);
        fs::create_dir_all(&d).unwrap();
        let f = d.join("data.bin");
        fs::write(&f, vec![1u8; bytes]).unwrap();
        age(&f, days);
        age(&d, days);
        d
    }

    const MIB: usize = 1024 * 1024;

    #[test]
    fn leftovers_lists_uninstalled_old_folder() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let d = make_folder(&base, "GoneApp", 2 * MIB, 200);
        let l = find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30);
        assert_eq!(l.len(), 1);
        assert_eq!(l[0].path, d);
        assert!(l[0].size >= 2 * MIB as u64);
    }

    #[test]
    fn leftovers_skips_installed_by_name_and_by_bundle_tail() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "Spotify", 2 * MIB, 200);
        make_folder(&base, "slackmacgap", 2 * MIB, 200);
        let i = installed(&["Spotify"], &["com.tinyspeck.slackmacgap"]);
        assert!(find_leftovers(&base, &i, SystemTime::now(), 1 << 20, 30).is_empty());
    }

    #[test]
    fn leftovers_skips_excluded_names() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        for n in ["AddressBook", "CloudDocs", "MobileSync", "com.apple.foo", "Google", "Slack", "Code", ".hidden"] {
            make_folder(&base, n, 2 * MIB, 200);
        }
        assert!(find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30).is_empty());
    }

    #[test]
    fn leftovers_skips_small_and_recent_and_future() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "Small", 100, 200);
        make_folder(&base, "Recent", 2 * MIB, 3);
        let fut = make_folder(&base, "Future", 2 * MIB, 200);
        let t = SystemTime::now() + Duration::from_secs(86_400);
        set_file_mtime(&fut, FileTime::from_system_time(t)).unwrap();
        assert!(find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30).is_empty());
    }

    #[test]
    fn leftovers_skips_files_and_symlinks() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let real = make_folder(&tmp.path().join("elsewhere"), "Real", 2 * MIB, 200);
        std::os::unix::fs::symlink(&real, base.join("LinkedApp")).unwrap();
        fs::write(base.join("loose.bin"), vec![1u8; 2 * MIB]).unwrap();
        assert!(find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30).is_empty());
    }

    #[test]
    fn leftovers_sorted_by_size_descending_and_missing_base_is_empty() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "Small", 2 * MIB, 200);
        make_folder(&base, "Big", 4 * MIB, 200);
        let l = find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30);
        assert_eq!(l.len(), 2);
        assert!(l[0].size >= l[1].size);
        assert!(l[0].path.ends_with("Big"));
        assert!(find_leftovers(&tmp.path().join("nope"), &installed(&[], &[]), SystemTime::now(), 1, 1).is_empty());
    }

    #[test]
    fn leftovers_never_modify_anything() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let d = make_folder(&base, "GoneApp", 2 * MIB, 200);
        find_leftovers(&base, &installed(&[], &[]), SystemTime::now(), 1 << 20, 30);
        assert!(d.join("data.bin").exists());
    }
```

**11-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `cannot find function find_leftovers` のコンパイルエラー。

**11-3. 実装する。** `use` に `use std::time::SystemTime;` を足し、Task 10 のコードの後ろに追記します。

```rust
pub const MIN_LEFTOVER_BYTES: u64 = 1024 * 1024;
pub const RECENT_DAYS: u64 = 30;

#[derive(Debug, Clone)]
pub struct Leftover {
    pub path: PathBuf,
    pub size: u64,
}

fn newest_mtime(path: &Path) -> Option<SystemTime> {
    let mut newest = fs::symlink_metadata(path).ok()?.modified().ok()?;
    for e in fs::read_dir(path).ok()?.flatten() {
        if let Ok(m) = fs::symlink_metadata(e.path()).and_then(|m| m.modified()) {
            newest = newest.max(m);
        }
    }
    Some(newest)
}

pub fn find_leftovers(
    base: &Path,
    installed: &InstalledApps,
    now: SystemTime,
    min_bytes: u64,
    recent_days: u64,
) -> Vec<Leftover> {
    let Ok(rd) = fs::read_dir(base) else {
        return vec![];
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if is_excluded_dir_name(&name) {
            continue;
        }
        if !e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if matches_installed(&name, installed) {
            continue;
        }
        let path = e.path();
        let size = dir_size(&path);
        if size < min_bytes {
            continue;
        }
        let Some(newest) = newest_mtime(&path) else {
            continue;
        };
        match now.duration_since(newest) {
            Ok(age) if age.as_secs() / 86_400 >= recent_days => {}
            _ => continue,
        }
        out.push(Leftover { path, size });
    }
    out.sort_by_key(|l| std::cmp::Reverse(l.size));
    out
}
```

**11-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 全件 passed。

**11-5. コミット。**

```bash
git add src/cleaners/app_support.rs
git commit -m "feat(app-support): アンインストール済みアプリの残骸検出を追加(表示専用)"
```

### Task 12: clean への組み込みと Finder 表示(第2段)

**12-1. 失敗するテストを書く。**

```rust
    #[test]
    fn leftover_line_shows_path_and_size() {
        let l = Leftover {
            path: PathBuf::from("/x/Application Support/GoneApp"),
            size: 2 * 1024 * 1024,
        };
        let s = format_leftover_line(&l);
        assert!(s.contains("/x/Application Support/GoneApp"));
        assert!(s.contains("2.0 MB"));
    }

    #[test]
    fn cleaner_leftovers_use_injected_app_roots() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "GoneApp", 2 * MIB, 200);
        make_folder(&base, "Foo", 2 * MIB, 200);
        let apps = tmp.path().join("Apps");
        fs::create_dir_all(apps.join("Foo.app")).unwrap();
        let c = cleaner_for(&tmp, &Fake::default()).with_app_roots(vec![apps]);
        let names: Vec<String> = c
            .leftovers()
            .iter()
            .map(|l| l.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["GoneApp"]);
    }

    #[test]
    fn detect_ignores_leftovers() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        make_folder(&base, "GoneApp", 2 * MIB, 200);
        let c = cleaner_for(&tmp, &Fake::default());
        assert!(matches!(c.detect().status, ScanStatus::Clean));
    }

    #[test]
    fn dry_run_with_leftovers_runs_no_destructive_commands() {
        let tmp = TempDir::new().unwrap();
        let base = base_of(&tmp);
        let d = make_folder(&base, "GoneApp", 2 * MIB, 200);
        let fake = Fake::default();
        let apps = tmp.path().join("Apps");
        fs::create_dir_all(&apps).unwrap();
        let c = cleaner_for(&tmp, &fake).with_app_roots(vec![apps]);
        let r = c.clean(true, &DeepSuppressReporter).unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert!(d.join("data.bin").exists());
        assert!(fake
            .calls()
            .iter()
            .all(|c| c.starts_with("plutil") || c.starts_with("pgrep")));
    }
```

(最後のテストでは Fake が `plutil` を `unexpected command` で Err にしますが、`collect_installed_names` はエラーを無視するので問題ありません。`Fake` に `"plutil"` の分岐を足す必要はありません。)

**12-2. 実行して失敗を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: `no method named with_app_roots` / `leftovers` / `cannot find function format_leftover_line` のコンパイルエラー。

**12-3. 実装する。**

`src/explorer.rs` は変更しません(Finder 起動は runner 経由の `open` で行います)。

(a) `src/cleaners/app_support.rs` の `mod tests` 内、Task 5 で書いた `cleaner_for` ヘルパーを次に置き換えます。単体テストが実機の `/Applications` を読まないようにするためです(共通規約 11.5)。これをしないと、Task 5 の `dry_run_touches_nothing_and_runs_no_commands` が実機の `.app` に対する `plutil` 呼び出しを記録して失敗します。

```rust
    fn cleaner_for(tmp: &TempDir, fake: &Fake) -> AppSupportCleaner {
        AppSupportCleaner::new(tmp.path(), Box::new(fake.clone()))
            .with_min_app_bytes(0)
            .with_app_roots(vec![])
    }
```

(b) `src/cleaners/app_support.rs`:

構造体にフィールドを足し、`new` を更新します。

```rust
pub struct AppSupportCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    min_app_bytes: Option<u64>,
    app_roots: Vec<PathBuf>,
}
```

`new` の本体を次にします。

```rust
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            min_app_bytes: None,
            app_roots: vec![
                PathBuf::from("/Applications"),
                PathBuf::from("/Applications/Utilities"),
                PathBuf::from("/System/Applications"),
                PathBuf::from("/System/Applications/Utilities"),
                home.join("Applications"),
            ],
        }
    }

    #[cfg(test)]
    fn with_app_roots(mut self, roots: Vec<PathBuf>) -> Self {
        self.app_roots = roots;
        self
    }

    fn leftovers(&self) -> Vec<Leftover> {
        let installed = collect_installed_apps(&self.app_roots, self.runner.as_ref());
        find_leftovers(
            &self.base(),
            &installed,
            SystemTime::now(),
            MIN_LEFTOVER_BYTES,
            RECENT_DAYS,
        )
    }

    /// Display only. Selecting an entry opens it in Finder; nothing is deleted.
    fn show_leftovers(&self, dry_run: bool) -> Result<()> {
        use dialoguer::MultiSelect;
        if !self.base().exists() {
            return Ok(());
        }
        if !dry_run && !std::io::stdin().is_terminal() {
            return Ok(());
        }
        let left = self.leftovers();
        if left.is_empty() {
            return Ok(());
        }
        println!("[{NAME}] possible leftovers of uninstalled apps (display only, nothing is deleted):");
        for l in &left {
            println!("{}", format_leftover_line(l));
        }
        if dry_run {
            return Ok(());
        }
        let items: Vec<String> = left.iter().map(format_leftover_line).collect();
        let defaults = vec![false; items.len()];
        let picked = MultiSelect::new()
            .with_prompt("Select leftovers to reveal in Finder (space to toggle, enter to confirm)")
            .items(&items)
            .defaults(&defaults)
            .interact()?;
        for i in picked {
            let path = left[i].path.to_string_lossy().to_string();
            match self.runner.run("open", &[path.as_str()]) {
                Ok(o) if o.status.success() => {}
                Ok(o) => eprintln!(
                    "[{NAME}] open returned {:?} for {path}",
                    o.status.code()
                ),
                Err(e) => eprintln!("[{NAME}] error opening Finder: {e}"),
            }
        }
        Ok(())
    }
```

`format_leftover_line` を `format_candidate_label` の近くに追加します。

```rust
pub fn format_leftover_line(l: &Leftover) -> String {
    format!("  {}  {}", l.path.display(), format_bytes(l.size))
}
```

`impl Cleaner for AppSupportCleaner` の `clean` を次の2つに分けます。既存の `fn clean(...)` の本体を、名前だけ `clean_cache_dirs` に変えて `impl AppSupportCleaner` に移し(中身は変更しません)、`impl Cleaner` 側は次にします。

```rust
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        let result = self.clean_cache_dirs(dry_run, reporter)?;
        self.show_leftovers(dry_run)?;
        Ok(result)
    }
```

移した関数の宣言は次です。

```rust
    fn clean_cache_dirs(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
    ) -> Result<CleanResult> {
        // (旧 clean の本体をそのまま。`Self::empty_result()` の呼び出しも変更なし)
    }
```

**12-4. 実行して成功を確認。**

```bash
cargo test --bin sasurahime app_support 2>&1 | tail -20
```

期待: 全件 passed。

**12-5. E2E を追加する。** `tests/app_support.rs` に追記します(失敗確認は 12-4 の後に実施: 追記前に `cargo test --test app_support` が通っていることを確認し、追記後に通ることを確認)。

```rust
#[test]
fn dry_run_shows_leftovers_without_deleting() {
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "ZzzNoSuchApp12345/data.bin", 2 * 1024 * 1024);
    let dir = tmp.path().join("Library/Application Support/ZzzNoSuchApp12345");
    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(200 * 86_400));
    set_file_mtime(dir.join("data.bin"), old).unwrap();
    set_file_mtime(&dir, old).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "app-support", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ZzzNoSuchApp12345"), "stdout:\n{stdout}");
    assert!(stdout.contains("display only"), "stdout:\n{stdout}");
    assert!(dir.join("data.bin").exists());
}

#[test]
fn non_interactive_clean_does_not_list_leftovers() {
    use filetime::{set_file_mtime, FileTime};
    use std::time::{Duration, SystemTime};

    let tmp = TempDir::new().unwrap();
    put(tmp.path(), "Slack/Cache/big.bin", OVER_THRESHOLD);
    put(tmp.path(), "ZzzNoSuchApp12345/data.bin", 2 * 1024 * 1024);
    let dir = tmp.path().join("Library/Application Support/ZzzNoSuchApp12345");
    let old = FileTime::from_system_time(SystemTime::now() - Duration::from_secs(200 * 86_400));
    set_file_mtime(dir.join("data.bin"), old).unwrap();
    set_file_mtime(&dir, old).unwrap();

    let output = sasurahime(tmp.path())
        .args(["clean", "app-support"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("ZzzNoSuchApp12345"));
    assert!(dir.join("data.bin").exists());
}
```

```bash
cargo test --test app_support 2>&1 | tail -20
```

期待: 7 件 passed。

**12-6. コミット。**

```bash
git add src/cleaners/app_support.rs tests/app_support.rs
git commit -m "feat(app-support): 残骸を表示専用で出し選択時に Finder で開く"
```

### Task 13: 第2段の最終確認

**13-1.** Task 7 で書いた SUPPORTED.md の「Leftovers」「残骸の表示」の記述が実装と一致していることを確認します(30 日以内に更新されたもの、1 MiB 未満、シンボリックリンクは出さない、を各節に1行ずつ足してください)。

英語節に追記:

```markdown
Entries smaller than 1 MiB, modified within the last 30 days, symbolic links,
Apple-provided folders and vendor parent folders (`Google`, `Microsoft`,
`Adobe`, `JetBrains`, and similar) are not listed.
```

日本語節に追記:

```markdown
1 MiB 未満のもの、直近 30 日以内に更新されたもの、シンボリックリンク、Apple 製のフォルダ、
ベンダー親フォルダ(`Google`、`Microsoft`、`Adobe`、`JetBrains` など)は一覧に出しません。
```

**13-2. 最終ゲート。**

```bash
cargo fmt --check && cargo clippy -- -D warnings && cargo test 2>&1 | tail -5
```

期待: fmt と clippy が無出力、`test result: ok.`。

**13-3. コミット。**

```bash
git add docs/SUPPORTED.md
git commit -m "docs(app-support): 残骸表示の除外条件を追記"
```

## 6. main.rs の変更(Task 6 の実装)

### 6.1 登録方法の決定

実装順(04 -> 02 -> 03 -> 05)により、本書の着手時点で 04 の `all_cleaners()` 再構成は完了しています。登録は `define_cleaners!` の標準エントリ 1 つだけで行います。

理由: `AppSupportCleaner` の CLI 引数は `--dry-run` だけで、マクロの `dispatch_clean` が呼ぶ `.clean(dry_run, reporter)` で足ります。標準エントリはマクロが変種、`command_name` / `dry_run`、`SUPPORTED_TARGETS`(`targets`)、ディスパッチ、`standard_cleaners()` 経由の `all_cleaners()`(scan、TUI)まで生成します。02 と 03 も同じ方法で登録済みです。

### 6.2 追加する 2 行

`define_cleaners! {` の表で、標準エントリの最後(03 が追加した `LibraryCaches` エントリの `(...)),` の行の次、単独の `;` だけの行の直前)に追加します(共通規約 5.0)。

引用(この 2 行の次に追加):

```rust
    LibraryCaches : "library-caches" => "Per-app ~/Library/Caches/ listing; known app caches removable (interactive)";
    (|home, _config| cleaners::library_caches::LibraryCachesCleaner::new(home, Box::new(SystemCommandRunner))),
```

追加:

```rust

    AppSupport : "app-support" => "Cache dirs of known apps in ~/Library/Application Support (interactive)";
    (|home, _config| cleaners::app_support::AppSupportCleaner::new(home, Box::new(SystemCommandRunner))),
```

`app-support` は `AUTO_CLEAN_TARGETS` に無いので `ManualOnly` に包まれ、`--yes` では実行されません。

### 6.3 書かないもの

次のものは書きません。同じ変種を特殊側にも書くと clap が重複で panic し、`all_cleaners()` への二重登録は 04 のテスト `standard_and_special_cleaners_do_not_overlap` / `all_cleaners_names_are_unique` が検出します。

- 特殊変種の並び(`;` より後ろ)への `AppSupport { ... }`
- `command_name()`、`dry_run()`、`extra_targets()`、`matches!(target, ...)`、`match target { ... }` の腕
- `special_cleaners()` への追加、`AUTO_CLEAN_TARGETS` への追加
- `--yes` 用の除外リストや除外関数

### 6.4 確認

```bash
cargo build
cargo run -q -- targets 2>/dev/null | grep -c '^app-support'
cargo run -q -- clean app-support --dry-run
cargo test --test app_support
```

期待: ビルド成功、`grep -c` が `1`、dry-run が終了コード 0、E2E が全件 passed。dry-run は実機の `~/Library/Application Support` を読むだけで、何も削除しません。

## 7. エッジケース

| ケース | 期待される挙動 | 保証するテスト |
|--------|----------------|----------------|
| アプリ実行中、終了未承認 | そのアプリの候補は skipped(理由 "X is running")。削除しない。osascript を呼ばない | `apply_blocks_running_app_without_approval` |
| アプリ実行中、終了承認 | osascript で終了を待ち、削除、`open -a` で再起動(relaunch_app があるアプリのみ) | `apply_quits_approved_running_app_then_deletes_and_relaunches` |
| 実行中でなかったアプリ | 再起動しない | `apply_does_not_relaunch_app_that_was_not_running` |
| 終了が10秒で完了しない | `quit_app_and_wait` が Err。そのアプリの候補は skipped | hint 側の既存 `auto_clean_skips_if_quit_times_out` と同じ経路 |
| キャッシュ dir 自体がシンボリックリンク | 候補にならない。再検証でも拒否 | `scan_excludes_symlinked_cache_dir`、`resolve_rejects_symlinked_leaf` |
| root(`Slack` 等)や中間(`Service Worker`)がリンク | 拒否 | `resolve_rejects_symlinked_root`、`resolve_rejects_symlinked_middle_component` |
| キャッシュ内にリンクがある | リンク先は消えない | `apply_does_not_follow_symlinks_inside_cache` |
| 走査後にディレクトリがリンクへ差し替えられた | 削除前の再検証で拒否 | `apply_rejects_dir_replaced_by_symlink_after_scan` |
| 許可リスト外のパス(Local Storage など) | 候補にならない。偽造した Candidate でも削除されない | `scan_returns_only_allowlisted_dirs`、`apply_never_deletes_forged_unlisted_sub` |
| Candidate の path フィールドを偽造 | 無視される(削除は resolve_allowed の結果のみ) | `apply_ignores_forged_path_field` |
| `..` を含むサブパス、絶対パス | NotAllowed | `resolve_rejects_path_traversal_and_absolute` |
| 読めないディレクトリ | 候補にならない(Unreadable)。パニックしない | `resolve_reports_unreadable_parent` |
| Application Support が無い | detect は NotFound、clean は何もせず成功 | `detect_is_not_found_without_application_support`、E2E |
| キャッシュが空 | 候補にならない(サイズ 0) | `scan_skips_empty_dirs_and_missing_base` |
| 合計が閾値(64 MiB)以下 | そのアプリは出さない | `scan_skips_app_below_threshold` |
| 非 TTY | 何も削除しない("not a terminal") | E2E `non_interactive_clean_deletes_nothing` |
| `--yes` | `auto_clean_allowed()` が `false` なので `run_auto` の対象外(`ManualOnly` にも包まれる)。`clean()` は呼ばれない | `app_support_is_never_auto_cleaned`、`app_support_is_registered_but_not_auto_cleaned` |
| 全件削除に失敗 | 解放 0 と skipped で終了コード 1 | `apply_never_deletes_forged_unlisted_sub` |
| 残骸: アプリ名が複数の .app に一致 | 1つでも一致すればインストール済み。残骸にしない | `duplicate_app_names_still_match_once_or_more` |
| 残骸: 外部ボリュームのアプリ | 30 日以内に更新があれば出さない | `leftovers_skips_small_and_recent_and_future` |
| 残骸: Apple 製・ベンダー親・既知アプリ | 出さない | `excluded_names`、`leftovers_skips_excluded_names` |
| 残骸: リンクとファイル | 出さない | `leftovers_skips_files_and_symlinks` |
| 残骸表示で削除 | しない。`open <path>` のみ | `leftovers_never_modify_anything` |
| plutil 失敗 | .app のファイル名だけで照合を続ける | `collect_installed_names_uses_file_stem_and_plist_names` |

## 8. やりがちな間違い、禁止事項、完了条件

### 8.1 やりがちな間違い

- `slice.contains(&sub)` で `&str` の寿命が合わずコンパイルエラーになる: `iter().any(|s| *s == sub)` を使います(本書のとおり)。
- 削除に `Candidate.path` を使う: 禁止です。必ず `resolve_allowed` の戻り値を使います。
- `hint::cleaner::auto_clean_hint` を呼ぶ: 禁止です。`rm -rf` で許可リストを無視します。
- 許可リストに `Service Worker` や `Default` を丸ごと入れる: ユーザーデータ(Database、Login Data)を含むため禁止です。
- `Fake` ランナーで終了コードを `from_raw(1)` と書く: Unix の wait status では 1 は「シグナル1で終了」で意味が違います。失敗は `from_raw(256)` を使います。
- E2E で `--yes` を実行する: 他のクリーナー(bun、go、brew 等)が実機のツールを呼ぶため禁止です。`--yes` で実行されないことは単体テスト `app_support_is_never_auto_cleaned` と `app_support_is_registered_but_not_auto_cleaned` で確認します。
- テストで実機の `~/Library` や `/Applications` に書く: 禁止です。第2段の E2E は `/Applications` を読み取りのみ行います(書き込みなし)。
- マクロの標準エントリに加えて特殊変種(`command_name()` など)にも登録する: 同じサブコマンドが重複して clap がパニックします。`targets` の件数テストで検出します。
- `--yes` から外すために除外リストや除外関数を作る、または `AUTO_CLEAN_TARGETS` に足す: 除外は `auto_clean_allowed()` の `false` だけで行います(共通規約 2.2)。
- `src/hint/mod.rs` の `mod cleaner;` / `mod detector;` を `pub(crate) mod` にする、`is_running` の可視性を変える、`pub use` で再エクスポートする: 共通規約 7.3、7.4 の形(`pub(crate) fn` と `pub(crate) use`)だけです。
- `crate::explorer::open_in_finder` を呼ぶ、または `src/explorer.rs` を変更する: Finder は runner 経由の `open` で開きます。
- 単体テストで `with_app_roots` を付けずに第2段を動かす: 実機の `/Applications` を読み、`plutil` 呼び出しが記録されてテストが失敗します。
- ゴミ箱を使うテスト: 先頭で `crate::trash::set_trash_mode(false);` を呼びます。

### 8.2 禁止事項

- 許可リスト外のパスを削除する変更。ユーザーデータ(設定、ログイン情報、IndexedDB、Local Storage、Partitions、ワークスペース)に触れる変更。
- 第2段の残骸の削除、および `detect()` の合計への算入。
- `--yes` で削除を実行する変更。
- `src/` のコードコメントに変更履歴やタスク ID を書くこと。コメントは非自明な WHY だけにします。
- `git add -A` / `git add .`。
- ドキュメントに EM DASH、「小なりイコール」「大なりイコール」「ほぼ等しい」の各記号を使うこと。

### 8.3 完了条件チェックリスト

- [ ] Task 1 から 7 が完了し、第1段が単独で動く。
- [ ] Task 8 の前提確認が通った(通らない場合は作業を止めて報告)。
- [ ] Task 9 から 13 が完了。
- [ ] `detect()` が残骸を合計に含めない(`detect_ignores_leftovers`)。
- [ ] `--dry-run` が副作用ゼロ(`dry_run_touches_nothing_and_runs_no_commands`、E2E)。
- [ ] MultiSelect の既定が全部未選択(`defaults(&vec![false; n])`)。
- [ ] 許可リストの回避テスト(トラバーサル、リンク、偽造)がすべてある。
- [ ] `main.rs` の変更はマクロ表の標準エントリ 1 つ(6.2 節)と単体テスト 1 件だけ。
- [ ] `docs/SUPPORTED.md` の件数が 51、新節が `## 51.`、stats が `## 52.`。`cargo run -q -- targets | wc -l` が 51。
- [ ] `src/explorer.rs` と `src/hint/detector.rs` に差分が無い。
- [ ] `docs/SUPPORTED.md` の英語節と日本語節、一覧表の両方と、`docs/HOWTO-USE.md` の 2 行を更新した。
- [ ] 実機の `~/Library` を触るテストが無い。

### 8.4 最終確認コマンド

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo run -q -- targets 2>/dev/null | grep -c '^app-support'   # 1 になること
git status --short                                               # 意図したファイルだけ
```
