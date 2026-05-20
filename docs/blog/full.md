---
title: "sasurahime: Rust 製 macOS キャッシュクリーナーの哲学と全機能"
emoji: "🗑️"
type: "tech"
topics: ["rust", "macos", "cli", "キャッシュクリーン", "sasurahime"]
published: false
---

## なぜ作ったのか

開発用 Mac のストレージ管理画面を開くと、いつも「その他」がやけに幅を利かせています。

あれの正体はだいたい決まっていて──`uv` がダウンロードした PyPI アーカイブのキャッシュ、Homebrew が取り置いた古い Formula のダウンロード、`mise` が取り残した旧 Node ランタイム、Playwright や Puppeteer が取り込んだブラウザの過去ビルド、`cargo` のレジストリキャッシュ、Docker や Colima のイメージ、Go や Deno や Rustup のビルドキャッシュ、Gradle や Maven や SwiftPM の依存キャッシュ……。

それぞれのツールには個別のクリーンコマンドが存在します。しかし、それらを毎回個別に思い出して実行するのは面倒です。そもそも「何がどれだけあるか」を一覧で見る手段がなく、気づいたときには SSD が 100GB 単位で埋まっている。

この問題を解決するために `sasurahime` を作りました。

## 名前の由来

「掃除」というコンセプトに合う名前を探していて、日本神話の大祓詞に登場する **速佐須良比売（ハヤサスラヒメ）** にたどり着きました。

川から海へ、海から根の国へと流れていった穢れを、最後に受け取って**跡形もなく消し去る**女神です。不要になったキャッシュを完全に消し去る、という役割にぴったりだと思い、そのままプロジェクト名にしました。

ターミナルで打ちやすい文字列であることも決め手のひとつです。

## 設計思想

### detect は絶対に削除しない

すべてのクリーナーが `Cleaner` トレイトを実装しています：

```rust
trait Cleaner {
    fn name(&self) -> &str;
    fn detect(&self) -> ScanResult;
    fn clean(&self, dry_run: bool) -> Result<CleanResult>;
}
```

`detect()` はスキャンのみ行い、絶対にファイルを削除しません。`clean(dry_run: true)` も同様です。この「副作用ゼロの検出」は最も重要な設計ルールです。`scan` サブコマンドで気軽に状況を確認できるのはこのおかげです。

### デフォルトで Trash モード

削除されたファイルはデフォルトで **macOS の Trash（ゴミ箱）** に移動されます。つまり、Finder から復元できます。間違って消してしまっても、ゴミ箱を開ければ戻せるという安心感があります。

完全に削除したい場合のみ `--permanent` フラグを指定します。`--yes` と組み合わせたときは「本当に全部消しますか？」という確認プロンプトが追加で表示されるようになっています。

### 外部コマンドのモック

`uv` や `brew` のような外部コマンドは `CommandRunner` トレイトを介して呼び出します。テスト時にはモックに差し替えられるため、実際のツールがインストールされていない CI 環境でもテストが通ります。

## 対応しているターゲット一覧

現時点で **40 以上**のターゲットが実装されています。一覧は `sasurahime targets` でいつでも確認できます。

主要なものをカテゴリ別に紹介します。

### 言語・パッケージマネージャ

| ターゲット | 対象 |
|---|---|
| uv | `~/.cache/uv/` の古い simple-vN ディレクトリ、`uv cache prune` |
| brew | Homebrew の古いダウンロードとバージョン (`brew cleanup -s --prune=all`) |
| bun, go, pip, npm, yarn, pnpm, pipx, poetry, conda | 各パッケージマネージャのキャッシュ |
| cargo | Cargo registry キャッシュ + `target/` ディレクトリ |
| rustup | 未使用の Rust ツールチェーン |
| deno | Deno キャッシュ |
| gradle, maven | Gradle 旧バージョンキャッシュ、Maven ローカルリポジトリ |
| spm (SwiftPM) | SwiftPM キャッシュ |
| cocoa-pods | CocoaPods キャッシュ |
| flutter | Flutter/Dart pub キャッシュ |

### ランタイム・ツール

| ターゲット | 対象 |
|---|---|
| mise | `~/.local/share/mise/installs/` の未使用ランタイム（設定ファイルとクロスチェック） |
| browsers | Puppeteer / Playwright の古いブラウザビルド（最新を残す） |
| node-gyp | node-gyp ビルドキャッシュ |

### Docker / VM

| ターゲット | 対象 |
|---|---|
| docker | `docker system prune`（イメージ・コンテナ・ビルドキャッシュ） |
| colima | Colima VM ディスクキャッシュ |
| orbstack | Orbstack プルーン |

### IDE・シミュレータ

| ターゲット | 対象 |
|---|---|
| xcode | Xcode DerivedData / アーカイブ |
| device-support | iOS DeviceSupport の古いシンボル（最新 N バージョンを保持） |
| simulator | iOS Simulator キャッシュ (`xcrun simctl delete unavailable`) |
| jetbrains | JetBrains IDE キャッシュ |
| vscode-extensions | VS Code 拡張キャッシュ |
| trash | macOS Trash のサイズ報告 |

### AI/ML・CI

| ターゲット | 対象 |
|---|---|
| huggingface | Hugging Face モデルキャッシュ (`hub/`) |
| ollama | Ollama モデルキャッシュ |
| act | GitHub Actions ローカルランナーキャッシュ |
| pre-commit | pre-commit フック環境キャッシュ |

### ログ・その他

| ターゲット | 対象 |
|---|---|
| logs | `~/.local/share/kilo/log/` の古いログ |
| library-logs | `~/Library/Logs/` の開発者ログ |
| downloads | `~/Downloads` の古いファイル |

各クリーナーは独立したファイルに分かれているため、新しい対象を追加するときは `Cleaner` トレイトを実装した struct をひとつ足すだけで済みます。

## 全コマンドリファレンス

### `sasurahime`

引数なしで実行するとインタラクティブ TUI が起動します。全ターゲットをスキャンしたあと、`dialoguer::MultiSelect` で削除する対象を選択します。

```bash
sasurahime
```

実際の動作はこんな感じです。

```
sasurahime v0.1.5
Scanning uv... [OK]
Scanning brew... [OK]
Scanning mise... [OK]
Scanning browsers... [OK]
Scanning xcode... [OK]
Scanning logs... [OK]

Select caches to clean  [space to toggle, enter to confirm]:
> [ ] uv                   3.6 GB
  [ ] brew                 75.1 MB
  [ ] logs                 43.5 MB
  [ ] act                  201.2 MB
  [ ] huggingface          1.1 GB
  [ ] pre-commit           242.8 MB
  [ ] library-logs         291.5 KB
  [ ] colima               100.3 GB
```

スペースで選択、Enter で確定します。確認プロンプトが表示されたら `y` で削除実行です。

```
Will free approximately 118.6 MB.
Proceed? [y/N]
```

削除中はスピナーと経過表示が出ます。

```
⠈ Cleaning brew...                                                  Freed: 54.5 MB
Cleaning brew... [OK]
⠙ Cleaning logs...                                                  [kilo] Removed: ...
Cleaning logs... [OK]                                                Removed 2 log files

Total freed: 98.0 MB
```

### `sasurahime scan`

削除はせず、スキャン結果だけを一覧表示します。

```bash
sasurahime scan
```

### `sasurahime clean <target>`

特定のターゲットだけを掃除します。

```bash
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean logs
```

### `sasurahime clean <target> --dry-run`

何も削除せずに、どれだけの容量が解放できるかをプレビューします。

```bash
sasurahime clean uv --dry-run
```

### `sasurahime --yes`

すべてのクリーナーを順番に実行します。確認プロンプトは出るので、無人実行には `--yes --permanent` を組み合わせます（その場合も最終確認は表示されます）。

```bash
sasurahime --yes
```

### `sasurahime --permanent`

Trash を経由せず完全に削除します。デフォルトの Trash モードをオーバーライドするためのフラグです。

```bash
sasurahime clean uv --permanent
```

### `sasurahime targets`

対応している全ターゲットの一覧を表示します。

```bash
sasurahime targets
```

### `sasurahime --version`

バージョン番号を表示します。

## 安全性の詳細

### uchg フラグの自動解除

macOS ではファイルに `uchg`（ユーザー immutable）フラグが立っていると、`rm -rf` でも削除できません。sasurahime は削除の直前に `chflags -R nouchg <path>` を実行してから削除します。

### mise のピン留め保護

`~/.config/mise/config.toml` やプロジェクトの `.mise.toml` に書かれているバージョンは削除されません。HOME 以下の最大深さ 5 まで `.mise.toml` を探しに行き、そこで pin されているバージョンを削除対象から除外します。

```toml
# .mise.toml に書かれている node 18 は絶対に消さない
[tools]
node = "18"
```

### Trash モードの設定ファイル

`~/.config/sasurahime/config.toml` に `trash_mode = false` と書けば、`--permanent` をつけなくても常に完全削除モードになります。

## インストール方法

### GitHub Releases（推奨）

プリビルドバイナリをダウンロードして配置するだけです。

```bash
curl -LO https://github.com/armaniacs/sasurahime/releases/download/v0.1.5/sasurahime-aarch64-apple-darwin.tar.gz
tar xzf sasurahime-x86_64-apple-darwin.tar.gz
sudo mv sasurahime /usr/local/bin/
```

### Cargo（Rust 環境がある場合）

```bash
cargo install sasurahime
```

## 実装のハイライト

### バイナリサイズ 872KB

Rust で書かれているとはいえ、`clap`、`dialoguer`、`indicatif`、`comfy-table`、`toml`、`serde`、`trash` といくつかの依存関係があります。リリースプロファイルで以下の最適化を施し、最終的なバイナリサイズは **872KB** になりました。

```toml
[profile.release]
opt-level      = "z"
strip          = true
lto            = true
codegen-units  = 1
panic          = "abort"
```

### トレイトによる拡張性

`Cleaner` トレイトのおかげで、新しいキャッシュ対象を追加するときは 1 ファイル + 数行の登録コードだけで完結します。実際に、最初のリリースから 3 回のアップデートでクリーナー数が倍以上に増えました。

### 外部コマンドの注入

`std::process::Command` を直接呼ばず、`CommandRunner` トレイトを経由することで、テストではモックに差し替えられるようになっています。これにより、`brew` が入っていない CI でも BrewCleaner のテストが動きます。

## さいごに

sasurahime はまだ v0.1.5 のプロジェクトです。「このキャッシュも掃除したい」「こういう機能がほしい」という要望があれば、Issue や PR を歓迎しています。

すぐに試したい方はこちらから：

```bash
cargo install sasurahime
sasurahime
```

スキャン結果を見て「こんなにあったのか」と驚くところから始まると思います。私がそうでした。
