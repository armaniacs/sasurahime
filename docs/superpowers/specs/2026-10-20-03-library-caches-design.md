# library-caches 実装設計書

共通規約書 (`docs/superpowers/specs/2026-10-20-00-common-conventions.md`) を参照してください。順序と衝突確認は `2026-10-20-01-roadmap-and-order.md` にあります。

## 前提 (着手前に確認)

- 実装順は 04 -> 02 -> 03 (本書) -> 05 で固定です。
- 前提タスク: 02 (orphan-containers) の全タスクが完了していること (02 の前提として 04 も完了済み)。コード上は 04 の S2 (`Cleaner::auto_clean_allowed()`) と S3 (`standard_cleaners()` と `ManualOnly`) だけを使います。`docs/SUPPORTED.md` は 02 の完了後の状態 (件数 49、`## 49. orphan-containers`、stats は `## 50.`) を前提にします。
- 確認コマンド (どれか 1 つでも期待と違えば着手せず報告):

  ```bash
  grep -n "fn auto_clean_allowed" src/cleaner.rs          # 1 行出る
  grep -n "fn standard_cleaners" src/main.rs             # 1 行出る
  grep -n "OrphanContainers :" src/main.rs               # 1 行出る
  cargo run -q -- targets | wc -l                        # 49
  grep -n "^## 50\. \`sasurahime stats\`" docs/SUPPORTED.md   # 2 行出る
  ```

- 本書の Task 1 が行う `hint::is_running` の可視性変更 (`pub(crate) fn` と `pub(crate) use detector::is_running;`) は、後続の 05 (app-support) も使います。共通規約 7.3 の形から変えないでください。

## 未検証の仮定

- 本書のコード片はコンパイル未確認です。
- `known_entry_for` の `e.path_suffixes == [name]` (`&'static [&'static str]` と `[&str; 1]` の比較) は標準ライブラリの `PartialEq` 実装で通る想定です。コンパイルエラーになった場合は止まって報告してください。
- E2E `scan_includes_library_caches_with_deletable_total_only` の `row.contains("65.")` は、65 MiB のファイルが `format_bytes` で `65.0 MB` 前後と表示される前提です。
- `src/hint/apps.rs` の `typescript` (`TypeScript server cache`)、`gopls`、`Microsoft Edge` (`process_name: Some("Microsoft Edge Helper")`)、`GeoServices` (`skip: true`) の各エントリと、`Config::default()` の存在は実コードで確認済みです。

## 1. 概要

### 1.1 目的

`~/Library/Caches` 直下のアプリ別サブディレクトリをサイズ順に列挙し、既知アプリ (`src/hint/apps.rs` の `BaseDir::Caches` エントリ) のキャッシュだけを対話的に削除できる新 cleaner `library-caches` を追加します。

### 1.2 非目標

- 未知アプリのキャッシュ削除 (表示のみ。削除対象にしません)
- `--yes` での実行 (実行しません。`auto_clean_allowed()` が `false`)
- `--all` フラグ (ありません)
- アプリの自動終了と再起動 (`hint::auto_clean_hint` は使いません)
- `sub_targets()` の実装 (後述の理由で空のままにします)
- `~/Library/Application Support` や `~/Library/Logs` の扱い

### 1.3 既存 `caches` 変種との関係 (名前衝突の決定)

`main.rs` の `CleanTarget::Caches` (CLI 名 `caches`) の実体は、bun / go / pip / node-gyp / npm / yarn / pnpm の 7 つの cleaner を順に実行する複合コマンドです。`~/Library/Caches` 全体を見るものではありません (node-gyp が `~/Library/Caches/node-gyp` を含むだけです)。

新 cleaner の CLI 名は **`library-caches`**、enum 変種名は **`LibraryCaches`** とします。`library-logs` / `LibraryLogs` と対になる命名で、`caches` とは一文字も衝突しません。`caches` は変更しません。

| 項目 | `caches` (既存) | `library-caches` (新規) |
|---|---|---|
| 対象 | 開発ツール 7 種の CLI / 専用ディレクトリ | `~/Library/Caches` 直下のアプリ別ディレクトリ |
| 方式 | 各ツールの CLI を実行 | 一覧表示後、MultiSelect で選んだものを `delete_path` |
| 既定選択 | なし (全部実行) | 全部未選択 |
| `--yes` | `run_auto` の対象 | 対象外 (`auto_clean_allowed()` が `false`、`ManualOnly` にも包まれる) |

### 1.4 用語

- **除外 (excluded)**: 既存 cleaner が触る領域。一覧にも出さず、サイズ計算もしません (二重計上防止)。
- **Deletable**: 既知アプリで、サイズが閾値 (`HintEntry.threshold_bytes`) を超えるもの。削除可能。
- **BelowThreshold**: 既知アプリだが閾値以下。一覧に出すが削除対象外。
- **Unknown**: 既知アプリ表にない。一覧に出すが削除対象外 (表示のみ)。

## 2. ファイル一覧

| 区分 | パス | 責務 |
|---|---|---|
| 新規 | `src/cleaners/library_caches.rs` | 型、除外リスト、走査、分類、`Cleaner` 実装、単体テスト |
| 変更 | `src/cleaners/mod.rs` | `pub mod library_caches;` を追加 |
| 変更 | `src/hint/detector.rs` | `is_running` を `pub(crate)` にする |
| 変更 | `src/hint/mod.rs` | `pub(crate) use detector::is_running;` を 1 行追加 |
| 変更 | `src/main.rs` | `define_cleaners!` の標準エントリを 1 つ追加、単体テスト 1 件 |
| 変更 | `docs/SUPPORTED.md` | 件数 49 -> 50、表の行、第 50 節の追加と stats の 50 -> 51 (英語節と日本語節) |
| 変更 | `docs/HOWTO-USE.md` | 対象表 (英語と日本語) に 1 行ずつ |
| 新規 | `tests/library_caches.rs` | E2E (`--dry-run` と非 TTY のみ) |

`src/interactive.rs` と `src/cleaner.rs` は変更しません。`README.md` は変更しません (`~/Library/Caches/` は既に Data access に列挙済み)。

## 3. 公開する型・関数

### 3.1 既存コードから再利用するもの (HEAD の実シグネチャ)

```rust
// src/hint/mod.rs
pub enum BaseDir { Caches, AppSupport, Logs }          // derive(Debug, Clone, Copy, PartialEq)
pub struct HintEntry {
    pub base_dir: BaseDir,
    pub path_suffixes: &'static [&'static str],
    pub display_name: &'static str,
    pub process_name: Option<&'static str>,
    pub commands: &'static [&'static str],
    pub threshold_bytes: u64,
    pub skip: bool,
    pub quit_app: Option<&'static str>,
    pub relaunch_app: Option<&'static str>,
}
// src/hint/apps.rs
pub static KNOWN_ENTRIES: &[HintEntry];
// src/hint/detector.rs (現状は private。Task 1 で pub(crate) にし、hint/mod.rs で pub(crate) use する)
fn is_running(process_name: &str, runner: &dyn CommandRunner) -> bool   // pgrep -x を実行
// src/format.rs
pub fn dir_size(path: &Path) -> u64          // st_blocks*512 の合計。エラーは無視して 0
pub fn format_bytes(bytes: u64) -> String
// src/trash.rs
pub fn delete_path(path: &Path) -> Result<()>   // 既定はゴミ箱
// src/cleaner.rs
pub struct SkippedEntry { pub path: PathBuf, pub reason: String }
```

`is_running` の使い方: `CacheEntry.process_name` が `Some` の Deletable エントリに対し、`clean()` 内でのみ `crate::hint::is_running(name, self.runner.as_ref())` を呼びます。`detect()` では呼びません (`scan` が並列で `detect` を呼ぶため、pgrep の起動を避ける)。実行中なら削除せず `skipped` に載せ、アプリの終了は行いません。理由: 確認プロンプトを足すと非対話で検証できず、実行中アプリのキャッシュ破壊は避けたいためです。`process_name` が `None` の既知エントリは実行中判定ができないので、判定なしで削除可能として扱います。

### 3.2 新規モジュールの公開 API (`src/cleaners/library_caches.rs`)

```rust
pub const EXCLUDED_NAMES: &[(&str, &str)];     // (ディレクトリ名, 担当 clean ターゲット)
pub const EXCLUDED_PREFIXES: &[(&str, &str)];  // (接頭辞, 担当)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition { Deletable, BelowThreshold, Unknown }

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub name: String,
    pub display_name: String,
    pub size: u64,
    pub disposition: Disposition,
    pub process_name: Option<&'static str>,
    pub running: bool,
}

pub struct LibraryCachesCleaner {
    pub home: PathBuf,
    pub runner: Box<dyn CommandRunner>,
    pub threshold_override: Option<u64>,   // テスト用。None なら HintEntry の閾値
}

pub fn excluded_by(name: &str) -> Option<&'static str>;
pub fn known_entry_for(name: &str) -> Option<&'static HintEntry>;
pub fn classify(known: Option<&HintEntry>, size: u64, threshold_override: Option<u64>) -> Disposition;
pub fn scan_caches(dir: &Path, include_unknown: bool, threshold_override: Option<u64>) -> Vec<CacheEntry>;
pub fn mark_running(entries: &mut [CacheEntry], runner: &dyn CommandRunner);
pub fn render_listing(entries: &[CacheEntry]) -> Vec<String>;
pub fn render_item(entry: &CacheEntry) -> String;
pub fn split_running(selected: Vec<CacheEntry>) -> (Vec<CacheEntry>, Vec<SkippedEntry>);

impl LibraryCachesCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self;
    fn caches_dir(&self) -> PathBuf;                       // home/Library/Caches
    pub(crate) fn scan(&self, include_unknown: bool) -> Vec<CacheEntry>;
    fn delete_entries(&self, entries: &[CacheEntry], reporter: &dyn ProgressReporter) -> CleanResult;
    fn interactive_select(&self, entries: &[CacheEntry]) -> Result<Vec<CacheEntry>>;
}
impl Cleaner for LibraryCachesCleaner { name() == "library-caches"; detect; clean; auto_clean_allowed() == false }
```

`sub_targets()` は実装しません (既定の空 Vec、共通規約 11.1)。理由: TUI (`run_interactive`) は sub_targets が非空だと `sasurahime clean <name> --sub <sub>` を子プロセスで実行しますが、`--sub` を持つのは xcode だけで、library-caches では失敗するためです。未知アプリのサイズは `clean library-caches --dry-run` の一覧でのみ見えます。

`detect()` の戻り値: ディレクトリなしは `NotFound`、Deletable の合計が 0 なら `Clean`、それ以外は `Pruneable(合計)`。Unknown と BelowThreshold は合計に含めません。`detect()` は既知エントリだけ `dir_size` を計算します (`~/Library/Caches` 全体の再帰走査は巨大になりうるため)。

### 3.3 除外リスト (完全な定数と対応表)

実際に各 cleaner が触るパスと `explorer.rs` の `MANAGED_PATTERNS` から洗い出しました。「根拠」が コード とあるものはリポジトリ内のコードに実パスがあり、CLI とあるものは CLI cleaner が使うツール規約上のキャッシュ位置です。

| ディレクトリ名 (~/Library/Caches 直下) | 担当ターゲット | 根拠 |
|---|---|---|
| `Homebrew` | `brew` | コード (brew.rs, MANAGED_PATTERNS) |
| `node-gyp` | `node-gyp` / `caches` | コード (generic.rs) |
| `org.swift.swiftpm` | `spm` | コード (generic.rs) |
| `JetBrains` | `jetbrains` | コード (gradle.rs) |
| `CocoaPods` | `cocoa-pods` | CLI (`pod cache clean --all`) |
| `Yarn` | `yarn` / `caches` | CLI |
| `pip` | `pip` / `caches` | CLI (`pip cache purge`) |
| `go-build` | `go` / `caches` | CLI (`go clean -cache`) |
| `pypoetry` | `poetry` | CLI |
| `deno` | `deno` | CLI |
| 接頭辞 `ms-playwright` (`ms-playwright`, `ms-playwright-go`) | `browsers` | コード (browser.rs, MANAGED_PATTERNS) |
| 接頭辞 `com.apple.` | OS 管理 | システム領域。削除しない |

```rust
pub const EXCLUDED_NAMES: &[(&str, &str)] = &[
    ("Homebrew", "brew"),
    ("node-gyp", "node-gyp"),
    ("org.swift.swiftpm", "spm"),
    ("JetBrains", "jetbrains"),
    ("CocoaPods", "cocoa-pods"),
    ("Yarn", "yarn"),
    ("pip", "pip"),
    ("go-build", "go"),
    ("pypoetry", "poetry"),
    ("deno", "deno"),
];

pub const EXCLUDED_PREFIXES: &[(&str, &str)] = &[
    ("ms-playwright", "browsers"),
    ("com.apple.", "os-managed"),
];
```

比較は ASCII 大文字小文字を無視します (macOS の既定 FS が大文字小文字非区別のため)。

### 3.4 入出力例

| 入力 (名前, サイズ) | 結果 |
|---|---|
| `Homebrew`, 1 GB | 一覧に出ない (除外) |
| `ms-playwright-go`, 1 GB | 一覧に出ない (接頭辞除外) |
| `com.apple.Safari`, 1 GB | 一覧に出ない |
| `typescript`, 65 MB | Deletable (閾値 64 MB 超) |
| `typescript`, 64 MB ちょうど | BelowThreshold (`size > threshold` が条件) |
| `GeoServices`, 1 GB | Unknown (`skip: true` のため既知扱いしない) |
| `SomeUnknownApp`, 10 MB | Unknown (表示のみ) |
| `.DS_Store` | 無視 (ドット始まり) |
| シンボリックリンク | 無視 |
| サイズ 0 | 無視 |

### 3.5 clean() の表示と挙動

1. `~/Library/Caches` がなければ空の `CleanResult`。
2. `scan(true)` を実行し、空なら `[library-caches] nothing to clean`。
3. `mark_running` を実行し、`render_listing` を stdout に出力 (サイズ降順、同サイズは名前順)。
4. `--dry-run`: Deletable ごとに `  would remove: <path>` を出して終了 (削除なし)。
5. Deletable が 0 件なら `[library-caches] no deletable known caches`。
6. stdin が TTY でなければ `not a terminal` を stderr に出して終了 (何も消さない)。
7. `MultiSelect` (既定すべて未選択) で選択。
8. 実行中のものは `skipped` に載せ、それ以外を `chflags -R nouchg` の後 `delete_path`。

## 4. タスク分割

各タスクの末尾でコミットします。コミットは `git add` で対象ファイルを個別指定してください (`git add -A` / `git add .` は禁止)。

### Task 1: 除外リストと分類 (純関数)

#### 1-1. 配線 (テストより先に行う小変更)

`src/hint/detector.rs` の `fn is_running(` を `pub(crate) fn is_running(` に変更します (34 行目付近。本体は変更しません)。

`src/hint/mod.rs` の次の行の直後に 1 行追加します (`pub use detector::collect_hints;` 自体は変更しません)。

```rust
pub use detector::collect_hints;
```
の次の行に
```rust
pub(crate) use detector::is_running;
```
を追加します。

- `mod detector;` / `mod cleaner;` は private のまま変えません (`pub(crate) mod` にしません)。
- `pub use detector::{collect_hints, is_running};` とは書きません。`pub(crate)` の関数は `pub use` で再エクスポートできずコンパイルエラーになります。
- この形は共通規約 7.3 で決めた最終形です。後続の 05 はこれを前提に `crate::hint::is_running` を使い、05 側では `is_running` の可視性を変更しません。
- 再エクスポートは Task 3 まで使われないため、Task 1 と 2 では `unused import: detector::is_running` の警告が出ます (共通規約 1.4 で許容)。新しいモジュールの関数への `dead_code` 警告も同様です。

`src/cleaners/mod.rs` の `pub mod library_logs;` の直前に `pub mod library_caches;` を追加します (アルファベット順)。

#### 1-2. 失敗するテスト

`src/cleaners/library_caches.rs` を新規作成し、**まずテストだけ**を書きます。

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excluded_by_exact_name() {
        assert_eq!(excluded_by("Homebrew"), Some("brew"));
        assert_eq!(excluded_by("homebrew"), Some("brew"));
        assert_eq!(excluded_by("go-build"), Some("go"));
        assert_eq!(excluded_by("node-gyp"), Some("node-gyp"));
    }

    #[test]
    fn excluded_by_prefix() {
        assert_eq!(excluded_by("ms-playwright"), Some("browsers"));
        assert_eq!(excluded_by("ms-playwright-go"), Some("browsers"));
        assert_eq!(excluded_by("com.apple.Safari"), Some("os-managed"));
    }

    #[test]
    fn excluded_by_returns_none_for_ordinary_apps() {
        assert_eq!(excluded_by("typescript"), None);
        assert_eq!(excluded_by("SomeUnknownApp"), None);
        assert_eq!(excluded_by("com.applesauce.Foo"), None);
    }

    #[test]
    fn known_entry_for_finds_caches_entries() {
        let e = known_entry_for("typescript").unwrap();
        assert_eq!(e.display_name, "TypeScript server cache");
        assert!(known_entry_for("Microsoft Edge").is_some());
    }

    #[test]
    fn known_entry_for_ignores_skip_and_other_base_dirs() {
        assert!(known_entry_for("GeoServices").is_none());
        assert!(known_entry_for("Slack/Cache").is_none());
        assert!(known_entry_for("Claude").is_none());
    }

    #[test]
    fn managed_hint_entries_are_all_excluded() {
        for name in ["Homebrew", "ms-playwright", "ms-playwright-go"] {
            assert!(excluded_by(name).is_some(), "{name} must be excluded");
        }
    }

    #[test]
    fn classify_respects_threshold() {
        let e = known_entry_for("typescript");
        let t = e.unwrap().threshold_bytes;
        assert_eq!(classify(e, t + 1, None), Disposition::Deletable);
        assert_eq!(classify(e, t, None), Disposition::BelowThreshold);
        assert_eq!(classify(None, t * 10, None), Disposition::Unknown);
    }

    #[test]
    fn classify_threshold_override() {
        let e = known_entry_for("typescript");
        assert_eq!(classify(e, 2, Some(1)), Disposition::Deletable);
        assert_eq!(classify(e, 1, Some(1)), Disposition::BelowThreshold);
    }
}
```

実行: `cargo test library_caches 2>&1 | tail -20`

期待される失敗: `cannot find function 'excluded_by' in this scope` などのコンパイルエラー。

#### 1-3. 実装

ファイルの先頭 (`#[cfg(test)]` の前) に次を追加します。

```rust
use crate::hint::{apps::KNOWN_ENTRIES, BaseDir, HintEntry};
use std::path::PathBuf;

pub const EXCLUDED_NAMES: &[(&str, &str)] = &[
    ("Homebrew", "brew"),
    ("node-gyp", "node-gyp"),
    ("org.swift.swiftpm", "spm"),
    ("JetBrains", "jetbrains"),
    ("CocoaPods", "cocoa-pods"),
    ("Yarn", "yarn"),
    ("pip", "pip"),
    ("go-build", "go"),
    ("pypoetry", "poetry"),
    ("deno", "deno"),
];

pub const EXCLUDED_PREFIXES: &[(&str, &str)] = &[
    ("ms-playwright", "browsers"),
    ("com.apple.", "os-managed"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Deletable,
    BelowThreshold,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub path: PathBuf,
    pub name: String,
    pub display_name: String,
    pub size: u64,
    pub disposition: Disposition,
    pub process_name: Option<&'static str>,
    pub running: bool,
}

/// Returns the clean target that owns `name`, or None when it is not excluded.
pub fn excluded_by(name: &str) -> Option<&'static str> {
    if let Some((_, target)) = EXCLUDED_NAMES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
    {
        return Some(target);
    }
    let lower = name.to_ascii_lowercase();
    EXCLUDED_PREFIXES
        .iter()
        .find(|(p, _)| lower.starts_with(&p.to_ascii_lowercase()))
        .map(|(_, target)| *target)
}

// `skip: true` entries (GeoServices) are OS-managed and must not become deletable.
pub fn known_entry_for(name: &str) -> Option<&'static HintEntry> {
    KNOWN_ENTRIES.iter().find(|e| {
        e.base_dir == BaseDir::Caches && !e.skip && e.path_suffixes == [name]
    })
}

pub fn classify(
    known: Option<&HintEntry>,
    size: u64,
    threshold_override: Option<u64>,
) -> Disposition {
    match known {
        None => Disposition::Unknown,
        Some(e) => {
            let threshold = threshold_override.unwrap_or(e.threshold_bytes);
            if size > threshold {
                Disposition::Deletable
            } else {
                Disposition::BelowThreshold
            }
        }
    }
}
```

実行: `cargo test library_caches 2>&1 | tail -20`

期待出力: `test result: ok. 8 passed`。

コミット:

```bash
git add src/hint/detector.rs src/hint/mod.rs src/cleaners/mod.rs src/cleaners/library_caches.rs
git commit -m "feat(library-caches): 除外リストと既知アプリ分類を追加"
```

### Task 2: ディレクトリ走査

#### 2-1. 失敗するテスト

`mod tests` の内側の末尾に追加します。`use super::*;` の下に `use std::fs; use tempfile::TempDir;` も追加してください。

```rust
    fn caches(tmp: &TempDir) -> PathBuf {
        let d = tmp.path().join("Library/Caches");
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(dir: &Path, name: &str, bytes: usize) {
        let d = dir.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("data.bin"), vec![1u8; bytes]).unwrap();
    }

    #[test]
    fn scan_sorts_by_size_desc_and_classifies() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        put(&c, "SomeUnknownApp", 40_000);
        put(&c, "gopls", 1_000);
        let r = scan_caches(&c, true, Some(10_000));
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["SomeUnknownApp", "typescript", "gopls"]);
        assert_eq!(r[0].disposition, Disposition::Unknown);
        assert_eq!(r[1].disposition, Disposition::Deletable);
        assert_eq!(r[2].disposition, Disposition::BelowThreshold);
        assert_eq!(r[1].display_name, "TypeScript server cache");
    }

    #[test]
    fn scan_skips_excluded_hidden_empty_and_symlinks() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "Homebrew", 5_000);
        put(&c, "ms-playwright-go", 5_000);
        put(&c, "com.apple.Safari", 5_000);
        put(&c, ".hidden", 5_000);
        fs::create_dir_all(c.join("emptyapp")).unwrap();
        put(&c, "real", 5_000);
        std::os::unix::fs::symlink(c.join("real"), c.join("linked")).unwrap();
        let r = scan_caches(&c, true, None);
        let names: Vec<&str> = r.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["real"]);
    }

    #[test]
    fn scan_without_unknown_returns_only_known() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 5_000);
        put(&c, "SomeUnknownApp", 5_000);
        let r = scan_caches(&c, false, Some(1));
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].name, "typescript");
    }

    #[test]
    fn scan_missing_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        assert!(scan_caches(&tmp.path().join("nope"), true, None).is_empty());
    }

    #[test]
    fn scan_keeps_process_name_of_known_entry() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "Microsoft Edge", 5_000);
        let r = scan_caches(&c, false, Some(1));
        assert_eq!(r[0].process_name, Some("Microsoft Edge Helper"));
        assert!(!r[0].running);
    }
```

実行: `cargo test library_caches 2>&1 | tail -20`

期待される失敗: `cannot find function 'scan_caches'`。

#### 2-2. 実装

ファイル先頭の `use` を次に置き換え、`classify` の後ろに `scan_caches` を追加します。

```rust
use crate::format::dir_size;
use crate::hint::{apps::KNOWN_ENTRIES, BaseDir, HintEntry};
use std::fs;
use std::path::{Path, PathBuf};
```

```rust
/// Lists first-level entries of `dir`, largest first.
/// Sizes are computed only for entries that will be shown: excluded, hidden and
/// symlinked entries are never measured, and unknown apps are measured only when
/// `include_unknown` is true (detect() passes false to avoid walking the whole tree).
pub fn scan_caches(
    dir: &Path,
    include_unknown: bool,
    threshold_override: Option<u64>,
) -> Vec<CacheEntry> {
    let Ok(read) = fs::read_dir(dir) else {
        return vec![];
    };
    let mut entries = Vec::new();
    for item in read.flatten() {
        let path = item.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()).map(str::to_string) else {
            continue;
        };
        if name.starts_with('.') || excluded_by(&name).is_some() {
            continue;
        }
        // An unreadable file type is treated as a symlink: never touch what we cannot classify.
        if item.file_type().map(|t| t.is_symlink()).unwrap_or(true) {
            continue;
        }
        let known = known_entry_for(&name);
        if known.is_none() && !include_unknown {
            continue;
        }
        let size = dir_size(&path);
        if size == 0 {
            continue;
        }
        entries.push(CacheEntry {
            display_name: known
                .map(|e| e.display_name.to_string())
                .unwrap_or_else(|| name.clone()),
            process_name: known.and_then(|e| e.process_name),
            disposition: classify(known, size, threshold_override),
            path,
            name,
            size,
            running: false,
        });
    }
    entries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));
    entries
}
```

注意: `dir_size` は物理ブロック (4 KB 単位) で数えます。テストのサイズ比較は 20,000 / 40,000 / 1,000 バイトのように、ブロック丸め後も順序が変わらない値にしてあります。`gopls` の 1,000 バイトは 1 ブロック (4096) で、閾値 10,000 以下です。

実行: `cargo test library_caches 2>&1 | tail -20`

期待出力: `test result: ok. 13 passed`。

コミット:

```bash
git add src/cleaners/library_caches.rs
git commit -m "feat(library-caches): Library/Caches 直下の走査と分類を追加"
```

### Task 3: 表示、実行中判定、Cleaner の骨格と detect

#### 3-1. 失敗するテスト

`mod tests` の末尾に追加します。`use super::*;` の下に `use crate::cleaner::{Cleaner, ScanStatus}; use crate::test_helpers::MockRunner;` を追加してください。

```rust
    fn entry(name: &str, size: u64, d: Disposition, proc_name: Option<&'static str>) -> CacheEntry {
        CacheEntry {
            path: PathBuf::from("/x").join(name),
            name: name.to_string(),
            display_name: name.to_string(),
            size,
            disposition: d,
            process_name: proc_name,
            running: false,
        }
    }

    #[test]
    fn render_listing_tags_each_disposition() {
        let mut running = entry("A", 3 << 20, Disposition::Deletable, Some("A"));
        running.running = true;
        let v = vec![
            running,
            entry("B", 2 << 20, Disposition::Deletable, None),
            entry("C", 1 << 20, Disposition::BelowThreshold, None),
            entry("D", 512, Disposition::Unknown, None),
        ];
        let lines = render_listing(&v);
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("[running]"), "{}", lines[0]);
        assert!(lines[0].contains("3.0 MB"));
        assert!(!lines[1].contains('['), "{}", lines[1]);
        assert!(lines[2].contains("[below threshold]"));
        assert!(lines[3].contains("[display only]"));
    }

    #[test]
    fn render_item_mentions_running() {
        let mut e = entry("A", 1 << 20, Disposition::Deletable, Some("A"));
        assert!(!render_item(&e).contains("running"));
        e.running = true;
        assert!(render_item(&e).contains("running"));
    }

    #[test]
    fn mark_running_uses_pgrep_for_entries_with_process_name() {
        let mut v = vec![
            entry("A", 10, Disposition::Deletable, Some("A")),
            entry("B", 10, Disposition::Deletable, None),
            entry("C", 10, Disposition::Unknown, Some("C")),
        ];
        // MockRunner: a program registered with with_success exits 0 (= running).
        mark_running(&mut v, &MockRunner::new().with_success("pgrep"));
        assert!(v[0].running);
        assert!(!v[1].running, "no process_name => cannot be judged");
        assert!(!v[2].running, "only deletable entries are checked");
    }

    #[test]
    fn mark_running_false_when_pgrep_unavailable() {
        let mut v = vec![entry("A", 10, Disposition::Deletable, Some("A"))];
        mark_running(&mut v, &MockRunner::new().with_not_found());
        assert!(!v[0].running);
    }

    #[test]
    fn split_running_moves_running_entries_to_skipped() {
        let mut r = entry("A", 10, Disposition::Deletable, Some("A"));
        r.running = true;
        let ok = entry("B", 10, Disposition::Deletable, None);
        let (to_delete, skipped) = split_running(vec![r, ok]);
        assert_eq!(to_delete.len(), 1);
        assert_eq!(to_delete[0].name, "B");
        assert_eq!(skipped.len(), 1);
        assert!(skipped[0].reason.contains("running"));
    }

    fn cleaner(tmp: &TempDir) -> LibraryCachesCleaner {
        LibraryCachesCleaner::new(tmp.path(), Box::new(MockRunner::new().with_not_found()))
    }

    #[test]
    fn name_is_library_caches() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(cleaner(&tmp).name(), "library-caches");
    }

    #[test]
    fn detect_not_found_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        assert!(matches!(cleaner(&tmp).detect().status, ScanStatus::NotFound));
    }

    #[test]
    fn detect_clean_when_only_unknown_apps() {
        let tmp = TempDir::new().unwrap();
        put(&caches(&tmp), "SomeUnknownApp", 50_000);
        assert!(matches!(cleaner(&tmp).detect().status, ScanStatus::Clean));
    }

    #[test]
    fn detect_sums_only_deletable_entries() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        put(&c, "gopls", 1_000);
        put(&c, "SomeUnknownApp", 90_000);
        put(&c, "Homebrew", 90_000);
        let mut cl = cleaner(&tmp);
        cl.threshold_override = Some(10_000);
        let expected = scan_caches(&c, false, Some(10_000))
            .iter()
            .filter(|e| e.disposition == Disposition::Deletable)
            .map(|e| e.size)
            .sum::<u64>();
        assert!(expected > 0);
        assert_eq!(cl.detect().bytes_for_test(), expected);
        assert_eq!(cl.detect().bytes_for_test(), scan_caches(&c, false, Some(10_000))[0].size);
    }

    #[test]
    fn sub_targets_is_empty() {
        let tmp = TempDir::new().unwrap();
        put(&caches(&tmp), "typescript", 20_000);
        assert!(cleaner(&tmp).sub_targets().is_empty());
    }

    #[test]
    fn library_caches_is_never_auto_cleaned() {
        let tmp = TempDir::new().unwrap();
        assert!(!cleaner(&tmp).auto_clean_allowed());
    }
```

最後から 2 つ目の detect テストの最終 assert は「typescript だけが Deletable」であることの確認です (gopls は閾値以下、未知と除外は合計外)。`scan_caches(.., false, ..)[0]` は最大の既知エントリ (typescript) です。

実行: `cargo test library_caches 2>&1 | tail -20`

期待される失敗: `cannot find function 'render_listing'` / `cannot find type 'LibraryCachesCleaner'`。

#### 3-2. 実装

`use` ブロックを次に置き換えます。

```rust
use crate::cleaner::{CleanResult, Cleaner, ScanResult, ScanStatus, SkippedEntry};
use crate::command::CommandRunner;
use crate::format::{dir_size, format_bytes};
use crate::hint::{apps::KNOWN_ENTRIES, BaseDir, HintEntry};
use crate::progress::ProgressReporter;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
```

`scan_caches` の後ろに追加します。`clean` は Task 4 で本実装するため、ここでは dry-run 相当の最小実装にします (Task 4 で置き換えます)。

```rust
pub fn mark_running(entries: &mut [CacheEntry], runner: &dyn CommandRunner) {
    for e in entries.iter_mut() {
        if e.disposition != Disposition::Deletable {
            continue;
        }
        if let Some(p) = e.process_name {
            e.running = crate::hint::is_running(p, runner);
        }
    }
}

pub fn render_listing(entries: &[CacheEntry]) -> Vec<String> {
    entries
        .iter()
        .map(|e| {
            let tag = match (e.disposition, e.running) {
                (Disposition::Deletable, true) => "  [running]",
                (Disposition::Deletable, false) => "",
                (Disposition::BelowThreshold, _) => "  [below threshold]",
                (Disposition::Unknown, _) => "  [display only]",
            };
            format!("  {:<32} {:>10}{}", e.name, format_bytes(e.size), tag)
        })
        .collect()
}

pub fn render_item(entry: &CacheEntry) -> String {
    let running = if entry.running {
        "  [running: will be skipped]"
    } else {
        ""
    };
    format!(
        "{:<32} {:>10}  {}{}",
        entry.name,
        format_bytes(entry.size),
        entry.display_name,
        running
    )
}

pub fn split_running(selected: Vec<CacheEntry>) -> (Vec<CacheEntry>, Vec<SkippedEntry>) {
    let mut to_delete = Vec::new();
    let mut skipped = Vec::new();
    for e in selected {
        if e.running {
            skipped.push(SkippedEntry {
                reason: format!("{} is running: quit it first", e.display_name),
                path: e.path,
            });
        } else {
            to_delete.push(e);
        }
    }
    (to_delete, skipped)
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

pub struct LibraryCachesCleaner {
    pub home: PathBuf,
    pub runner: Box<dyn CommandRunner>,
    pub threshold_override: Option<u64>,
}

impl LibraryCachesCleaner {
    pub fn new(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
        Self {
            home: home.to_path_buf(),
            runner,
            threshold_override: None,
        }
    }

    fn caches_dir(&self) -> PathBuf {
        self.home.join("Library/Caches")
    }

    pub(crate) fn scan(&self, include_unknown: bool) -> Vec<CacheEntry> {
        scan_caches(&self.caches_dir(), include_unknown, self.threshold_override)
    }
}

impl Cleaner for LibraryCachesCleaner {
    fn name(&self) -> &'static str {
        "library-caches"
    }

    fn detect(&self) -> ScanResult {
        let dir = self.caches_dir();
        if !dir.exists() {
            return ScanResult::new(self.name(), ScanStatus::NotFound);
        }
        let total: u64 = self
            .scan(false)
            .iter()
            .filter(|e| e.disposition == Disposition::Deletable)
            .map(|e| e.size)
            .sum();
        let mut r = ScanResult::new(
            self.name(),
            if total > 0 {
                ScanStatus::Pruneable(total)
            } else {
                ScanStatus::Clean
            },
        );
        if crate::context::is_verbose() {
            r = r.with_target(dir.to_string_lossy().to_string());
        }
        r
    }

    fn clean(&self, _dry_run: bool, _reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        Ok(empty_result(self.name()))
    }

    // Removing an app cache is an explicit per-app choice; `--yes` must never pick it.
    fn auto_clean_allowed(&self) -> bool {
        false
    }
}
```

`auto_clean_allowed()` は 04 の Task S2 で `Cleaner` trait に追加済みのメソッドのオーバーライドです (共通規約 2.2)。Task 4 で `clean` を置き換えるときも、このメソッドは残します。

`dir_size` の import は Task 2 で使用済みです (`format::{dir_size, format_bytes}`)。`KNOWN_ENTRIES` `BaseDir` `HintEntry` も使用済みです。`fs` は `scan_caches` で使用済みです。

実行: `cargo test library_caches 2>&1 | tail -20`

期待出力: `test result: ok. 24 passed`。

コミット:

```bash
git add src/cleaners/library_caches.rs
git commit -m "feat(library-caches): 一覧表示・実行中判定・detect を追加"
```

### Task 4: clean (dry-run、非 TTY、対話選択、削除)

#### 4-1. 失敗するテスト

`mod tests` の末尾に追加します。`use` に `crate::progress::DeepSuppressReporter` は不要です (パスで書きます)。

```rust
    #[test]
    fn delete_entries_removes_and_sums_bytes() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        let mut cl = LibraryCachesCleaner::new(
            tmp.path(),
            Box::new(MockRunner::new().with_success("chflags")),
        );
        cl.threshold_override = Some(1);
        let entries = cl.scan(false);
        let size = entries[0].size;
        let r = cl.delete_entries(&entries, &crate::progress::DeepSuppressReporter);
        assert!(!c.join("typescript").exists());
        assert_eq!(r.bytes_freed, size);
        assert!(r.skipped.is_empty());
        assert!(r.uses_trash);
        crate::trash::set_trash_mode(true);
    }

    #[test]
    fn delete_entries_records_failures_as_skipped() {
        crate::trash::set_trash_mode(false);
        let tmp = TempDir::new().unwrap();
        let ghost = entry("ghost", 100, Disposition::Deletable, None);
        let mut ghost = ghost;
        ghost.path = tmp.path().join("does-not-exist");
        let cl = cleaner(&tmp);
        let r = cl.delete_entries(&[ghost], &crate::progress::DeepSuppressReporter);
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.skipped.len(), 1);
        assert_eq!(r.exit_code(), 1);
        crate::trash::set_trash_mode(true);
    }

    #[test]
    fn clean_dry_run_deletes_nothing() {
        let tmp = TempDir::new().unwrap();
        let c = caches(&tmp);
        put(&c, "typescript", 20_000);
        put(&c, "SomeUnknownApp", 20_000);
        let mut cl = cleaner(&tmp);
        cl.threshold_override = Some(1);
        let r = cl.clean(true, &crate::progress::DeepSuppressReporter).unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert!(c.join("typescript/data.bin").exists());
        assert!(c.join("SomeUnknownApp/data.bin").exists());
    }

    #[test]
    fn clean_without_dir_is_ok_and_empty() {
        let tmp = TempDir::new().unwrap();
        let r = cleaner(&tmp)
            .clean(false, &crate::progress::DeepSuppressReporter)
            .unwrap();
        assert_eq!(r.bytes_freed, 0);
        assert_eq!(r.exit_code(), 0);
    }
```

実行: `cargo test library_caches 2>&1 | tail -20`

期待される失敗: `no method named 'delete_entries' found` (コンパイルエラー)。`clean_dry_run_*` は現状の空実装でも通るため、失敗の主因は `delete_entries` の欠如です。

#### 4-2. 実装

`use` ブロックに `use std::io::IsTerminal;` を追加します。`impl LibraryCachesCleaner` に次の 2 メソッドを追加します。

```rust
    fn delete_entries(&self, entries: &[CacheEntry], reporter: &dyn ProgressReporter) -> CleanResult {
        let mut result = empty_result(self.name());
        result.uses_trash = true;
        reporter.progress_init(self.name(), entries.len());
        for (i, e) in entries.iter().enumerate() {
            reporter.progress_tick(&e.path, i + 1, e.size);
            let path_str = e.path.to_string_lossy();
            let _ = self.runner.run("chflags", &["-R", "nouchg", &path_str]);
            match crate::trash::delete_path(&e.path) {
                Ok(()) => {
                    result.bytes_freed += e.size;
                    println!(
                        "[library-caches] removed: {} (freed {})",
                        e.path.display(),
                        format_bytes(e.size)
                    );
                }
                Err(err) => {
                    eprintln!("[library-caches] error removing {}: {err}", e.path.display());
                    result.skipped.push(SkippedEntry {
                        path: e.path.clone(),
                        reason: err.to_string(),
                    });
                }
            }
        }
        reporter.progress_finish();
        result
    }

    fn interactive_select(&self, entries: &[CacheEntry]) -> Result<Vec<CacheEntry>> {
        use dialoguer::MultiSelect;
        let items: Vec<String> = entries.iter().map(render_item).collect();
        let defaults = vec![false; entries.len()];
        println!("\nSelect known app caches to move to Trash:\n");
        let picked = MultiSelect::new()
            .items(&items)
            .defaults(&defaults)
            .interact()?;
        Ok(picked.into_iter().map(|i| entries[i].clone()).collect())
    }
```

`Cleaner for LibraryCachesCleaner` の `clean` を次で置き換えます。

```rust
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult> {
        if !self.caches_dir().exists() {
            return Ok(empty_result(self.name()));
        }
        let mut entries = self.scan(true);
        if entries.is_empty() {
            println!("[library-caches] nothing to clean");
            return Ok(empty_result(self.name()));
        }
        mark_running(&mut entries, self.runner.as_ref());

        println!("\n~/Library/Caches/ by size:\n");
        for line in render_listing(&entries) {
            println!("{line}");
        }

        let deletable: Vec<CacheEntry> = entries
            .into_iter()
            .filter(|e| e.disposition == Disposition::Deletable)
            .collect();

        if dry_run {
            println!(
                "\n[library-caches] dry-run: {} deletable known cache(s)",
                deletable.len()
            );
            for e in &deletable {
                println!("  would remove: {}", e.path.display());
            }
            return Ok(empty_result(self.name()));
        }
        if deletable.is_empty() {
            println!("\n[library-caches] no deletable known caches");
            return Ok(empty_result(self.name()));
        }
        if !std::io::stdin().is_terminal() {
            eprintln!(
                "[library-caches] not a terminal - skipping. \
                 Run `sasurahime clean library-caches` interactively."
            );
            return Ok(empty_result(self.name()));
        }

        let selected = self.interactive_select(&deletable)?;
        if selected.is_empty() {
            println!("[library-caches] nothing selected");
            return Ok(empty_result(self.name()));
        }
        let (to_delete, mut skipped) = split_running(selected);
        let mut result = self.delete_entries(&to_delete, reporter);
        result.skipped.append(&mut skipped);
        println!(
            "[library-caches] total freed: {}",
            format_bytes(result.bytes_freed)
        );
        Ok(result)
    }
```

テストの `delete_entries_records_failures_as_skipped` は `set_trash_mode(false)` で `delete_path` が存在しないパスの `is_dir()` を false と見て `remove_file` を呼び Err になることを利用しています。

実行: `cargo test library_caches 2>&1 | tail -20`

期待出力: `test result: ok. 28 passed`。`cargo fmt` を実行して差分が出た場合は整形結果を採用してください。

コミット:

```bash
git add src/cleaners/library_caches.rs
git commit -m "feat(library-caches): 対話選択と削除を実装"
```

### Task 5: main.rs への登録

前提: 04 の S3 が完了していること (`standard_cleaners()` が存在する)。`src/interactive.rs` は変更しません。

#### 5-1. 失敗するテスト

`src/main.rs` の `mod tests` の末尾 (最後のテスト関数の後ろ、`mod tests` を閉じる `}` の直前) に追加します。04 の S1 で追加された `names_of` を使います。

```rust
    #[test]
    fn library_caches_is_registered_and_distinct_from_caches() {
        let tmp = tempfile::TempDir::new().unwrap();
        let cleaners = all_cleaners(tmp.path(), &config::Config::default());
        assert!(names_of(&cleaners).contains(&"library-caches"));
        let c = cleaners
            .iter()
            .find(|c| c.name() == "library-caches")
            .unwrap();
        assert!(!c.auto_clean_allowed());
        let names: Vec<&str> = SUPPORTED_TARGETS
            .iter()
            .chain(extra_targets())
            .map(|(n, _)| *n)
            .collect();
        assert!(names.contains(&"library-caches"));
        assert!(names.contains(&"caches"));
        assert_eq!(
            CleanTarget::LibraryCaches { dry_run: true }.command_name(),
            "library-caches"
        );
        assert!(CleanTarget::LibraryCaches { dry_run: true }.dry_run());
    }
```

実行: `cargo test --bin sasurahime library_caches_is_registered 2>&1 | tail -20`

期待される失敗: `no variant named 'LibraryCaches'` (コンパイルエラー)。

#### 5-2. main.rs の実装 (1 か所)

`define_cleaners! {` の表で、標準エントリの最後 (02 が追加した `OrphanContainers` エントリの `(...)),` の行の次、単独の `;` だけの行の直前) に次の 2 行を追加します (共通規約 5.0)。

引用 (この 2 行の次に追加):

```rust
    OrphanContainers : "orphan-containers" => "Leftover Containers / Group Containers of uninstalled apps (interactive)";
    (|home, _config| cleaners::orphan_containers::OrphanContainersCleaner::new(home, Box::new(SystemCommandRunner))),
```

追加:

```rust

    LibraryCaches : "library-caches" => "Per-app ~/Library/Caches/ listing; known app caches removable (interactive)";
    (|home, _config| cleaners::library_caches::LibraryCachesCleaner::new(home, Box::new(SystemCommandRunner))),
```

これで `CleanTarget::LibraryCaches { dry_run }`、`command_name()` / `dry_run()` (マクロ生成の `dispatch_command_name` / `dispatch_dry_run` 経由)、`sasurahime targets` の行、`sasurahime clean library-caches [--dry-run]` のディスパッチ、`standard_cleaners()` 経由の `all_cleaners()` (scan と TUI) への登録が揃います。`library-caches` は `AUTO_CLEAN_TARGETS` に無いので `ManualOnly` に包まれ、`--yes` では実行されません。

次のものは書きません (書くと clap の重複 panic や二重登録になります):

- 特殊変種の並び (`;` より後ろ) への `LibraryCaches { ... }`
- `command_name()`、`dry_run()`、`extra_targets()`、`matches!(target, ...)`、`match target { ... }` の腕
- `special_cleaners()` への追加、`AUTO_CLEAN_TARGETS` への追加
- `src/interactive.rs` の除外リストや `is_auto_eligible` のような除外関数

実行:

```bash
cargo test --bin sasurahime library_caches_is_registered 2>&1 | tail -20
cargo test 2>&1 | tail -30
cargo clippy -- -D warnings 2>&1 | tail -20
cargo fmt --check
```

期待出力: 追加テストが ok、`cargo test` はすべて `test result: ok.` (失敗 0。04 の `only_legacy_targets_are_auto_clean_allowed` と `standard_cleaners_follow_the_macro_table_order`、`tests/coverage.rs` を含む)。clippy は警告 0、fmt は出力なし。

コミット:

```bash
git add src/main.rs
git commit -m "feat(library-caches): CLI と scan への登録を追加"
```

### Task 6: E2E テスト (--dry-run と非 TTY のみ)

`tests/library_caches.rs` を新規作成します。実機の `~/Library` は触らず、すべて tempdir を `HOME` にします。`--yes` の E2E は書きません (全 cleaner が実ツールを呼ぶため)。

```rust
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn sasurahime(home: &std::path::Path) -> Command {
    let mut cmd = Command::cargo_bin("sasurahime").unwrap();
    cmd.env("HOME", home).env("PATH", "/usr/bin:/bin");
    cmd
}

fn put(home: &std::path::Path, name: &str, bytes: usize) {
    let d = home.join("Library/Caches").join(name);
    fs::create_dir_all(&d).unwrap();
    fs::write(d.join("data.bin"), vec![1u8; bytes]).unwrap();
}

fn fixture(home: &std::path::Path) {
    put(home, "typescript", 65 * 1024 * 1024);
    put(home, "BigUnknownApp", 4096);
    put(home, "Homebrew", 4096);
    put(home, "com.apple.foo", 4096);
}

#[test]
fn dry_run_lists_deletable_and_hides_excluded() {
    let tmp = TempDir::new().unwrap();
    fixture(tmp.path());
    let out = sasurahime(tmp.path())
        .args(["clean", "library-caches", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("would remove:"), "stdout:\n{stdout}");
    assert!(stdout.contains("typescript"), "stdout:\n{stdout}");
    assert!(stdout.contains("BigUnknownApp"), "stdout:\n{stdout}");
    assert!(stdout.contains("[display only]"), "stdout:\n{stdout}");
    assert!(!stdout.contains("Homebrew"), "stdout:\n{stdout}");
    assert!(!stdout.contains("com.apple.foo"), "stdout:\n{stdout}");
    assert!(
        !stdout
            .lines()
            .any(|l| l.contains("would remove:") && l.contains("BigUnknownApp")),
        "unknown apps must never be removable:\n{stdout}"
    );
    assert!(tmp.path().join("Library/Caches/typescript/data.bin").exists());
}

#[test]
fn non_tty_clean_deletes_nothing() {
    let tmp = TempDir::new().unwrap();
    fixture(tmp.path());
    let out = sasurahime(tmp.path())
        .args(["clean", "library-caches"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(tmp.path().join("Library/Caches/typescript/data.bin").exists());
    assert!(tmp.path().join("Library/Caches/BigUnknownApp/data.bin").exists());
}

#[test]
fn missing_caches_dir_exits_zero() {
    let tmp = TempDir::new().unwrap();
    let out = sasurahime(tmp.path())
        .args(["clean", "library-caches", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn scan_includes_library_caches_with_deletable_total_only() {
    let tmp = TempDir::new().unwrap();
    fixture(tmp.path());
    let out = sasurahime(tmp.path()).arg("scan").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    let row = stdout
        .lines()
        .find(|l| l.contains("library-caches"))
        .unwrap_or_else(|| panic!("no library-caches row:\n{stdout}"));
    assert!(row.contains("pruneable"), "row: {row}");
    assert!(row.contains("65."), "only typescript (about 65 MB) is counted: {row}");
}

#[test]
fn targets_lists_both_caches_and_library_caches() {
    let tmp = TempDir::new().unwrap();
    let out = sasurahime(tmp.path()).arg("targets").output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.lines().any(|l| l.starts_with("caches ")), "{stdout}");
    assert!(stdout.contains("library-caches"), "{stdout}");
}
```

注意:
- `scan` は `hint::collect_hints` の後に `offer_auto_clean` を呼びますが、tempdir の HOME では ~/Library 側のヒントが閾値を超えないので対話は起きません。`typescript` は 65 MB で `collect_hints` の対象 (64 MB 超) になり得るため、`offer_auto_clean` が stdin から読みます。assert_cmd は stdin を閉じるので `None` となり、何も実行されません。
- `row.contains("65.")` はサイズ表記 `65.0 MB` を見ます。ブロック丸めで `65.1 MB` になっても `65.` で一致します。

実行: `cargo test --test library_caches 2>&1 | tail -20`

期待出力: `test result: ok. 5 passed`。

コミット:

```bash
git add tests/library_caches.rs
git commit -m "test(library-caches): E2E を追加 (dry-run と非 TTY のみ)"
```

### Task 7: ドキュメント (docs/SUPPORTED.md と docs/HOWTO-USE.md)

文字は Shift-JIS で表せる範囲にしてください (EM DASH、不等号の合成文字、波線イコールは使わない)。既存の表にある EM DASH は触らず、追記分では ASCII の `-` を使います。

番号と件数は共通規約 5.2 の表のとおりです (03 完了後: 件数 50、新節 `## 50.`、stats は `## 51.`)。着手時点では 02 の結果で件数 49、`## 49. orphan-containers`、`## 50. stats` になっています。

#### 7-1. 英語節

(a) Quick Reference 表の `| \`trash\` |` で始まる行の直前 (02 が追加した `orphan-containers` 行の次) に追加します。

```markdown
| `library-caches` | `~/Library/Caches/` per-app listing; known app caches removable - interactive only, never in `--yes` |
```

(b) 英語節の `## 50. \`sasurahime stats\`` の見出し行の直前 (その前の `---` 行の後ろ) に次の新節を挿入し、stats の見出しを `## 51. \`sasurahime stats\`` に変更します。新節の末尾の `---` が stats 節との区切りになります。

```markdown
## 50. `sasurahime clean library-caches`

**Category:** Sprint 6

**What it removes:** Caches of known apps under `~/Library/Caches/` (the entries
in the built-in app table whose base directory is `Caches`, for example
`typescript`, `gopls`, `electron`, `Microsoft Edge`, `BraveSoftware`).
**Interactive only.** Unknown apps are listed but never removed.

**Difference from `caches`:** `sasurahime clean caches` runs the CLI-based
cleaners (bun, go, pip, node-gyp, npm, yarn, pnpm). `library-caches` looks at
the per-app directories directly under `~/Library/Caches/`.

**How detect works:**
1. Reads the immediate children of `~/Library/Caches/`.
2. Skips dot-entries, symlinks, and directories owned by another cleaner
   (see below). Only known apps are measured.
3. Reports the sum of known caches larger than their threshold (64 MB)
   as pruneable. Unknown apps are not counted.

**How clean works:**
1. Lists every entry by size, with tags `[running]`, `[below threshold]`,
   `[display only]`.
2. **If `--dry-run`:** prints `would remove:` for each deletable entry and stops.
3. If stdin is not a terminal, prints a message and removes nothing.
4. Otherwise opens `dialoguer::MultiSelect` with nothing pre-selected, then
   moves the selected entries to the Trash (`--permanent` deletes them).
5. Entries whose app is running are skipped and reported; the app is never
   quit automatically.

**Excluded (owned by other cleaners, never listed):**

| Directory | Cleaner |
|---|---|
| `Homebrew` | `brew` |
| `ms-playwright*` | `browsers` |
| `node-gyp` | `node-gyp` |
| `org.swift.swiftpm` | `spm` |
| `JetBrains` | `jetbrains` |
| `CocoaPods` | `cocoa-pods` |
| `Yarn` | `yarn` |
| `pip` | `pip` |
| `go-build` | `go` |
| `pypoetry` | `poetry` |
| `deno` | `deno` |
| `com.apple.*` | OS managed |

**Safety:**
- Nothing is selected by default; `--yes` never runs this cleaner.
- Unknown apps are display-only.
- `--dry-run` guarantees zero side effects.

---
```

(c) 英語節の冒頭の件数 `sasurahime provides **49 clean targets**.` を `sasurahime provides **50 clean targets**.` に変更します。

#### 7-2. 日本語節

(a) 日本語のクイックリファレンス表の `| \`trash\` |` で始まる行の直前 (02 が追加した `orphan-containers` 行の次) に追加します。

```markdown
| `library-caches` | `~/Library/Caches/` のアプリ別一覧。既知アプリのキャッシュのみ削除可能 - 対話的のみ、`--yes` では非実行 |
```

(b) 日本語節の `## 50. \`sasurahime stats\`` の見出し行の直前 (その前の `---` 行の後ろ) に次の新節を挿入し、stats の見出しを `## 51. \`sasurahime stats\`` に変更します。

```markdown
## 50. `sasurahime clean library-caches`

**カテゴリ:** Sprint 6

**削除対象:** `~/Library/Caches/` 直下の既知アプリのキャッシュ (内蔵のアプリ表のうち基準ディレクトリが `Caches` のもの。例: `typescript`、`gopls`、`electron`、`Microsoft Edge`、`BraveSoftware`)。**対話的のみ。** 未知のアプリは一覧に表示しますが、削除はしません。

**`caches` との違い:** `sasurahime clean caches` は CLI ベースの cleaner (bun、go、pip、node-gyp、npm、yarn、pnpm) をまとめて実行します。`library-caches` は `~/Library/Caches/` 直下のアプリ別ディレクトリを直接扱います。

**detect の動作:**
1. `~/Library/Caches/` の直下を読み取ります。
2. ドットエントリ、シンボリックリンク、他の cleaner が担当するディレクトリ (下表) を除外します。サイズを測るのは既知アプリだけです。
3. 閾値 (64 MB) を超える既知アプリのキャッシュ合計を pruneable として報告します。未知アプリは合計に含めません。

**clean の動作:**
1. すべてのエントリをサイズ順に表示します。タグは `[running]`、`[below threshold]`、`[display only]` です。
2. `--dry-run`: 削除可能なエントリごとに `would remove:` を表示して終了します。
3. stdin が端末でない場合は、メッセージを表示して何も削除しません。
4. それ以外は `dialoguer::MultiSelect` を (何も選択しない状態で) 表示し、選択したものをゴミ箱へ移動します (`--permanent` では完全削除)。
5. 対象アプリが実行中のエントリはスキップして報告します。アプリの自動終了は行いません。

**除外 (他の cleaner が担当。一覧に出さない):**

| ディレクトリ | 担当 cleaner |
|---|---|
| `Homebrew` | `brew` |
| `ms-playwright*` | `browsers` |
| `node-gyp` | `node-gyp` |
| `org.swift.swiftpm` | `spm` |
| `JetBrains` | `jetbrains` |
| `CocoaPods` | `cocoa-pods` |
| `Yarn` | `yarn` |
| `pip` | `pip` |
| `go-build` | `go` |
| `pypoetry` | `poetry` |
| `deno` | `deno` |
| `com.apple.*` | OS 管理 |

**安全性:**
- 既定では何も選択されません。`--yes` ではこの cleaner は実行されません。
- 未知のアプリは表示のみです。
- `--dry-run` は副作用ゼロを保証します。

---
```

(c) 日本語節の件数 `sasurahime は **49 のクリーンターゲット** を提供しています。` を `sasurahime は **50 のクリーンターゲット** を提供しています。` に変更します。

#### 7-3. docs/HOWTO-USE.md

(a) 英語の対象表 (99 行付近) の `| \`library-logs\`` で始まる行の直前に追加します。

```markdown
| `library-caches` | Per-app `~/Library/Caches/` listing; known app caches removable (interactive only) |
```

(b) 日本語の対象表 (671 行付近) の `| \`library-logs\`` で始まる行の直前に追加します。

```markdown
| `library-caches` | `~/Library/Caches/` のアプリ別一覧。既知アプリのキャッシュのみ削除可能（対話的のみ） |
```

確認:

```bash
grep -n "library-caches" docs/SUPPORTED.md        # 表 2 行 + 見出し 2 行 + 本文
grep -n "^## 50\. \|^## 51\. " docs/SUPPORTED.md   # library-caches 2 行、stats 2 行
grep -n "50 clean targets\|50 のクリーンターゲット" docs/SUPPORTED.md   # 2 行
cargo run -q -- targets | wc -l                   # 50
```

コミット:

```bash
git add docs/SUPPORTED.md docs/HOWTO-USE.md
git commit -m "docs(library-caches): SUPPORTED.md と HOWTO-USE.md に library-caches を追記"
```

## 5. main.rs の変更箇所のまとめ

`src/main.rs` の変更は次の 2 つだけです。

| 箇所 | 内容 |
|---|---|
| `define_cleaners!` の標準エントリの最後 (単独の `;` 行の直前、02 の `OrphanContainers` エントリの次) | `LibraryCaches : "library-caches" => ...; (factory)` の 1 エントリ (Task 5-2) |
| `mod tests` の末尾 | `library_caches_is_registered_and_distinct_from_caches` (Task 5-1) |

特殊変種にせず、マクロの標準エントリで登録する理由: `library-caches` の CLI 引数は `--dry-run` だけで、マクロの `dispatch_clean` が呼ぶ `.clean(dry_run, reporter)` で足ります。04 の実装後は標準エントリ 1 つで、変種、`command_name` / `dry_run`、`SUPPORTED_TARGETS` (`targets`)、ディスパッチ、`standard_cleaners()` 経由の `all_cleaners()` 登録まで揃います。特殊変種にすると `command_name` / `dry_run` / `extra_targets` / `matches!` / `match target` / `special_cleaners` の 6 か所を手で触ることになり、02 と 05 の変更と同じ行で衝突しやすくなります。`--yes` からの除外は登録方法と無関係に `auto_clean_allowed()` で行います。

## 6. エッジケース表

| ケース | 挙動 | 理由 / 検証 |
|---|---|---|
| 対象アプリが実行中 | 一覧に `[running]`、選択しても削除せず `skipped` に理由付きで記録。アプリは終了しない | `split_running` / `mark_running` のテスト。全件実行中なら `bytes_freed == 0` かつ skipped あり、終了コード 1 |
| `process_name` が `None` の既知アプリ | 判定不能のため実行中扱いしない | `mark_running_uses_pgrep...` |
| `pgrep` が使えない | 実行中ではないとみなす (`is_running` が `unwrap_or(false)`) | 既存 hint と同じ挙動 |
| シンボリックリンク | 一覧にも出さず、サイズも測らず、削除もしない | `scan_skips_excluded_hidden_empty_and_symlinks` |
| 読めないディレクトリ | `dir_size` はエラーを無視して読めた分だけ数える。0 ならエントリごと非表示 | `format::dir_size` の仕様 |
| `~/Library/Caches` が読めない / ない | ない場合は `NotFound`、`clean` は空の結果で終了 0。`read_dir` 失敗時は空一覧 | `scan_missing_dir_returns_empty` |
| 巨大ディレクトリでの `detect` | 既知アプリだけ `dir_size` を測り、未知と除外は測らない。`scan` は rayon で cleaner 間並列 | 実測値は固定せず、全体走査をしない設計で対処。`clean` / `--dry-run` は全件を測るので数秒かかることがある |
| 除外リストに該当 | 一覧・合計・削除のいずれにも出ない | `scan_skips_excluded...` と E2E |
| 閾値ちょうど | 超過のみ Deletable (`size > threshold`) | `classify_respects_threshold` |
| `GeoServices` (`skip: true`) | 既知扱いにせず Unknown (表示のみ) | `known_entry_for_ignores_skip...` |
| 空 (サイズ 0) / ドット始まり | 非表示 | 同上 |
| 非 TTY | 削除せず終了 0 | E2E `non_tty_clean_deletes_nothing` |
| `sasurahime --yes` | `auto_clean_allowed()` が `false` なので `run_auto` の対象外 (`ManualOnly` にも包まれる)。TUI (引数なし) では 1 項目として選べ、選ぶと内部で `clean` の MultiSelect が出る | `library_caches_is_never_auto_cleaned`、`library_caches_is_registered_and_distinct_from_caches` |
| 削除失敗 (権限など) | `skipped` に記録して続行 | `delete_entries_records_failures_as_skipped` |
| 同名で大文字小文字違い | 除外判定は大文字小文字を無視 | `excluded_by_exact_name` |

## 7. 注意事項

### 7.1 やりがちな間違い

- `sub_targets()` を実装する。TUI が `clean library-caches --sub ...` を起動して失敗します。実装しないでください。
- `detect()` で `scan(true)` を呼ぶ。`~/Library/Caches` 全体を測るため `scan` が遅くなります。`detect()` は必ず `scan(false)` です。
- `detect()` の合計に Unknown / BelowThreshold を含める。`scan` の合計が過大になります。
- `--yes` から外すために `src/interactive.rs` や `src/main.rs` に除外リストや除外関数を作る、または `AUTO_CLEAN_TARGETS` に足す。除外は `auto_clean_allowed()` の `false` だけで行います (共通規約 2.2)。
- `is_running` を `pub use` で再エクスポートする、または `mod detector` を `pub(crate) mod` にする。共通規約 7.3 の形 (`pub(crate) fn` と `pub(crate) use detector::is_running;`) だけです。
- MultiSelect の `defaults` を `true` にする (`library_logs.rs` からのコピペ事故)。`vec![false; n]` です。
- `library_logs.rs` の `clean_with_opts` / `--all` までコピーする。`library-caches` に `--all` はありません。
- 特殊変種として `;` の後ろの列や `command_name()` などに書く。`define_cleaners!` の標準エントリ 1 つだけです (Task 5-2)。両方に書くと clap が重複で panic します。
- `Caches` 変種や `caches` の説明を変更する。触りません。
- 除外リストを二箇所に書く。定数は `library_caches.rs` の `EXCLUDED_NAMES` / `EXCLUDED_PREFIXES` の 1 か所だけです。
- テストで実機の `~/Library` を触る。すべて `TempDir` を HOME にします。

### 7.2 禁止事項

- 未知アプリの削除経路を作ること (Unknown / BelowThreshold を `delete_entries` に渡さない)。
- アプリの自動終了、`osascript`、`rm -rf` の直接実行。削除は `crate::trash::delete_path` のみです。
- `git add -A` / `git add .`。
- 共通規約書や他 cleaner の設計書にある範囲外ファイルの変更。
- コードコメントに変更履歴やタスク ID を書くこと。コメントは非自明な WHY だけです。

### 7.3 完了条件チェックリスト

- [ ] `src/cleaners/library_caches.rs` が追加され、単体テストが通る
- [ ] `is_running` が `pub(crate) fn` になり、`src/hint/mod.rs` で `pub(crate) use detector::is_running;` されている (他の挙動は不変)
- [ ] `main.rs` の変更がマクロ表の標準エントリ 1 つと単体テスト 1 件だけである
- [ ] `sasurahime targets` に `caches` と `library-caches` が両方出る
- [ ] `--yes` が `library-caches` を実行しない (`library_caches_is_never_auto_cleaned`、`library_caches_is_registered_and_distinct_from_caches`)
- [ ] `tests/library_caches.rs` の E2E 5 件が通る
- [ ] `docs/SUPPORTED.md` の件数 (50)、英語節・日本語節 (`## 50.`、stats は `## 51.`)・表と、`docs/HOWTO-USE.md` の 2 行が更新済み
- [ ] `cargo run -q -- targets | wc -l` が 50
- [ ] `clippy` 警告 0、`fmt` 差分なし
- [ ] 実機の `~/Library` を触るテストがない
- [ ] コミットが日本語の Conventional Commits で、ファイル個別 add

### 7.4 最終確認コマンド

```bash
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo test --test library_caches
cargo build
./target/debug/sasurahime targets | grep -E "^(caches|library-caches) "
HOME=$(mktemp -d) ./target/debug/sasurahime clean library-caches --dry-run
git status --short
```

期待: fmt と clippy が無出力で終わる。`cargo test` は失敗 0。`targets` の出力に 2 行出る。最後の dry-run は終了コード 0 (空の HOME なので何も表示されない)。`git status` に意図しないファイルがない。

## 8. 未決事項

なし。判断事項と理由は次のとおりです。

- CLI 名は `library-caches` (`library-logs` と対称で、`caches` と衝突しない)。
- 実行中アプリは確認ではなく自動スキップ (非対話でテストでき、キャッシュ破壊を避けられる)。
- 閾値は `HintEntry.threshold_bytes` (64 MB) をそのまま使い、テスト用にだけ上書きを許す。
- 除外は一覧からも隠す (二重計上と誤操作の防止)。除外の件数表示はしない (コードを増やさないため)。
- `sub_targets()` は空 (TUI が `--sub` を要求するため。共通規約 11.1)。未知分のサイズは `--dry-run` の一覧で見る。
- `--yes` からの除外は `auto_clean_allowed()` の `false` (共通規約 2.2)。
- 登録はマクロの標準エントリ (5 章)。
- `hint::is_running` の公開は `pub(crate)` と `pub(crate) use` (共通規約 7.3)。05 も同じ形を使う。
