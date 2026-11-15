<details{% if page.lang == 'ja' %} open{% endif %} markdown="1" data-lang="ja" lang="ja">
<summary markdown="0"><strong>🇯🇵 日本語</strong></summary>

<div class="doc-body" markdown="1">

sasurahime に新しいキャッシュクリーンターゲットを追加する手順を説明します。

## 1. まず Issue を開く

コードを書く前に、**Issue を開いて**（または既存の PR にコメントして）提案を議論してください。以下を確認します：

- 対象ツールが使用するキャッシュ
- ディスク上のキャッシュ位置
- クリーン方法（外部 CLI コマンドまたはディレクトリ削除）
- 安全性に関する懸念（不変フラグ、実行中プロセスなど）

```
Title: feat: support <tool> cache cleaning
Body:  - ツール名 / バージョン
       - キャッシュディレクトリのパス
       - クリーン方法
       - 安全性に関する懸念
```

## 2. 実装

クリーナーの複雑さに応じて 3 つのパスがあります。

### パス G：ジェネリッククリーナー（最短、キャッシュターゲットには推奨）

新しいターゲットが外部 CLI ツールへの委譲か、既知ディレクトリの削除のいずれかなら、`src/cleaners/generic.rs` の `CliCleaner` または `PathCleaner` を選びます。合計 **2 箇所** の変更で、新規ファイルは不要です：

```rust
// src/cleaners/generic.rs の impl CliCleaner 内（外部ツールがクリーンする場合）：
pub fn my_tool(runner: Box<dyn CommandRunner>) -> Self {
    Self::new("my-tool", "my_tool", &["cache", "clean"], runner)
}
// または impl PathCleaner 内（固定ディレクトリリスト、年齢/サイズフィルタ可）：
pub fn my_cache(home: &Path, runner: Box<dyn CommandRunner>) -> Self {
    Self::new("my-cache", vec![home.join(".my-cache")], runner)
}
```

```rust
// src/main.rs の define_cleaners! ブロック内：
MyTool : "my-tool" => "MyTool パッケージキャッシュ";
(|_home, _config| cleaners::generic::CliCleaner::my_tool(Box::new(SystemCommandRunner))),
```

CLI 委譲とパス削除の分岐は 2 つの型に閉じているため、共有ディスパッチ列挙型を理解する必要はありません。サイズをファイルシステムから計測したい場合は `CliCleaner::with_detect_dir` ファクトリを、年齢やサイズのフィルタが必要なら `PathCleaner::with_older_than` / `with_larger_than` を使います。

### パス A：標準クリーナー（推奨）

ほとんどのクリーナー（シンプルな `{ dry_run: bool }` 型で単一のファクトリ関数を持つもの）は `src/main.rs` の `define_cleaners!` マクロを使用します。追加には **3 箇所** の変更が必要です：

1. `Cleaner` トレイトを実装した `src/cleaners/<name>.rs` を **作成**
2. `src/cleaners/mod.rs` に `pub mod <name>;` を **登録**
3. `src/main.rs` の `define_cleaners!` 呼び出しに **1 行追加**：

```rust
// src/main.rs の define_cleaners! ブロック内：
MyNewTarget : "my-new-target" => "このクリーナーの説明";
(|home, _config| cleaners::my_new_target::MyNewTargetCleaner::new(home, Box::new(SystemCommandRunner))),
```

これだけです。マクロが自動生成するもの：
- `CleanTarget::MyNewTarget { dry_run: bool }` 列挙子
- `SUPPORTED_TARGETS` エントリ（名前 → 説明）
- `dispatch_clean()` のマッチアーム
- `command_name()` / `dry_run()` ディスパッチヘルパー

標準クリーナーでは `CleanTarget`、`SUPPORTED_TARGETS`、`all_cleaners()`、`main()` のマッチへの手動編集は不要です。

新しい標準ターゲットは自動で scan に出ます。--yes（無人実行）の対象にするには、cleaner の `risk()` を `Risk::Safe` にしてください。`--yes` の対象は `risk()` 経由の `auto_clean_allowed()` から導かれ、Safe 以外の cleaner は自動で `ManualOnly` に包まれます。名前リストの管理は不要です。

### パス B：特別ディスパッチクリーナー

一部のクリーナーはマクロが提供する以上のカスタムディスパッチロジックが必要です：

| 理由 | 例 |
|--------|------|
| `--dry-run` 以外の追加 CLI フラグ | `Logs`（`--keep-days`）、`LibraryLogs`（`--all` / インタラクティブ） |
| 複合（複数クリーナーを実行） | `Caches` |
| クリーン前の事前チェック | `Xcode`（実行中プロセスの検出） |
| 完全に異なる動作 | `Trash`（スキャンのみ、クリーン時に警告） |

これらの場合は、`define_cleaners!` の表に **special 行** を 1 行追加し、`src/main.rs` に **手動ディスパッチ** を追加します：

1. 表に special 行を 1 行追加します。行ヘッダが列挙子、CLI 名、clap 用の説明、`special` マーカー、`targets` 一覧の説明、`dry_run` マーカーを持ち、フィールドは verbatim で書きます：

```rust
// src/main.rs の define_cleaners! 表の special 行セクション内：
MySpecialTarget : "my-target"
    => "このクリーナーの説明" ;
special ;
{
    #[arg(long)]
    dry_run: bool,
} ;
"targets 一覧の説明" ;
dry_run,
```

2. `main()` の special-targets ブロック内にマッチアームを追加します（`Logs`、`Xcode`、`Caches`、`Trash`、`LibraryLogs` の既存ハンドラを参照）：

```rust
// src/main.rs の if target.is_special() { match target { ... } } ブロック内：
CleanTarget::MySpecialTarget { dry_run } => {
    let cleaner = cleaners::my_target::MyTargetCleaner::new(&home, Box::new(SystemCommandRunner));
    run_clean_target("my-target", |dry| cleaner.clean(dry), dry_run)?;
}
```

これだけです。マクロが行から自動生成するもの：
- `CleanTarget::MySpecialTarget { ... }` 列挙子（clap の属性は行ヘッダから）
- `SPECIAL_TARGETS` エントリと `ALL_TARGETS` への合流（`targets` の出力）
- `command_name()` / `dry_run()` / `is_special()` のマッチアーム。`_` 腕なしの完全一致なので、行から CLI 名を削除するとコンパイルが落ちます
- `main()` の `is_special()` ゲートが自動で列挙子を拾います

special クリーナーでは `impl CleanTarget`、`SUPPORTED_TARGETS`、`matches!()` チェックへの手動編集は不要です。run 腕は手書きのままです。特殊な本体は validation・exit パスが genuinely に異なるため、腕の本体だけ手で書きます。

### 両方のパスに共通

4. `tests/<name>.rs` に E2E テストを追加し、必要に応じてユニットテストを追加します。
5. 品質ゲートを通過させます：

```bash
cargo fmt --check && cargo clippy --tests -- -D warnings && cargo test
```

参考として、既存のクリーナー（標準クリーナーは `uv.rs`、特別ディスパッチクリーナーは `library_logs.rs`）を参照してください。


</div>

</details>
