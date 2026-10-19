# orphan-containers 実装設計書

対象: `sasurahime clean orphan-containers` (新規 Cleaner `OrphanContainersCleaner`)
ブランチ: `feat/apfs-snapshot-enhancement` で、04 (Docker と scan 網羅) の完了後に続けて実装します。
共通規約書: `docs/superpowers/specs/2026-10-20-00-common-conventions.md` を参照してください。順序と衝突確認は `2026-10-20-01-roadmap-and-order.md` にあります。

## 前提 (着手前に確認)

- 実装順は 04 -> 02 (本書) -> 03 -> 05 で固定です。
- 前提タスク: 04 の全タスク (S1 から T5) が完了していること。本書が直接使うのは 04 の S2 (`Cleaner::auto_clean_allowed()`) と S3 (`standard_cleaners()` と `ManualOnly` による `all_cleaners()` の再構成) です。`docs/SUPPORTED.md` の件数は 04 の T5 で 48 になっている前提です。
- 確認コマンド (どれか 1 つでも期待と違えば着手せず報告):

  ```bash
  grep -n "fn auto_clean_allowed" src/cleaner.rs          # 1 行出る
  grep -n "fn standard_cleaners" src/main.rs             # 1 行出る
  grep -n "const AUTO_CLEAN_TARGETS" src/main.rs         # 1 行出る
  cargo run -q -- targets | wc -l                        # 48
  grep -n "48 clean targets" docs/SUPPORTED.md           # 1 行出る
  ```

- 本書が定義する `collect_installed_ids` は 05 (app-support) の第 2 段が使います。名前とシグネチャを変えないでください。
- 既知の既存問題 (今回は触らない): `ios-backup` は `--yes` で TTY 上だと確認プロンプトを出します。

## 未検証の仮定

- Task 1 から 4 のコード (`src/cleaners/orphan_containers.rs`) は、本書の原著者が基準コミットの複製に適用して `cargo fmt --check`、`cargo clippy -- -D warnings`、`cargo test` が通ることを確認したと報告しています (設計書の整理時には再検証していません)。そのまま写してください。ただし Task 4 に追加した `auto_clean_allowed()` のオーバーライドとそのテストは、04 の S2 が前提なので基準コミットではコンパイルできず、未検証です。
- Task 5 の `src/main.rs` の変更 (マクロ表への 1 エントリと単体テスト 1 件) は、04 の実装後の `main.rs` を前提に書き直したため未コンパイルです。
- 04 の `all_cleaners()` が `OrphanContainersCleaner::new` を呼ぶだけで実機の `/Applications` を読まないこと (読むのは `detect()` / `clean()` のとき) は、本書のコードで確認済みです。

---

## 1. 概要と非目標

### 1.1 目的

`~/Library/Containers` と `~/Library/Group Containers` のうち、対応するアプリがインストールされていないフォルダを「残骸候補」として検出し、ユーザーが選んだものだけをゴミ箱へ送ります。

### 1.2 非目標 (やらないこと)

- `~/Library/Application Support` など、Containers 以外の残骸検出は別件です。
- `--all` は提供しません。どのフォルダが不要かは人間の判断が必要なためです。
- `--yes` では実行しません (`auto_clean_allowed()` が `false`)。非 TTY でも実行しません。
- 権限の自動変更 (Full Disk Access の付与) はしません。案内文を出すだけです。
- iOS アプリを Apple シリコン Mac で動かす場合の `Wrapper/*.app` 形式は、bundle ID を読みません (6 章に記載)。
- 新しい依存クレートは追加しません。

### 1.3 実機調査の事実 (読み取り専用の `ls` で確認)

- `~/Library/Containers`: 約 784 件。名前は bundle ID そのものです (例 `com.amazon.Lassen`、`com.amazon.Lassen.SendToKindleExtension`)。
- `~/Library/Group Containers`: 約 137 件。名前の形式は次のとおり複数あります。

| 実在する名前 | 形式 |
|---|---|
| `S8EX82NJP6.com.macpaw.CleanMyMac-mas.HealthMonitor` | チーム ID + bundle ID (親は `com.macpaw.CleanMyMac-mas`) |
| `243LU875E5.groups.com.apple.podcasts` | チーム ID + `groups.` + com.apple |
| `BQR82RBBHL.slack` | チーム ID + 任意の短い名前 (bundle ID ではない) |
| `BQR82RBBHL.com.tinyspeck.slackmacgap` | チーム ID + bundle ID (同じチームの別フォルダ) |
| `com.apple.Home.group` | 接頭辞なし。com.apple で始まる |

- `/Applications` は 31 件。`plutil -extract CFBundleIdentifier raw -o - <Info.plist>` が動作します (バイナリ plist も読めます)。
- 実機を確認し直す場合も `ls` と `plutil -extract` だけを使ってください。`rm` や `mv` は実行しません。

---

## 2. ファイル一覧

| 区分 | パス | 責務 |
|---|---|---|
| 新規 | `src/cleaners/orphan_containers.rs` | 純関数 (正規化、照合、plist 解析) と `OrphanContainersCleaner`。単体テストを同居させる |
| 変更 | `src/cleaners/mod.rs` | `pub mod orphan_containers;` を 1 行追加 |
| 変更 | `src/main.rs` | `define_cleaners!` の標準エントリを 1 つ追加、単体テスト 1 件 |
| 新規 | `tests/orphan_containers.rs` | E2E (`--dry-run` のみ。tempdir を HOME にする) |
| 変更 | `docs/SUPPORTED.md` | 一覧表の行、ターゲット数 48 -> 49、第 49 節の追加と stats 節の 49 -> 50 (英語節と日本語節) |
| 変更 | `docs/HOWTO-USE.md` | 対象表 (英語と日本語) に 1 行ずつ |
| 変更 | `README.md` | Data access 節 (英語と日本語) に 1 行ずつ |

`src/interactive.rs` と `src/cleaner.rs` は変更しません。

`Cargo.toml` は変更しません。

### 2.1 設計判断とその理由

| 判断 | 理由 |
|---|---|
| plist は `plist` クレートを使わず、XML は文字列検索、バイナリ (`bplist00`) だけ macOS 標準の `plutil` に委譲する (共通規約 11.2) | `Cargo.toml` に plist 系クレートは無く、追加すると推移依存が増えます。release は `opt-level = "z"` でサイズ重視です。`plutil` は `CommandRunner` 経由なのでテストで差し替えられ、macOS 専用というプロジェクト方針にも合います |
| `sub_targets()` は実装しない (共通規約 11.1) | TUI が `--sub` 付きの子プロセスを起動するため |
| 登録は `define_cleaners!` の標準エントリ (特殊変種にしない) | CLI 引数が `--dry-run` だけなので、04 の実装後はマクロ表の 1 エントリで変種、`targets`、ディスパッチ、`all_cleaners()` (scan、TUI) まで揃います。特殊変種にすると `command_name` / `dry_run` / `extra_targets` / `matches!` / `match target` / `special_cleaners` の 6 か所を手で触ることになり、03 と 05 の変更と同じ行で衝突しやすくなります |
| インストール先ディレクトリを `with_app_dirs` で注入する (`#[cfg(test)]`) | 実機の `/Applications` に依存するテストを避けるためです。本番コードでは未使用なので `cfg(test)` にしないと `clippy -D warnings` が dead_code で落ちます |
| Group Containers は「インストール済みアプリと一致するフォルダを持つチーム ID は、そのチームの全フォルダを除外」 | `BQR82RBBHL.slack` のような bundle ID でない名前は照合できず、Slack がインストール済みでも残骸と誤判定されるためです。誤検出より見逃しを選びます |
| 保護対象は `com.apple` と `com.docker` を「同名または子孫」で除外 | `group.com.docker` (正規化後 `com.docker`) も Docker 本体のものだからです。`com.applesauce` のような別ベンダは除外しません (ドット境界) |
| 比較は小文字化して行う | macOS のファイルシステムは大文字小文字を区別しない設定が一般的で、bundle ID の綴りが揺れても同じアプリとみなすためです |
| `--yes` 除外は `Cleaner::auto_clean_allowed()` を `false` にオーバーライドして行う (共通規約 2.2) | `clean()` 内の非 TTY 判定だけでは、TTY 上で `--yes` を付けた場合にプロンプトが出てしまい「`--yes` では実行しない」を満たせません。04 の `run_auto` は `auto_clean_allowed()` が `false` の cleaner を対象から外します。`AUTO_CLEAN_TARGETS` に入れないので `ManualOnly` にも包まれます |
| `detect()` は件数を返さず、読めなかった件数は `clean()` で案内する | `ScanResult` には件数欄が無く、`detect()` は並列実行されるので出力もしません |
| 削除は `chflags -R nouchg` -> `crate::trash::delete_path` (既定でゴミ箱) | `library_logs` と同じ手順で、CLAUDE.md の安全規則に従います |
| 空 (サイズ 0) のフォルダは候補にしない | 解放量が無く、`library_logs` も同じ扱いです |

---

## 3. 公開する型と関数のシグネチャ

`src/cleaners/orphan_containers.rs` の公開面です (本文のコードと一致します)。

```rust
pub enum ContainerKind { Containers, GroupContainers }
impl ContainerKind { pub fn label(self) -> &'static str }

pub fn team_id_prefix(name: &str) -> Option<&str>;
pub fn normalize_container_id(name: &str, kind: ContainerKind) -> String;
pub fn is_protected_id(id: &str) -> bool;
pub fn is_owned_by_installed(id: &str, installed: &HashSet<String>) -> bool;
pub fn find_orphan_names(kind: ContainerKind, names: &[String], installed: &HashSet<String>) -> Vec<String>;

pub fn parse_bundle_id_from_xml(xml: &str) -> Option<String>;
pub fn read_bundle_id(plist: &Path, runner: &dyn CommandRunner) -> Option<String>;
pub fn collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String>;
pub fn default_app_dirs(home: &Path) -> Vec<PathBuf>;

pub struct OrphanCandidate { pub path: PathBuf, pub name: String, pub kind: ContainerKind, pub size: u64 }
pub struct OrphanScan { pub candidates: Vec<OrphanCandidate>, pub unreadable: usize }
pub fn scan_roots(roots: &[(ContainerKind, PathBuf)], installed: &HashSet<String>) -> OrphanScan;
pub fn status_from_scan(scan: &OrphanScan) -> crate::cleaner::ScanStatus;

pub struct OrphanContainersCleaner { /* home, app_dirs, runner (非公開) */ }
impl OrphanContainersCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self;
    #[cfg(test)] pub fn with_app_dirs(self, app_dirs: Vec<PathBuf>) -> Self;
    pub fn scan(&self) -> OrphanScan;
}
pub fn format_item(c: &OrphanCandidate) -> String;
pub fn unreadable_hint(count: usize) -> Option<String>;
impl Cleaner for OrphanContainersCleaner { /* name / detect / clean / auto_clean_allowed (false) */ }
```

`main.rs` には関数や定数を追加しません (マクロ表の 1 エントリとテスト 1 件だけです)。

05 (app-support) は `collect_installed_ids` をこのシグネチャのまま呼びます。戻り値の bundle ID は小文字化済みです。

### 3.1 入出力例

`normalize_container_id(name, kind)` (出力は常に小文字):

| name | kind | 結果 | 備考 |
|---|---|---|---|
| `com.foo.App` | Containers | `com.foo.app` | 小文字化のみ |
| `S8EX82NJP6.com.foo` | Containers | `s8ex82njp6.com.foo` | Containers ではチーム ID を外さない |
| `S8EX82NJP6.com.macpaw.CleanMyMac-mas.HealthMonitor` | Group | `com.macpaw.cleanmymac-mas.healthmonitor` | チーム ID を外す |
| `243LU875E5.groups.com.apple.podcasts` | Group | `com.apple.podcasts` | チーム ID と `groups.` |
| `BQR82RBBHL.slack` | Group | `slack` | bundle ID でなくてもそのまま |
| `group.com.foo.App` | Group | `com.foo.app` | `group.` を外す |
| `S8EX82NJP6.group.com.foo` | Group | `com.foo` | チーム ID の後に `group.` |
| `com.apple.Home.group` | Group | `com.apple.home.group` | 末尾の `.group` は外さない。先頭のみ |
| `ABCDE1234.foo` | Group | `abcde1234.foo` | 9 文字はチーム ID ではない |
| `abcde12345.foo` | Group | `abcde12345.foo` | 小文字はチーム ID ではない |
| `S8EX82NJP6.` | Group | `` (空) | 空は `find_orphan_names` が除外 |
| `group.` | Group | `group.` | 接頭辞だけの名前は外さない |
| `あ.com` | Group | `あ.com` | 非 ASCII でも panic しない |

`is_protected_id(id)` (id は正規化済み):

| id | 結果 |
|---|---|
| `com.apple.safari` / `com.apple` | true |
| `com.applesauce.x` | false (ドット境界) |
| `com.docker.docker` / `com.docker` | true |
| `com.dockerx` | false |
| `com.foo.apple` | false |

`is_owned_by_installed(id, installed = {"com.foo.app"})`:

| id | 結果 | 理由 |
|---|---|---|
| `com.foo.app` | true | 完全一致 |
| `com.foo.app.helper` / `com.foo.app.a.b` | true | 親 ID の前方一致 (ドット区切り) |
| `com.foo.appx` | false | 境界がドットでない |
| `com.foo` | false | 親が短い側。子孫ではない |
| `com.bar.app` | false | 無関係 |

`find_orphan_names` (installed の記載がある場合は小文字):

| kind | 入力 | installed | 出力 |
|---|---|---|---|
| Containers | `com.foo.App`, `com.foo.App.Helper`, `com.foo.AppX`, `com.apple.Safari`, `com.docker.docker`, `com.bar.Gone`, `.hidden` | `com.foo.app` | `com.foo.AppX`, `com.bar.Gone` |
| Group | `BQR82RBBHL.com.tinyspeck.slackmacgap`, `BQR82RBBHL.slack`, `243LU875E5.groups.com.apple.podcasts`, `ZZZZZ99999.group.com.example.gone`, `group.com.foo.App`, `com.apple.Home.group`, `S8EX82NJP6.`, `UBF8T346G9.com.microsoft.oneauth` | `com.tinyspeck.slackmacgap`, `com.foo.app` | `ZZZZZ99999.group.com.example.gone`, `UBF8T346G9.com.microsoft.oneauth` |
| Group | `BQR82RBBHL.com.tinyspeck.slackmacgap` | `com.tinyspeck.slackmacgap` | 空 (チーム付きでも同じ ID なら除外) |
| 任意 | 空配列 | 任意 | 空 |

`parse_bundle_id_from_xml`:

| 入力 | 結果 |
|---|---|
| `CFBundleIdentifier` の直後に `<string>com.foo.App</string>` | `Some("com.foo.App")` |
| キーが無い | `None` |
| キーの直後が `<true/>` で、その先に別の `<key>` と `<string>` | `None` (別キーの値を誤って拾わない) |
| 値が空白だけ | `None` |

---

## 4. タスク分割

進め方: 各タスクは (a) 失敗するテストを書く -> (b) 失敗を確認 -> (c) 実装 -> (d) 成功を確認 -> (e) コミット、の順です。作業ディレクトリは `/Users/yaar/Playground/sasurahime` です。

注意:
- テストの実行は `cargo test --bin sasurahime orphan_containers` を使います (単体テストのみ。E2E は Task 5)。
- 途中のタスクでは、まだ呼ばれていない関数に対する `dead_code` 警告が出ます。警告は Task 4 と Task 5 で消えるので、途中では無視してください (`-D warnings` は Task 6 で確認します)。
- 各タスクのコードブロックは「そのタスクで追加する部分」です。インポート行は「置き換え」と書いてある場合だけ、先頭のインポート行全体を差し替えます。
- コミット前に `cargo fmt` を実行してください (コードは整形済みのため差分は出ないはずです)。

### Task 1: ContainerKind と判定の純関数

#### 1-1. モジュールを登録する

`src/cleaners/mod.rs` の `pub mod ollama;` の次の行に追加します。

```rust
pub mod orphan_containers;
```

(結果として `ollama`、`orphan_containers`、`pre_commit` の順になります。)

#### 1-2. 失敗するテストだけのファイルを作る

`src/cleaners/orphan_containers.rs` を新規作成し、次の内容にします。

```rust
use std::collections::HashSet;

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[&str]) -> HashSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    fn strings(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn team_id_prefix_accepts_only_ten_upper_alnum() {
        assert_eq!(team_id_prefix("S8EX82NJP6.com.foo"), Some("S8EX82NJP6"));
        assert_eq!(team_id_prefix("S8EX82NJP.com.foo"), None);
        assert_eq!(team_id_prefix("S8EX82NJP66.com.foo"), None);
        assert_eq!(team_id_prefix("s8ex82njp6.com.foo"), None);
        assert_eq!(team_id_prefix("com.foo.App"), None);
        assert_eq!(team_id_prefix("S8EX82NJP6"), None);
    }

    #[test]
    fn normalize_container_id_table() {
        use ContainerKind::{Containers as C, GroupContainers as G};
        let cases: &[(&str, ContainerKind, &str)] = &[
            ("com.foo.App", C, "com.foo.app"),
            ("S8EX82NJP6.com.foo", C, "s8ex82njp6.com.foo"),
            (
                "S8EX82NJP6.com.macpaw.CleanMyMac-mas.HealthMonitor",
                G,
                "com.macpaw.cleanmymac-mas.healthmonitor",
            ),
            (
                "243LU875E5.groups.com.apple.podcasts",
                G,
                "com.apple.podcasts",
            ),
            ("BQR82RBBHL.slack", G, "slack"),
            ("group.com.foo.App", G, "com.foo.app"),
            ("S8EX82NJP6.group.com.foo", G, "com.foo"),
            ("com.apple.Home.group", G, "com.apple.home.group"),
            ("ABCDE1234.foo", G, "abcde1234.foo"),
            ("abcde12345.foo", G, "abcde12345.foo"),
            ("S8EX82NJP6.", G, ""),
            ("group.", G, "group."),
            ("\u{3042}.com", G, "\u{3042}.com"),
        ];
        for (name, kind, expected) in cases {
            assert_eq!(
                normalize_container_id(name, *kind),
                *expected,
                "name={name} kind={kind:?}"
            );
        }
    }

    #[test]
    fn is_protected_id_table() {
        let cases: &[(&str, bool)] = &[
            ("com.apple.safari", true),
            ("com.apple", true),
            ("com.applesauce.x", false),
            ("com.docker.docker", true),
            ("com.docker", true),
            ("com.dockerx", false),
            ("com.foo.apple", false),
        ];
        for (id, expected) in cases {
            assert_eq!(is_protected_id(id), *expected, "id={id}");
        }
    }

    #[test]
    fn is_owned_by_installed_uses_dot_boundary() {
        let installed = set(&["com.foo.app"]);
        assert!(is_owned_by_installed("com.foo.app", &installed));
        assert!(is_owned_by_installed("com.foo.app.helper", &installed));
        assert!(is_owned_by_installed("com.foo.app.a.b", &installed));
        assert!(!is_owned_by_installed("com.foo.appx", &installed));
        assert!(!is_owned_by_installed("com.foo", &installed));
        assert!(!is_owned_by_installed("com.bar.app", &installed));
    }

    #[test]
    fn find_orphan_names_containers() {
        let names = strings(&[
            "com.foo.App",
            "com.foo.App.Helper",
            "com.foo.AppX",
            "com.apple.Safari",
            "com.docker.docker",
            "com.bar.Gone",
            ".hidden",
        ]);
        let got = find_orphan_names(ContainerKind::Containers, &names, &set(&["com.foo.app"]));
        assert_eq!(got, strings(&["com.foo.AppX", "com.bar.Gone"]));
    }

    #[test]
    fn find_orphan_names_group_containers() {
        let names = strings(&[
            "BQR82RBBHL.com.tinyspeck.slackmacgap",
            "BQR82RBBHL.slack",
            "243LU875E5.groups.com.apple.podcasts",
            "ZZZZZ99999.group.com.example.gone",
            "group.com.foo.App",
            "com.apple.Home.group",
            "S8EX82NJP6.",
            "UBF8T346G9.com.microsoft.oneauth",
        ]);
        let installed = set(&["com.tinyspeck.slackmacgap", "com.foo.app"]);
        let got = find_orphan_names(ContainerKind::GroupContainers, &names, &installed);
        assert_eq!(
            got,
            strings(&[
                "ZZZZZ99999.group.com.example.gone",
                "UBF8T346G9.com.microsoft.oneauth"
            ])
        );
    }

    #[test]
    fn find_orphan_names_group_exact_id_with_team_prefix_is_kept() {
        let names = strings(&["BQR82RBBHL.com.tinyspeck.slackmacgap"]);
        let installed = set(&["com.tinyspeck.slackmacgap"]);
        assert!(find_orphan_names(ContainerKind::GroupContainers, &names, &installed).is_empty());
    }

    #[test]
    fn find_orphan_names_empty_input() {
        assert!(find_orphan_names(ContainerKind::Containers, &[], &set(&[])).is_empty());
    }
}
```

#### 1-3. 失敗を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: コンパイルエラー。`error[E0425]: cannot find function `team_id_prefix` in this scope`、`cannot find type `ContainerKind``、`cannot find function `find_orphan_names``などが並びます。

#### 1-4. 実装を追加する

`use std::collections::HashSet;` の行と `#[cfg(test)]` の行の間に、次のコードを挿入します。

```rust
/// Vendors whose containers are never reported as orphans. Apple data is
/// managed by the OS, and Docker has its own cleaner.
const PROTECTED_VENDORS: &[&str] = &["com.apple", "com.docker"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerKind {
    Containers,
    GroupContainers,
}

impl ContainerKind {
    /// Directory name under `~/Library`, also used as the display label.
    pub fn label(self) -> &'static str {
        match self {
            ContainerKind::Containers => "Containers",
            ContainerKind::GroupContainers => "Group Containers",
        }
    }
}

/// Returns the leading team ID when `name` starts with `"<10 chars of A-Z0-9>."`.
pub fn team_id_prefix(name: &str) -> Option<&str> {
    let (head, _rest) = name.split_once('.')?;
    let is_team_id = head.len() == 10
        && head
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit());
    if is_team_id {
        Some(head)
    } else {
        None
    }
}

// `get` instead of slicing: folder names may contain multi-byte characters,
// and slicing at a non-boundary would panic.
fn strip_group_prefix(s: &str) -> &str {
    for prefix in ["groups.", "group."] {
        if let Some(head) = s.get(..prefix.len()) {
            if s.len() > prefix.len() && head.eq_ignore_ascii_case(prefix) {
                return &s[prefix.len()..];
            }
        }
    }
    s
}

/// Turns a folder name into a lowercase bundle ID for matching.
/// Group Containers lose a leading team ID and then a leading `group.` /
/// `groups.`. Containers names are only lowercased.
pub fn normalize_container_id(name: &str, kind: ContainerKind) -> String {
    let stripped = match kind {
        ContainerKind::Containers => name,
        ContainerKind::GroupContainers => {
            let without_team = match team_id_prefix(name) {
                Some(team) => &name[team.len() + 1..],
                None => name,
            };
            strip_group_prefix(without_team)
        }
    };
    stripped.to_ascii_lowercase()
}

fn is_same_or_child(id: &str, parent: &str) -> bool {
    id == parent
        || (id.len() > parent.len()
            && id.starts_with(parent)
            && id.as_bytes()[parent.len()] == b'.')
}

/// `id` must already be normalized (lowercase).
pub fn is_protected_id(id: &str) -> bool {
    PROTECTED_VENDORS.iter().any(|v| is_same_or_child(id, v))
}

/// True when `id` equals an installed ID or is a dot-separated descendant of
/// one (`com.foo.app.helper` belongs to `com.foo.app`; `com.foo.appx` does not).
/// Both `id` and the set members must be lowercase.
pub fn is_owned_by_installed(id: &str, installed: &HashSet<String>) -> bool {
    if installed.contains(id) {
        return true;
    }
    id.bytes()
        .enumerate()
        .any(|(i, b)| b == b'.' && installed.contains(&id[..i]))
}

/// Returns the folder names (original spelling, input order) that look orphaned.
/// For Group Containers, a team ID that owns at least one installed app keeps
/// all of its folders, because the same team often uses unrelated group names
/// (`TEAM.slack` next to `TEAM.com.tinyspeck.slackmacgap`).
pub fn find_orphan_names(
    kind: ContainerKind,
    names: &[String],
    installed: &HashSet<String>,
) -> Vec<String> {
    let entries: Vec<(&String, String)> = names
        .iter()
        .map(|n| (n, normalize_container_id(n, kind)))
        .collect();

    let alive_teams: HashSet<&str> = if kind == ContainerKind::GroupContainers {
        entries
            .iter()
            .filter(|(_, id)| !id.is_empty() && is_owned_by_installed(id, installed))
            .filter_map(|(n, _)| team_id_prefix(n.as_str()))
            .collect()
    } else {
        HashSet::new()
    };

    entries
        .into_iter()
        .filter(|(n, id)| {
            if id.is_empty() || n.starts_with('.') {
                return false;
            }
            if is_protected_id(id) || is_owned_by_installed(id, installed) {
                return false;
            }
            match team_id_prefix(n.as_str()) {
                Some(team) => !alive_teams.contains(team),
                None => true,
            }
        })
        .map(|(n, _)| n.clone())
        .collect()
}
```

#### 1-5. 成功を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: `test result: ok. 8 passed; 0 failed`。

#### 1-6. コミット

```bash
git add src/cleaners/orphan_containers.rs src/cleaners/mod.rs
git commit -m "feat(orphan-containers): bundle ID の正規化と残骸判定の純関数を追加"
```

### Task 2: インストール済み bundle ID の収集

#### 2-1. インポート行を置き換える

ファイル先頭のインポート行 (`use std::collections::HashSet;` の 1 行) を次の 4 行に置き換えます。

```rust
use crate::command::CommandRunner;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
```

#### 2-2. 失敗するテストを追加する

`mod tests` の閉じ括弧 `}` の直前に、次のコードを追加します。

```rust

    use crate::test_helpers::MockRunner;
    use std::os::unix::process::ExitStatusExt;
    use tempfile::TempDir;

    struct PlutilStub {
        stdout: &'static str,
        exit_code: i32,
    }

    impl CommandRunner for PlutilStub {
        fn run(&self, program: &str, _args: &[&str]) -> anyhow::Result<std::process::Output> {
            if program != "plutil" {
                anyhow::bail!("unexpected program {program}");
            }
            Ok(std::process::Output {
                status: std::process::ExitStatus::from_raw(self.exit_code << 8),
                stdout: self.stdout.as_bytes().to_vec(),
                stderr: vec![],
            })
        }

        fn exists(&self, program: &str) -> bool {
            program == "plutil"
        }
    }

    fn plist_xml(bundle_id: &str) -> String {
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleName</key>\n\t<string>X</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>{bundle_id}</string>\n</dict>\n</plist>\n"
        )
    }

    fn write_app(apps_dir: &Path, rel: &str, bundle_id: &str) {
        let contents = apps_dir.join(rel).join("Contents");
        fs::create_dir_all(&contents).unwrap();
        fs::write(contents.join("Info.plist"), plist_xml(bundle_id)).unwrap();
    }

    #[test]
    fn parse_bundle_id_from_xml_reads_value() {
        assert_eq!(
            parse_bundle_id_from_xml(&plist_xml("com.foo.App")),
            Some("com.foo.App".to_string())
        );
    }

    #[test]
    fn parse_bundle_id_from_xml_missing_key_is_none() {
        let xml = "<dict><key>CFBundleName</key><string>X</string></dict>";
        assert_eq!(parse_bundle_id_from_xml(xml), None);
    }

    #[test]
    fn parse_bundle_id_from_xml_key_without_string_is_none() {
        let xml =
            "<dict><key>CFBundleIdentifier</key><true/><key>Other</key><string>y</string></dict>";
        assert_eq!(parse_bundle_id_from_xml(xml), None);
    }

    #[test]
    fn parse_bundle_id_from_xml_empty_value_is_none() {
        let xml = "<dict><key>CFBundleIdentifier</key><string>  </string></dict>";
        assert_eq!(parse_bundle_id_from_xml(xml), None);
    }

    #[test]
    fn read_bundle_id_reads_xml_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, plist_xml("com.foo.App")).unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(read_bundle_id(&p, &runner), Some("com.foo.App".to_string()));
    }

    #[test]
    fn read_bundle_id_garbage_is_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, [0xffu8, 0xfe, 0x00, 0x01]).unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(read_bundle_id(&p, &runner), None);
    }

    #[test]
    fn read_bundle_id_missing_file_is_none() {
        let tmp = TempDir::new().unwrap();
        let runner = MockRunner::new().with_not_found();
        assert_eq!(
            read_bundle_id(&tmp.path().join("nope.plist"), &runner),
            None
        );
    }

    #[test]
    fn read_bundle_id_binary_plist_uses_plutil() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, b"bplist00\x00\x01\x02").unwrap();
        let runner = PlutilStub {
            stdout: "com.example.bin\n",
            exit_code: 0,
        };
        assert_eq!(
            read_bundle_id(&p, &runner),
            Some("com.example.bin".to_string())
        );
    }

    #[test]
    fn read_bundle_id_binary_plist_plutil_failure_is_none() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Info.plist");
        fs::write(&p, b"bplist00\x00\x01\x02").unwrap();
        let failing = PlutilStub {
            stdout: "",
            exit_code: 1,
        };
        assert_eq!(read_bundle_id(&p, &failing), None);
        let not_found = MockRunner::new().with_not_found();
        assert_eq!(read_bundle_id(&p, &not_found), None);
    }

    #[test]
    fn collect_installed_ids_scans_top_level_and_one_subfolder() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        write_app(&apps, "Foo.app", "com.foo.App");
        write_app(&apps, "Utilities/Bar.app", "com.bar.Tool");
        write_app(&apps, "Utilities/Deep/Baz.app", "com.baz.TooDeep");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[apps], &runner);
        assert_eq!(ids, set(&["com.foo.app", "com.bar.tool"]));
    }

    #[test]
    fn collect_installed_ids_ignores_broken_and_non_app_entries() {
        let tmp = TempDir::new().unwrap();
        let apps = tmp.path().join("Applications");
        fs::create_dir_all(apps.join("Broken.app/Contents")).unwrap();
        fs::create_dir_all(apps.join("NotAnApp/Contents")).unwrap();
        fs::write(
            apps.join("NotAnApp/Contents/Info.plist"),
            plist_xml("com.no.App"),
        )
        .unwrap();
        fs::write(apps.join("file.app"), b"plain file").unwrap();
        write_app(&apps, "Good.app", "com.good.App");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[apps], &runner);
        assert_eq!(ids, set(&["com.good.app"]));
    }

    #[test]
    fn collect_installed_ids_handles_missing_dirs_and_multiple_roots() {
        let tmp = TempDir::new().unwrap();
        let a = tmp.path().join("A");
        let b = tmp.path().join("B");
        write_app(&a, "One.app", "com.one.App");
        write_app(&b, "Two.app", "com.two.App");
        let missing = tmp.path().join("missing");
        let runner = MockRunner::new().with_not_found();
        let ids = collect_installed_ids(&[a, missing, b], &runner);
        assert_eq!(ids, set(&["com.one.app", "com.two.app"]));
    }

    #[test]
    fn default_app_dirs_lists_three_roots() {
        let dirs = default_app_dirs(Path::new("/Users/x"));
        assert_eq!(
            dirs,
            vec![
                PathBuf::from("/Applications"),
                PathBuf::from("/Users/x/Applications"),
                PathBuf::from("/System/Applications"),
            ]
        );
    }
```

#### 2-3. 失敗を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: コンパイルエラー。`cannot find function `parse_bundle_id_from_xml``、`read_bundle_id`、`collect_installed_ids`、`default_app_dirs` が見つからない旨が出ます。

#### 2-4. 実装を追加する

`#[cfg(test)]` の行の直前 (Task 1 の実装の後ろ) に、次のコードを挿入します。

```rust
/// Extracts `CFBundleIdentifier` from an XML plist. Returns `None` when the
/// key is missing or is not directly followed by a `<string>` value.
pub fn parse_bundle_id_from_xml(xml: &str) -> Option<String> {
    const KEY: &str = "<key>CFBundleIdentifier</key>";
    const OPEN: &str = "<string>";
    const CLOSE: &str = "</string>";
    let after = &xml[xml.find(KEY)? + KEY.len()..];
    let open_at = after.find(OPEN)?;
    if after.find("<key>").is_some_and(|k| k < open_at) {
        return None;
    }
    let rest = &after[open_at + OPEN.len()..];
    let value = rest[..rest.find(CLOSE)?].trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Reads `CFBundleIdentifier` from an `Info.plist`. Binary plists (magic
/// `bplist00`) are decoded by `plutil`, which ships with macOS; XML plists are
/// parsed directly so that no extra crate is needed.
pub fn read_bundle_id(plist: &Path, runner: &dyn CommandRunner) -> Option<String> {
    let bytes = fs::read(plist).ok()?;
    if bytes.starts_with(b"bplist00") {
        let path = plist.to_str()?;
        let out = runner
            .run(
                "plutil",
                &["-extract", "CFBundleIdentifier", "raw", "-o", "-", path],
            )
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let id = String::from_utf8(out.stdout).ok()?;
        let id = id.trim();
        return if id.is_empty() {
            None
        } else {
            Some(id.to_string())
        };
    }
    parse_bundle_id_from_xml(std::str::from_utf8(&bytes).ok()?)
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("app"))
        && path.is_dir()
}

/// Lists `*.app` directly under `dir` and one level of sub-folders
/// (for example `/Applications/Utilities`). `.app` bundles are not entered.
fn list_app_bundles(dir: &Path) -> Vec<PathBuf> {
    let mut apps = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return apps;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if is_app_bundle(&path) {
            apps.push(path);
        } else if path.is_dir() {
            if let Ok(sub) = fs::read_dir(&path) {
                apps.extend(sub.flatten().map(|e| e.path()).filter(|p| is_app_bundle(p)));
            }
        }
    }
    apps
}

/// Collects lowercase bundle IDs of the apps found under `app_dirs`.
/// Bundles whose `Info.plist` cannot be read are ignored.
pub fn collect_installed_ids(app_dirs: &[PathBuf], runner: &dyn CommandRunner) -> HashSet<String> {
    let mut ids = HashSet::new();
    for dir in app_dirs {
        for app in list_app_bundles(dir) {
            if let Some(id) = read_bundle_id(&app.join("Contents/Info.plist"), runner) {
                ids.insert(id.to_ascii_lowercase());
            }
        }
    }
    ids
}

pub fn default_app_dirs(home: &Path) -> Vec<PathBuf> {
    vec![
        PathBuf::from("/Applications"),
        home.join("Applications"),
        PathBuf::from("/System/Applications"),
    ]
}
```

#### 2-5. 成功を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: `test result: ok. 21 passed; 0 failed`。

#### 2-6. コミット

```bash
git add src/cleaners/orphan_containers.rs
git commit -m "feat(orphan-containers): インストール済みアプリの bundle ID 収集を追加"
```

### Task 3: Containers と Group Containers の走査

#### 3-1. インポート行を置き換える

ファイル先頭のインポート行 4 行を、次の 5 行に置き換えます (`dir_size` を追加)。

```rust
use crate::command::CommandRunner;
use crate::format::dir_size;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
```

#### 3-2. 失敗するテストを追加する

`mod tests` の閉じ括弧 `}` の直前に追加します。

```rust

    fn write_container(home: &Path, sub: &str, name: &str) -> PathBuf {
        let dir = home.join("Library").join(sub).join(name);
        fs::create_dir_all(dir.join("Data")).unwrap();
        fs::write(dir.join("Data/blob.bin"), vec![1u8; 8192]).unwrap();
        dir
    }

    fn cleaner_for(tmp: &TempDir) -> OrphanContainersCleaner {
        OrphanContainersCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()))
            .with_app_dirs(vec![tmp.path().join("Applications")])
    }

    #[test]
    fn scan_reports_only_orphans_in_containers() {
        let tmp = TempDir::new().unwrap();
        write_app(
            &tmp.path().join("Applications"),
            "Keep.app",
            "com.example.keep",
        );
        write_container(tmp.path(), "Containers", "com.example.keep");
        write_container(tmp.path(), "Containers", "com.example.keep.Helper");
        write_container(tmp.path(), "Containers", "com.apple.Safari");
        let gone = write_container(tmp.path(), "Containers", "com.example.gone");

        let scan = cleaner_for(&tmp).scan();

        assert_eq!(scan.unreadable, 0);
        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].path, gone);
        assert_eq!(scan.candidates[0].name, "com.example.gone");
        assert_eq!(scan.candidates[0].kind, ContainerKind::Containers);
        assert!(scan.candidates[0].size > 0);
    }

    #[test]
    fn scan_keeps_group_containers_of_a_team_with_an_installed_app() {
        let tmp = TempDir::new().unwrap();
        write_app(
            &tmp.path().join("Applications"),
            "Slack.app",
            "com.tinyspeck.slackmacgap",
        );
        write_container(
            tmp.path(),
            "Group Containers",
            "BQR82RBBHL.com.tinyspeck.slackmacgap",
        );
        write_container(tmp.path(), "Group Containers", "BQR82RBBHL.slack");
        write_container(
            tmp.path(),
            "Group Containers",
            "243LU875E5.groups.com.apple.podcasts",
        );
        write_container(tmp.path(), "Group Containers", "com.apple.Home.group");
        let gone = write_container(
            tmp.path(),
            "Group Containers",
            "ZZZZZ99999.group.com.example.gone",
        );

        let scan = cleaner_for(&tmp).scan();

        assert_eq!(scan.candidates.len(), 1);
        assert_eq!(scan.candidates[0].path, gone);
        assert_eq!(scan.candidates[0].kind, ContainerKind::GroupContainers);
    }

    #[test]
    fn scan_of_missing_or_empty_roots_is_empty() {
        let tmp = TempDir::new().unwrap();
        let scan = cleaner_for(&tmp).scan();
        assert!(scan.candidates.is_empty());
        assert_eq!(scan.unreadable, 0);

        fs::create_dir_all(tmp.path().join("Library/Containers")).unwrap();
        fs::create_dir_all(tmp.path().join("Library/Group Containers")).unwrap();
        let scan = cleaner_for(&tmp).scan();
        assert!(scan.candidates.is_empty());
        assert_eq!(scan.unreadable, 0);
    }

    #[test]
    fn scan_skips_symlinks_files_and_zero_size_folders() {
        let tmp = TempDir::new().unwrap();
        let real = write_container(tmp.path(), "Containers", "com.example.real");
        let containers = tmp.path().join("Library/Containers");
        std::os::unix::fs::symlink(&real, containers.join("com.example.link")).unwrap();
        fs::write(containers.join("com.example.file"), vec![1u8; 8192]).unwrap();
        fs::create_dir_all(containers.join("com.example.empty")).unwrap();

        let scan = cleaner_for(&tmp).scan();

        let names: Vec<&str> = scan.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.real"]);
    }

    #[test]
    fn scan_counts_unreadable_folders_without_listing_them() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        let locked = write_container(tmp.path(), "Containers", "com.example.locked");
        write_container(tmp.path(), "Containers", "com.example.open");
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();

        let scan = cleaner_for(&tmp).scan();

        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(scan.unreadable, 1);
        let names: Vec<&str> = scan.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.open"]);
    }

    #[test]
    fn scan_sorts_candidates_by_size_descending() {
        let tmp = TempDir::new().unwrap();
        let small = write_container(tmp.path(), "Containers", "com.example.small");
        let big = write_container(tmp.path(), "Containers", "com.example.big");
        fs::write(big.join("Data/more.bin"), vec![1u8; 65536]).unwrap();
        assert!(small.exists());

        let scan = cleaner_for(&tmp).scan();

        let names: Vec<&str> = scan.candidates.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["com.example.big", "com.example.small"]);
    }

    #[test]
    fn status_from_scan_maps_total_size() {
        use crate::cleaner::ScanStatus;
        let empty = OrphanScan::default();
        assert!(matches!(status_from_scan(&empty), ScanStatus::Clean));
        let one = OrphanScan {
            candidates: vec![OrphanCandidate {
                path: PathBuf::from("/x"),
                name: "x".into(),
                kind: ContainerKind::Containers,
                size: 4096,
            }],
            unreadable: 3,
        };
        assert!(matches!(
            status_from_scan(&one),
            ScanStatus::Pruneable(4096)
        ));
    }

    #[test]
    fn scan_counts_an_unreadable_root_once() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = TempDir::new().unwrap();
        write_container(tmp.path(), "Containers", "com.example.gone");
        let root = tmp.path().join("Library/Containers");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).unwrap();

        let scan = cleaner_for(&tmp).scan();

        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(scan.unreadable, 1);
        assert!(scan.candidates.is_empty());
    }
```

#### 3-3. 失敗を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: コンパイルエラー。`cannot find type `OrphanContainersCleaner``、`OrphanScan`、`OrphanCandidate`、`status_from_scan` が見つからない旨が出ます。

#### 3-4. 実装を追加する

`#[cfg(test)]` の行の直前に挿入します。

```rust
#[derive(Debug, Clone)]
pub struct OrphanCandidate {
    pub path: PathBuf,
    pub name: String,
    pub kind: ContainerKind,
    pub size: u64,
}

#[derive(Debug, Default)]
pub struct OrphanScan {
    /// Sorted by size, largest first.
    pub candidates: Vec<OrphanCandidate>,
    /// Folders (or whole roots) that could not be listed. Without Full Disk
    /// Access, macOS denies access to other apps' containers.
    pub unreadable: usize,
}

/// Names of real sub-directories. Symlinks and plain files are skipped:
/// `DirEntry::file_type` does not follow symlinks.
fn child_dir_names(dir: &Path) -> std::io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)?.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        if let Some(name) = entry.file_name().to_str() {
            names.push(name.to_string());
        }
    }
    Ok(names)
}

pub fn scan_roots(roots: &[(ContainerKind, PathBuf)], installed: &HashSet<String>) -> OrphanScan {
    let mut scan = OrphanScan::default();
    for (kind, root) in roots {
        let names = match child_dir_names(root) {
            Ok(names) => names,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                scan.unreadable += 1;
                continue;
            }
        };
        for name in find_orphan_names(*kind, &names, installed) {
            let path = root.join(&name);
            if fs::read_dir(&path).is_err() {
                scan.unreadable += 1;
                continue;
            }
            let size = dir_size(&path);
            if size == 0 {
                continue;
            }
            scan.candidates.push(OrphanCandidate {
                path,
                name,
                kind: *kind,
                size,
            });
        }
    }
    scan.candidates
        .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    scan
}

pub fn status_from_scan(scan: &OrphanScan) -> crate::cleaner::ScanStatus {
    let total: u64 = scan.candidates.iter().map(|c| c.size).sum();
    if total > 0 {
        crate::cleaner::ScanStatus::Pruneable(total)
    } else {
        crate::cleaner::ScanStatus::Clean
    }
}

pub struct OrphanContainersCleaner {
    home: PathBuf,
    app_dirs: Vec<PathBuf>,
    runner: Box<dyn CommandRunner>,
}

impl OrphanContainersCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            app_dirs: default_app_dirs(home),
            runner,
        }
    }

    /// Replaces the install locations searched for `.app` bundles, so tests
    /// never read the real `/Applications`.
    #[cfg(test)]
    pub fn with_app_dirs(mut self, app_dirs: Vec<PathBuf>) -> Self {
        self.app_dirs = app_dirs;
        self
    }

    fn roots(&self) -> Vec<(ContainerKind, PathBuf)> {
        vec![
            (
                ContainerKind::Containers,
                self.home.join("Library/Containers"),
            ),
            (
                ContainerKind::GroupContainers,
                self.home.join("Library/Group Containers"),
            ),
        ]
    }

    fn any_root_exists(&self) -> bool {
        self.roots().iter().any(|(_, dir)| dir.exists())
    }

    pub fn scan(&self) -> OrphanScan {
        let installed = collect_installed_ids(&self.app_dirs, self.runner.as_ref());
        scan_roots(&self.roots(), &installed)
    }
}
```

#### 3-5. 成功を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: `test result: ok. 29 passed; 0 failed`。`chmod 000` を使うテストは root で実行すると失敗します。通常のユーザーで実行してください。

#### 3-6. コミット

```bash
git add src/cleaners/orphan_containers.rs
git commit -m "feat(orphan-containers): Containers と Group Containers の走査を追加"
```

### Task 4: Cleaner 実装 (detect と clean)

#### 4-1. インポート行を置き換える

ファイル先頭のインポート行 5 行を、次の 9 行に置き換えます。

```rust
use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::collections::HashSet;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
```

#### 4-2. 失敗するテストを追加する

`mod tests` の閉じ括弧 `}` の直前に追加します。

```rust

    use crate::progress::SuppressReporter;

    #[test]
    fn format_item_contains_name_size_and_kind() {
        let c = OrphanCandidate {
            path: PathBuf::from("/x/com.example.gone"),
            name: "com.example.gone".into(),
            kind: ContainerKind::GroupContainers,
            size: 2048,
        };
        let s = format_item(&c);
        assert!(s.contains("com.example.gone"), "{s}");
        assert!(s.contains("2.0 KB"), "{s}");
        assert!(s.contains("[Group Containers]"), "{s}");
    }

    #[test]
    fn unreadable_hint_is_none_for_zero_and_mentions_full_disk_access() {
        assert_eq!(unreadable_hint(0), None);
        let hint = unreadable_hint(3).unwrap();
        assert!(hint.contains("3 folder(s)"), "{hint}");
        assert!(hint.contains("Full Disk Access"), "{hint}");
    }

    #[test]
    fn name_is_orphan_containers() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(cleaner_for(&tmp).name(), "orphan-containers");
    }

    #[test]
    fn detect_without_roots_is_not_found() {
        let tmp = TempDir::new().unwrap();
        let r = cleaner_for(&tmp).detect();
        assert!(matches!(r.status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_with_only_installed_containers_is_clean() {
        let tmp = TempDir::new().unwrap();
        write_app(
            &tmp.path().join("Applications"),
            "Keep.app",
            "com.example.keep",
        );
        write_container(tmp.path(), "Containers", "com.example.keep");
        let r = cleaner_for(&tmp).detect();
        assert!(matches!(r.status, ScanStatus::Clean));
    }

    #[test]
    fn detect_with_orphans_is_pruneable_with_total_size() {
        let tmp = TempDir::new().unwrap();
        write_container(tmp.path(), "Containers", "com.example.gone");
        write_container(
            tmp.path(),
            "Group Containers",
            "ZZZZZ99999.group.com.example.gone",
        );
        let c = cleaner_for(&tmp);
        let expected: u64 = c.scan().candidates.iter().map(|x| x.size).sum();
        let r = c.detect();
        assert!(expected > 0);
        assert_eq!(r.bytes_for_test(), expected);
    }

    #[test]
    fn dry_run_lists_without_deleting() {
        let tmp = TempDir::new().unwrap();
        let gone = write_container(tmp.path(), "Containers", "com.example.gone");
        let result = cleaner_for(&tmp).clean(true, &SuppressReporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert!(result.skipped.is_empty());
        assert!(gone.join("Data/blob.bin").exists());
    }

    #[test]
    fn clean_without_roots_is_empty_success() {
        let tmp = TempDir::new().unwrap();
        let result = cleaner_for(&tmp).clean(false, &SuppressReporter).unwrap();
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(result.exit_code(), 0);
    }

    #[test]
    fn delete_selected_records_failures_as_skipped() {
        let tmp = TempDir::new().unwrap();
        let missing = OrphanCandidate {
            path: tmp.path().join("does-not-exist"),
            name: "does-not-exist".into(),
            kind: ContainerKind::Containers,
            size: 4096,
        };
        let result = cleaner_for(&tmp).delete_selected(&[missing], &SuppressReporter);
        assert_eq!(result.bytes_freed, 0);
        assert_eq!(result.skipped.len(), 1);
        assert_eq!(result.exit_code(), 1);
    }

    #[test]
    fn orphan_containers_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!cleaner_for(&tmp).auto_clean_allowed());
    }
```

#### 4-3. 失敗を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: コンパイルエラー。`cannot find function `format_item``、`unreadable_hint`、`the trait bound ... Cleaner is not satisfied` (`name`、`detect`、`clean` が無い) が出ます。

#### 4-4. 実装を追加する

`#[cfg(test)]` の行の直前に挿入します。

```rust
pub fn format_item(c: &OrphanCandidate) -> String {
    format!(
        "{:<56}  {:>10}  [{}]",
        c.name,
        format_bytes(c.size),
        c.kind.label()
    )
}

/// Message shown when some folders could not be examined. `None` when all were readable.
pub fn unreadable_hint(count: usize) -> Option<String> {
    if count == 0 {
        return None;
    }
    Some(format!(
        "[orphan-containers] {count} folder(s) could not be read and were not checked. \
         Grant Full Disk Access to your terminal \
         (System Settings > Privacy & Security > Full Disk Access) to check all of them."
    ))
}

fn empty_result(name: &'static str) -> CleanResult {
    CleanResult {
        name,
        bytes_freed: 0,
        uses_trash: false,
        skipped: vec![],
        deleted_paths: vec![],
    }
}

fn print_unreadable_hint(count: usize) {
    if let Some(hint) = unreadable_hint(count) {
        eprintln!("{hint}");
    }
}

impl OrphanContainersCleaner {
    fn interactive_select(candidates: &[OrphanCandidate]) -> Result<Vec<OrphanCandidate>> {
        use dialoguer::MultiSelect;

        let items: Vec<String> = candidates.iter().map(format_item).collect();
        println!("\nContainers of apps that are not installed (nothing is selected by default):\n");
        let selections = MultiSelect::new()
            .with_prompt("Select folders to move to Trash  [space to toggle, enter to confirm]")
            .items(&items)
            .interact()?;
        Ok(selections
            .into_iter()
            .map(|i| candidates[i].clone())
            .collect())
    }

    fn delete_selected(
        &self,
        selected: &[OrphanCandidate],
        reporter: &dyn ProgressReporter,
    ) -> CleanResult {
        let mut freed: u64 = 0;
        let mut skipped = Vec::new();
        let mut deleted_paths = Vec::new();
        reporter.progress_init(self.name(), selected.len());
        for (i, c) in selected.iter().enumerate() {
            reporter.progress_tick(&c.path, i + 1, c.size);
            let path_str = c.path.to_string_lossy();
            let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
            match crate::trash::delete_path(&c.path) {
                Ok(()) => {
                    freed += c.size;
                    deleted_paths.push(c.path.clone());
                    println!(
                        "[orphan-containers] removed: {} (freed {})",
                        c.path.display(),
                        format_bytes(c.size)
                    );
                }
                Err(e) => {
                    eprintln!(
                        "[orphan-containers] error removing {}: {e}",
                        c.path.display()
                    );
                    skipped.push(SkippedEntry {
                        path: c.path.clone(),
                        reason: format!("{e:#}"),
                    });
                }
            }
        }
        reporter.progress_finish();
        println!("[orphan-containers] total freed: {}", format_bytes(freed));
        CleanResult {
            name: self.name(),
            bytes_freed: freed,
            uses_trash: crate::trash::is_trash_mode(),
            skipped,
            deleted_paths,
        }
    }
}

impl Cleaner for OrphanContainersCleaner {
    fn name(&self) -> &'static str {
        "orphan-containers"
    }

    fn detect(&self) -> ScanResult {
        if !self.any_root_exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let mut r = ScanResult::new(self.name(), status_from_scan(&self.scan()));
        if crate::context::is_verbose() {
            r = r.with_target(self.home.join("Library/Containers").to_string_lossy());
        }
        r
    }

    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.any_root_exists() {
            println!("[orphan-containers] nothing to clean");
            return Ok(empty_result(self.name()));
        }

        let scan = self.scan();
        if scan.candidates.is_empty() {
            println!("[orphan-containers] nothing to clean");
            print_unreadable_hint(scan.unreadable);
            return Ok(empty_result(self.name()));
        }

        if dry_run {
            println!(
                "[orphan-containers] dry-run: {} orphan candidate(s)",
                scan.candidates.len()
            );
            for c in &scan.candidates {
                println!(
                    "  would remove: {}  ({}, {})",
                    c.path.display(),
                    format_bytes(c.size),
                    c.kind.label()
                );
            }
            print_unreadable_hint(scan.unreadable);
            return Ok(empty_result(self.name()));
        }

        // Which folders are safe to remove is a judgement call that only the
        // user can make, so there is no non-interactive path (no `--all`).
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "[orphan-containers] not a terminal - skipping. Run \
                 `sasurahime clean orphan-containers` to choose folders interactively."
            );
            return Ok(empty_result(self.name()));
        }

        let selected = Self::interactive_select(&scan.candidates)?;
        if selected.is_empty() {
            println!("[orphan-containers] nothing selected");
            print_unreadable_hint(scan.unreadable);
            return Ok(empty_result(self.name()));
        }

        let result = self.delete_selected(&selected, reporter);
        print_unreadable_hint(scan.unreadable);
        Ok(result)
    }

    // Which folders are leftovers is a human judgement, so `--yes` must skip this
    // cleaner even on a terminal where the prompt would otherwise appear.
    fn auto_clean_allowed(&self) -> bool {
        false
    }
}
```

#### 4-5. 成功を確認する

```bash
cargo test --bin sasurahime orphan_containers
```

期待: `test result: ok. 39 passed; 0 failed`。

テストが走るとき、`delete_selected_records_failures_as_skipped` は存在しないパスを `trash::delete` に渡し、Finder 経由のエラーを得ます (数秒かかる場合があります)。実在するファイルはゴミ箱へ送られません。

#### 4-6. コミット

```bash
git add src/cleaners/orphan_containers.rs
git commit -m "feat(orphan-containers): detect と対話的 clean を持つ Cleaner を追加"
```

### Task 5: main.rs への登録と E2E テスト

#### 5-1. 失敗するテストを書く

E2E: `tests/orphan_containers.rs` を新規作成します。

```rust
use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn sasurahime(home: &Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn make_container(home: &Path, sub: &str, name: &str) -> PathBuf {
    let dir = home.join("Library").join(sub).join(name);
    fs::create_dir_all(dir.join("Data")).unwrap();
    fs::write(dir.join("Data/blob.bin"), vec![1u8; 8192]).unwrap();
    dir
}

#[test]
fn clean_orphan_containers_dry_run_lists_orphans_and_keeps_files() {
    let tmp = TempDir::new().unwrap();
    let gone = make_container(tmp.path(), "Containers", "com.example.sasurahimetest.gone");
    let group = make_container(
        tmp.path(),
        "Group Containers",
        "ZZZZZ99999.group.com.example.sasurahimetest.gone",
    );
    make_container(tmp.path(), "Containers", "com.apple.sasurahimetest");

    let output = sasurahime(tmp.path())
        .args(["clean", "orphan-containers", "--dry-run"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("com.example.sasurahimetest.gone"),
        "stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("ZZZZZ99999.group.com.example.sasurahimetest.gone"),
        "stdout:\n{stdout}"
    );
    assert!(
        !stdout.contains("com.apple.sasurahimetest"),
        "com.apple.* must be excluded:\n{stdout}"
    );
    assert!(gone.exists() && group.exists(), "dry-run must not delete");
}

#[test]
fn clean_orphan_containers_without_library_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "orphan-containers", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
}

#[test]
fn targets_lists_orphan_containers() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path()).arg("targets").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("orphan-containers"), "stdout:\n{stdout}");
}

#[test]
fn clean_orphan_containers_rejects_all_flag() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .args(["clean", "orphan-containers", "--all"])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
```

`src/main.rs` の `mod tests` の末尾 (最後のテスト関数の後ろ、`mod tests` を閉じる `}` の直前) に追加します。04 の S1 で追加された `names_of` を使います。

```rust
    #[test]
    fn orphan_containers_is_registered_but_not_auto_cleaned() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        assert!(names_of(&cleaners).contains(&"orphan-containers"));
        let c = cleaners
            .iter()
            .find(|c| c.name() == "orphan-containers")
            .unwrap();
        assert!(!c.auto_clean_allowed());
    }
```

#### 5-2. 失敗を確認する

```bash
cargo test --test orphan_containers
```

期待: 4 件中 3 件が失敗します (`dry_run_lists_orphans_and_keeps_files`、`without_library_exits_zero`、`targets_lists_orphan_containers`)。`error: unrecognized subcommand 'orphan-containers'` が stderr に出ます。`rejects_all_flag` は未登録でもサブコマンド不明で失敗終了するため、この時点でも通ります。

```bash
cargo test --bin sasurahime orphan_containers_is_registered
```

期待: `assertion failed` で 1 件失敗します (`all_cleaners()` にまだ無いため)。

#### 5-3. main.rs を変更する (1 か所)

`define_cleaners! {` の表で、標準エントリの最後 (`TreeSitter` エントリの `(...)),` の行の次、単独の `;` だけの行の直前) に次の 2 行を追加します (共通規約 5.0)。

引用 (この 2 行の次に追加):

```rust
    TreeSitter : "tree-sitter" => "tree-sitter parser compilation cache";
    (|home, config| cleaners::generic::GenericCleaner::tree_sitter(home, Box::new(SystemCommandRunner)).with_config(config)),
```

追加:

```rust

    OrphanContainers : "orphan-containers" => "Leftover Containers / Group Containers of uninstalled apps (interactive)";
    (|home, _config| cleaners::orphan_containers::OrphanContainersCleaner::new(home, Box::new(SystemCommandRunner))),
```

これで `CleanTarget::OrphanContainers { dry_run }`、`sasurahime clean orphan-containers [--dry-run]`、`sasurahime targets` の行、`standard_cleaners()` 経由の `all_cleaners()` (scan と TUI) への登録が揃います。`orphan-containers` は `AUTO_CLEAN_TARGETS` に無いので `ManualOnly` に包まれ、`--yes` では実行されません。

次のものは書きません (書くと clap の重複 panic や二重登録になります):

- 特殊変種の並び (`;` より後ろ) への `OrphanContainers { ... }`
- `command_name()`、`dry_run()`、`extra_targets()`、`matches!(target, ...)`、`match target { ... }` の腕
- `special_cleaners()` への追加
- `AUTO_CLEAN_TARGETS` への追加
- `INTERACTIVE_ONLY_CLEANERS` のような除外リストや除外関数

#### 5-4. 成功を確認する

```bash
cargo fmt --check
cargo test --test orphan_containers
cargo test --bin sasurahime orphan_containers_is_registered
cargo test --bin sasurahime
cargo test --test coverage
cargo test --test interactive
```

期待: `orphan_containers` E2E は `4 passed`、`orphan_containers_is_registered_but_not_auto_cleaned ... ok`、`cargo test --bin sasurahime` の全件 (04 が追加した `only_legacy_targets_are_auto_clean_allowed`、`standard_and_special_cleaners_do_not_overlap`、`standard_cleaners_follow_the_macro_table_order` など) が全件 ok、`coverage` は 2 件 ok (`targets` に載った名前が `clean <name> --help` で成功し、重複も無いことの確認)、`interactive` も全件 ok。

#### 5-5. コミット

```bash
git add src/main.rs tests/orphan_containers.rs
git commit -m "feat(orphan-containers): clean サブコマンドと一覧への登録、E2E テストを追加"
```

### Task 6: ドキュメントと最終確認

#### 6-1. ドキュメントを更新する (SUPPORTED.md 6 か所、HOWTO-USE.md 2 か所、README.md 2 か所)

番号と件数は共通規約 5.2 の表のとおりです (02 完了後: 件数 49、新節 `## 49.`、stats は `## 50.`)。

(1) `docs/SUPPORTED.md` 英語節の冒頭の件数。`sasurahime provides **48 clean targets**.` を `sasurahime provides **49 clean targets**.` に変更します。

(2) 日本語節の件数。`sasurahime は **48 のクリーンターゲット** を提供しています。` を `sasurahime は **49 のクリーンターゲット** を提供しています。` に変更します。

(3) 英語の Quick Reference 表。`| \`trash\` |` で始まる行の直前に追加します。

```markdown
| `orphan-containers` | Leftover `~/Library/Containers` and `~/Library/Group Containers` of uninstalled apps - interactive only, never in `--yes` |
```

(4) 日本語のクイックリファレンス表。`| \`trash\` |` で始まる行の直前に追加します。

```markdown
| `orphan-containers` | アンインストール済みアプリの `~/Library/Containers` と `~/Library/Group Containers` の残骸 - 対話的のみ、`--yes` では実行しない |
```

(5) 英語の第 49 節。英語節の `## 49. \`sasurahime stats\`` の見出しを `## 50. \`sasurahime stats\`` に変更し、その見出しの直前にある `---` 行の前に、次の内容を挿入します (挿入後は `... dotnet 節の本文`、`---`、`## 49. orphan-containers`、`---`、`## 50. stats` の順になります)。

```markdown
---

## 49. `sasurahime clean orphan-containers`

**Category:** Sprint 6

**What it removes:** Folders under `~/Library/Containers` and
`~/Library/Group Containers` whose app is not installed. Selected folders are
moved to the Trash (use `--permanent` to delete them instead).

**How detect works:**
1. Collects the `CFBundleIdentifier` of every `*.app` directly under
   `/Applications`, `~/Applications` and `/System/Applications`, plus one level
   of sub-folders (for example `/Applications/Utilities`). Bundles whose
   `Info.plist` cannot be read are ignored.
2. A `Containers` folder is kept when its name equals an installed ID or is a
   dot-separated child of one (`com.foo.App.Helper` belongs to `com.foo.App`;
   `com.foo.AppX` does not). Matching is case-insensitive.
3. For `Group Containers`, a leading team ID (10 uppercase letters or digits
   plus `.`) and then a leading `group.` / `groups.` are removed before the same
   matching. If any folder of a team matches an installed app, every folder of
   that team is kept.
4. `com.apple.*` (including `group.com.apple.*`) and `com.docker.*` are never
   reported.
5. Returns `Pruneable` with the total size of the candidates, or `Clean` when
   there are none. Folders that cannot be read (no Full Disk Access) are not
   counted.

**How clean works:**
1. `--dry-run` lists the candidates and removes nothing.
2. Without `--dry-run`, a `dialoguer::MultiSelect` prompt opens with nothing
   selected. Only the chosen folders are moved to the Trash.
3. There is no `--all`. With a non-TTY stdin, or under `--yes`, nothing is
   removed.
4. When some folders could not be read, the number of them is printed together
   with a hint to grant Full Disk Access to your terminal. sasurahime never
   changes permissions.

**Limits:** Containers of iOS apps running on Apple silicon (`Wrapper/*.app`)
are not matched to an installed app and may be listed; review the list before
selecting.

**Safety:**
- Nothing is selected by default; `--yes` never runs this cleaner.
- `--dry-run` has no side effects.

```

(6) 日本語の第 49 節。日本語節の `## 49. \`sasurahime stats\`` の見出しを `## 50. \`sasurahime stats\`` に変更し、その見出しの直前にある `---` 行の前に、次の内容を挿入します。

```markdown
---

## 49. `sasurahime clean orphan-containers`

**カテゴリ:** Sprint 6

**削除対象:** 対応するアプリがインストールされていない `~/Library/Containers` と
`~/Library/Group Containers` 配下のフォルダ。選択したフォルダはゴミ箱へ移動します
（`--permanent` を付けると完全削除します）。

**detect の動作:**
1. `/Applications`、`~/Applications`、`/System/Applications` の直下と、その 1 段下の
   サブフォルダ（例 `/Applications/Utilities`）にある `*.app` の `CFBundleIdentifier`
   を集めます。`Info.plist` を読めない .app は無視します。
2. `Containers` のフォルダは、名前がインストール済み ID と一致するか、そのドット区切り
   の子（`com.foo.App.Helper` は `com.foo.App` に属します。`com.foo.AppX` は属しません）
   であれば除外します。大文字小文字は区別しません。
3. `Group Containers` は、先頭のチーム ID（英数字大文字 10 文字 + `.`）と、続く
   `group.` / `groups.` を外してから同じ照合をします。あるチームのフォルダが 1 つでも
   インストール済みアプリと一致する場合、そのチームの全フォルダを除外します。
4. `com.apple.*`（`group.com.apple.*` を含む）と `com.docker.*` は報告しません。
5. 候補の合計サイズで `Pruneable` を返します。候補がなければ `Clean` です。
   読み取れないフォルダ（Full Disk Access なし）は数えません。

**clean の動作:**
1. `--dry-run` は候補を一覧表示するだけで、何も削除しません。
2. `--dry-run` なしでは `dialoguer::MultiSelect` を開きます。既定では何も選択されて
   いません。選択したフォルダだけをゴミ箱へ移動します。
3. `--all` はありません。非 TTY の場合と `--yes` では何も削除しません。
4. 読み取れなかったフォルダがあるときは、その件数と、ターミナルに Full Disk Access を
   付与すると全件を調べられる旨を表示します。sasurahime が権限を変更することはありません。

**制限:** Apple シリコンで動作する iOS アプリ（`Wrapper/*.app`）のコンテナは、
インストール済みアプリと対応付けられず候補に出ることがあります。選択前に一覧を確認してください。

**安全性:**
- 既定では何も選択されません。`--yes` ではこの cleaner は実行されません。
- `--dry-run` は副作用ゼロです。

```

(7) `docs/HOWTO-USE.md` の英語の対象表 (99 行付近)。`| \`ollama\`` で始まる行の次に追加します。

```markdown
| `orphan-containers` | Leftover `~/Library/Containers` / `~/Library/Group Containers` of uninstalled apps (interactive only) |
```

(8) `docs/HOWTO-USE.md` の日本語の対象表 (671 行付近)。`| \`ollama\`` で始まる行の次に追加します。

```markdown
| `orphan-containers` | アンインストール済みアプリの `~/Library/Containers` / `~/Library/Group Containers` の残骸（対話的のみ） |
```

(9) `README.md` の英語の「Data access」節。04 が追加した `- \`~/Library/Containers/com.docker.docker/Data/\`` で始まる行の次に追加します (共通規約 11.5)。

```markdown
- `~/Library/Containers/`, `~/Library/Group Containers/`, and the `Info.plist` of `.app` bundles in `/Applications`, `~/Applications`, `/System/Applications` (bundle IDs and app names only; `sasurahime clean orphan-containers` and `app-support`)
```

(10) `README.md` の日本語の「アクセスするデータ」節。04 が追加した `- \`~/Library/Containers/com.docker.docker/Data/\`` で始まる行の次に追加します。

```markdown
- `~/Library/Containers/`、`~/Library/Group Containers/`、`/Applications`・`~/Applications`・`/System/Applications` にある `.app` の `Info.plist`（bundle ID とアプリ名のみ。`sasurahime clean orphan-containers` と `app-support` で使用）
```

確認:

```bash
cargo run -q -- targets | wc -l                      # 49
grep -n "49 clean targets\|49 のクリーンターゲット" docs/SUPPORTED.md   # 2 行
grep -n "^## 49\. \|^## 50\. " docs/SUPPORTED.md     # orphan-containers 2 行、stats 2 行
```

#### 6-2. 最終確認コマンド

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
git status --short
```

期待:
- `cargo fmt --check` は出力なし。
- `cargo clippy -- -D warnings` は警告 0 (`cargo clippy --all-targets` では HEAD から存在する `src/cleaner.rs` の `#[expect(dead_code)]` 未達成の警告が出る場合があります。これは今回の変更とは無関係なので、プロジェクト規定の `cargo clippy -- -D warnings` を基準にします)。
- `cargo test` は全件成功 (02 の着手前から 44 件増えます。単体 39 件、`main.rs` の単体 1 件、E2E 4 件)。
- `git status --short` は `?? docs/superpowers/` 以外に未コミットの変更がないこと。

#### 6-3. コミット

```bash
git add docs/SUPPORTED.md docs/HOWTO-USE.md README.md
git commit -m "docs(orphan-containers): SUPPORTED.md、HOWTO-USE.md、README.md に orphan-containers を追記"
```

---

## 5. 実機での手動確認 (任意、読み取り専用)

```bash
cargo run -- clean orphan-containers --dry-run
```

実機の `~/Library` を読むだけで、`--dry-run` は何も削除しません。Full Disk Access が無い端末では、読み取れない件数の案内が出ます。対話的な削除 (`--dry-run` なし) は、一覧を確認してから自分で選択する場合のみ実行してください。自動テストでは絶対に実行しません。

---

## 6. エッジケース表

| ケース | 挙動 | テスト |
|---|---|---|
| Containers 内のシンボリックリンク | `DirEntry::file_type` はリンクを辿らないため、ディレクトリ扱いにならず無視する | 要 (`scan_skips_symlinks_files_and_zero_size_folders`) |
| Containers 内の通常ファイル、`.` で始まる名前 | 無視する | 要 (同上、`find_orphan_names_containers` の `.hidden`) |
| `/Applications` 内のシンボリックリンクの `.app` | `is_dir` はリンクを辿るので実体として読む。再帰は 1 段のみでループしない | 不要 (std の挙動) |
| 読めないフォルダ (Full Disk Access 無し) | `read_dir` 失敗を `unreadable` に数え、候補に含めない | 要 (`scan_counts_unreadable_folders_without_listing_them`) |
| `~/Library/Containers` 自体が読めない | root 1 件として `unreadable` に数える | 要 (`scan_counts_an_unreadable_root_once`) |
| root ディレクトリが存在しない | 数えずにスキップ。両方無ければ `detect` は `NotFound` | 要 (`detect_without_roots_is_not_found`) |
| 空の Containers | 候補ゼロ、`detect` は `Clean` | 要 (`scan_of_missing_or_empty_roots_is_empty`) |
| サイズ 0 のフォルダ | 候補にしない | 要 (同上のシンボリックリンク用テスト内の `com.example.empty`) |
| `.app` が壊れている (Info.plist 無し、キー無し、バイナリ読取失敗) | その .app は無視する。ID が集まらないので、その ID のコンテナは候補に出る | 要 (`collect_installed_ids_ignores_broken_and_non_app_entries`、`read_bundle_id_*`) |
| バイナリ plist | `plutil` に委譲。失敗や未インストールなら `None` | 要 (`read_bundle_id_binary_plist_*`) |
| 大文字小文字の違い | 両側を小文字化して照合する | 要 (正規化表、`collect_installed_ids_scans_top_level_and_one_subfolder`) |
| チーム ID 付き Group Containers で、インストール済みアプリが同じ ID を持つ | 正規化後に完全一致するので除外 | 要 (`find_orphan_names_group_exact_id_with_team_prefix_is_kept`) |
| 同じチーム ID の別名フォルダ (`TEAM.slack`) | チーム内に一致があれば全て除外 | 要 (`scan_keeps_group_containers_of_a_team_with_an_installed_app`) |
| `com.apple.*` と `group.com.apple.*`、`groups.com.apple.*` | 常に除外 | 要 (保護表、Group の `find_orphan_names`) |
| Docker (`com.docker.docker`、`group.com.docker`) | 除外 | 要 (保護表) |
| `com.foo.AppX` と `com.foo.App` | ドット境界で別物。誤一致させない | 要 (`is_owned_by_installed_uses_dot_boundary`) |
| 非 ASCII のフォルダ名 | 正規化でスライスしないため panic しない。UTF-8 でない名前は一覧に出さない | 要 (正規化表の `あ.com`) |
| 削除に失敗 (ゴミ箱エラー、権限) | `skipped` に載せ、全件失敗なら終了コード 1 | 要 (`delete_selected_records_failures_as_skipped`) |
| 非 TTY で `--dry-run` なし | 何も削除せず終了コード 0 | 単体・E2E とも不要 (テスト実行時の stdin が端末だと実機の対話が走り得るため書かない) |
| `--yes` | `auto_clean_allowed()` が `false` なので `run_auto` の対象外 (`ManualOnly` にも包まれる) | 要 (`orphan_containers_is_never_auto_cleaned`、`orphan_containers_is_registered_but_not_auto_cleaned`) |
| `--all` を付ける | 引数が無いのでエラー終了 | 要 (E2E `rejects_all_flag`) |
| iOS アプリ (Wrapper) のコンテナ | 判別しない。候補に出ても既定で未選択 | 不要 (制限として文書化) |
| `uchg` フラグ | 削除前に `chflags -R nouchg` を試す (失敗は無視) | 不要 (`library_logs` と同じ手順) |

---

## 7. 実装者がやりがちな間違いと禁止事項

禁止事項:
1. 実機の `~/Library` や `/Applications` を書き換えるテストを書かない。テストは `TempDir` を HOME にし、`with_app_dirs` で `tempdir/Applications` を渡します。
2. `delete_selected` や `clean(false, ...)` に実在するフォルダを渡すテストを書かない (ゴミ箱に実際に送られます)。`clean(false)` は端末で対話が始まるので単体テストから呼びません。
3. E2E は `--dry-run` のみです。`--dry-run` なしの E2E を書かない。
4. `--all` を追加しない。`--yes` で動かさない。
5. 新しい依存クレートを `Cargo.toml` に追加しない (`plist` を含む)。
6. `git add -A` / `git add .` を使わない。`docs/superpowers/` を巻き込まない。
7. `.env` や認証情報ファイルを読まない。

やりがちな間違い:
1. `with_app_dirs` から `#[cfg(test)]` を外す。`clippy -D warnings` が `method is never used` で失敗します。
2. 前方一致を `starts_with(parent)` だけで書く。`com.foo.AppX` を誤一致させます。必ずドット境界を見ます (`is_owned_by_installed`)。
3. チーム ID の判定を小文字や 9 文字・11 文字に広げる。`team_id_prefix` は「ちょうど 10 文字の英大文字と数字」だけです。
4. 文字列を `&s[..n]` で切る。非 ASCII の名前で panic します。`str::get` を使います。
5. `detect()` の中で `println!` や `eprintln!` をする。並列スキャンの表示が崩れます。案内は `clean()` だけで出します。
6. `ScanStatus` に `PermissionDenied` を足す。`cleaner.rs` の方針で意図的に省略されています。
7. コメントに「何をしているか」や変更履歴を書く。CLAUDE.md の規約で、非自明な WHY だけを書きます。
8. 設計書のコードの文字列にある `-` を 全角のダッシュ (EM DASH) に変える。非 TTY の案内文は ASCII のハイフンです。
9. `--yes` から外すために除外リストや除外関数を `main.rs` / `interactive.rs` に作る、または `orphan-containers` を `AUTO_CLEAN_TARGETS` に足す。除外は `auto_clean_allowed()` の `false` だけで行います (共通規約 2.2)。TUI (`run_interactive`) では選択して実行できます。
10. SUPPORTED.md に issue 番号や経緯を書く。最新仕様のスナップショットだけを書きます。
11. 特殊変種として `command_name()` などに手で登録する。マクロ表の標準エントリ 1 つだけです (5-3)。

---

## 8. 完了条件チェックリスト

- [ ] `src/cleaners/orphan_containers.rs` が新規作成され、Task 1 から 4 のコードがすべて入っている
- [ ] `src/cleaners/mod.rs` に `pub mod orphan_containers;` がある
- [ ] `src/main.rs` の変更がマクロ表の標準エントリ 1 つ (5-3) と単体テスト 1 件だけである
- [ ] `tests/orphan_containers.rs` の E2E 4 件が成功する (すべて tempdir の HOME、`--dry-run` のみ)
- [ ] `sasurahime targets` に `orphan-containers` が出る
- [ ] `sasurahime scan` の一覧に `orphan-containers` が出る
- [ ] `sasurahime --yes` が `orphan-containers` を実行しない (`orphan_containers_is_never_auto_cleaned`、`orphan_containers_is_registered_but_not_auto_cleaned`)
- [ ] `docs/SUPPORTED.md` の 6 か所 (件数 48 -> 49 が 2、表 2、第 49 節 2 と stats の 50 への繰り下げ) と、`docs/HOWTO-USE.md` の 2 行、`README.md` の 2 行が更新されている
- [ ] `cargo run -q -- targets | wc -l` が 49
- [ ] 新規依存が `Cargo.toml` に無い
- [ ] 実機の `~/Library` に変更を加えていない
- [ ] コミットは 6 つ (Task 1 から 6)、すべて日本語 Conventional Commits、個別 `git add`

最終確認コマンド:

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
git log --oneline -6
git status --short
```
