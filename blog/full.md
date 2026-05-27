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

### サブターゲットによる部分削除

Xcode のように複数の種類のキャッシュを持つクリーナーは、内部で `sub_targets()` を実装することで TUI で展開表示されます。選択されたサブターゲットは `sasurahime clean <target> --sub <name>` で個別に実行されます。

## 対応しているターゲット一覧

現時点で **40 以上**のターゲットが実装されています。一覧は `sasurahime targets` でいつでも確認できます。

主要なものをカテゴリ別に紹介します。

### 言語・パッケージマネージャ

| ターゲット | 対象 |
|---|---|
| uv | `~/.cache/uv/` の古い simple-vN ディレクトリ、`uv cache prune` |
| brew | Homebrew の古いダウンロードとバージョン (`brew cleanup -s --prune=all`) |
| bun, go, pip, npm, yarn, pnpm, pipx, poetry, conda | 各パッケージマネージャのキャッシュ |
| cargo | Cargo registry キャッシュ |
| rustup | 未使用の Rust ツールチェーン |
| deno | Deno キャッシュ |
| gradle, maven | Gradle 旧バージョンキャッシュ、Maven ローカルリポジトリ |
| spm (SwiftPM) | SwiftPM キャッシュ |
| cocoa-pods | CocoaPods キャッシュ |
| flutter | Flutter/Dart pub キャッシュ |
| sbt | Scala/sbt ビルドキャッシュ |
| volta | Volta Node.js マネージャキャッシュ |

### ランタイム・ツール

| ターゲット | 対象 |
|---|---|
| mise | `~/.local/share/mise/installs/` の未使用ランタイム（設定ファイルとクロスチェック） |
| browsers | Puppeteer / Playwright の古いブラウザビルド（最新を残す） |
| node-gyp | node-gyp ビルドキャッシュ |
| tree-sitter | tree-sitter パーサーキャッシュ |
| terraform | Terraform プロバイダプラグインキャッシュ |

### Docker / VM

| ターゲット | 対象 |
|---|---|
| docker | `docker system prune`（イメージ・コンテナ・ビルドキャッシュ） |
| colima | Colima VM ディスクキャッシュ |
| orbstack | Orbstack プルーン |

### IDE・シミュレータ

| ターゲット | 対象 |
|---|---|
| xcode | Xcode DerivedData / Archives（部分削除可能） |
| device-support | iOS DeviceSupport の古いシンボル（最新 N バージョンを保持） |
| simulator | iOS Simulator キャッシュ (`xcrun simctl delete unavailable`) |
| jetbrains | JetBrains IDE キャッシュ |
| vscode-extensions | VS Code 拡張キャッシュ |
| ios-backup | iOS デバイスバックアップ（インタラクティブのみ） |
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
| logs | 保持期間を過ぎたログファイル |
| library-logs | `~/Library/Logs/` の開発者ログ |
| downloads | `~/Downloads` の古いファイル（デフォルト 30 日フィルタ） |
| apfs-snapshot | APFS ローカルスナップショット（インタラクティブのみ） |
| git |  Git キャッシュ |
| torrent | トレントキャッシュ |

各クリーナーは独立したファイルに分かれているため、新しい対象を追加するときは `Cleaner` トレイトを実装した struct をひとつ足すだけで済みます。

## 全コマンドリファレンス

### `sasurahime`

引数なしで実行するとインタラクティブ TUI が起動します。全ターゲットをスキャンしたあと、`dialoguer::MultiSelect` で削除する対象を選択します。

```bash
sasurahime
```

実際の動作はこんな感じです。スキャンは **rayon による並列処理**で高速化されています。

```
sasurahime v0.1.27
Scanning... (12/32) [▓▓▓▓▓░░░░░░░░░░░]

Select caches to clean  [space to toggle, enter to confirm]:
> [ ] uv                   3.6 GB
  [ ] brew                 75.1 MB
  [ ] xcode > DerivedData  15.3 GB
  [ ] xcode > Archives     5.2 GB
  [ ] logs                 43.5 MB
  [ ] huggingface          1.1 GB
  [ ] colima               9.3 GB
```

Xcode のように複数カテゴリを持つターゲットはサブ展開され、個別に選択できます。

スペースで選択、Enter で確定します。確認プロンプトが表示されたら `y` で削除実行です。

```
Will free approximately 15.4 GB.
Proceed? [y/N]
```

削除中はスピナーと経過表示が出ます。

```
Cleaning brew... [OK]              Freed: 54.5 MB
Cleaning xcode > DerivedData [OK]  Freed: 15.0 GB

Total freed: 15.1 GB
```

### `sasurahime scan`

削除はせず、スキャン結果だけを一覧表示します。`--verbose` で各クリーナーの監視パスも表示されます。

```bash
sasurahime scan
sasurahime scan --verbose
```

### `sasurahime clean <target>`

特定のターゲットだけを掃除します。Xcode はサブターゲットを指定できます。

```bash
sasurahime clean uv
sasurahime clean brew
sasurahime clean xcode --sub derived-data
sasurahime clean xcode --sub derived-data,archives
```

### `sasurahime clean <target> --dry-run`

何も削除せずに、どれだけの容量が解放できるかをプレビューします。

```bash
sasurahime clean uv --dry-run
```

### `sasurahime --yes`

すべてのクリーナーを確認プロンプトなしで一括実行します。cron や CI での定期実行に最適です。

```bash
sasurahime --yes
```

設定ファイルで `exclude` にクリーナー名を書いておけば、安全な範囲だけ自動掃除できます。

### `sasurahime --yes --permanent`

Trash を経由せず完全に削除します。この組み合わせのときは最終確認プロンプトが表示されます。

```bash
sasurahime --yes --permanent
```

### `sasurahime stats`

削除履歴の累計と直近の実行一覧を表示します。

```bash
$ sasurahime stats
Total freed:  12.5 GB
Runs:         15

Recent cleanups:
  #  Date                Cleaner        Size
  1  2026-05-27 10:30   uv             500.0 MB
  2  2026-05-26 22:15   brew           1.2 GB
```

履歴は `clean` 実行時に自動記録されます。`--dry-run` のときは記録されません。

```bash
# 直近 5 件だけ表示
sasurahime stats --last 5
```

### `sasurahime explore`

OmniDiskSweeper 風のディスク探索コマンドです。`~/Library/Caches/` や `~/.cache/` などを第 1 階層までスキャンし、sasurahime が管理しているもの・していないもの両方を一覧表示します。

```bash
sasurahime explore --top 5
```

### `sasurahime targets`

対応している全ターゲットの一覧を表示します。

```bash
sasurahime targets
```

### `sasurahime --version`

バージョン番号を表示します。

## 設定ファイル

`~/.config/sasurahime/config.toml` に設定を書けば、CLI フラグを毎回指定する必要がなくなります。

```toml
# スキャンから除外するクリーナー（scan / TUI で非表示に）
exclude = ["huggingface", "ollama"]

# 任意のディレクトリをクリーナーとして追加
[[custom]]
name = "my-project"
path = "~/work/.cache"

# クリーナーごとのフィルタ（DeleteDirs 方式のみ有効）
[cleaner.act]
older_than_days = 30

[cleaner.colima]
larger_than_mb = 500

# ログ保持日数
[logs]
keep_days = 14
```

### `--config <path>` フラグ

デフォルトの `~/.config/sasurahime/config.toml` とは別の設定ファイルを読み込みたいときに使います。

```bash
sasurahime scan --config /tmp/my-config.toml
```

## 安全性の詳細

### uchg フラグの自動解除

macOS ではファイルに `uchg`（ユーザー immutable）フラグが立っていると、`rm -rf` でも削除できません。sasurahime は削除の直前に `chflags -R nouchg <path>` を実行してから削除します。カスタムパスクリーナー (`[[custom]]`) にも適用されます。

### 環境変数による任意パス削除の防止

`TF_PLUGIN_CACHE_DIR`（terraform）や `PUB_CACHE`（flutter）のような環境変数で削除パスを指定できるクリーナーは、`is_safe_delete_target()` でパスの安全性を検証します。`/etc` や `/System` などのシステムパスを指している場合は警告を出してデフォルトパスにフォールバックします。

### mise のピン留め保護

`~/.config/mise/config.toml` やプロジェクトの `.mise.toml` に書かれているバージョンは削除されません。HOME 以下の最大深さ 5 まで `.mise.toml` を探しに行き、そこで pin されているバージョンを削除対象から除外します。

```toml
# .mise.toml に書かれている node 18 は絶対に消さない
[tools]
node = "18"
```

### エラーハンドリング

権限エラー (`EPERM`) やファイルロック (`EBUSY`) が発生した場合、該当ファイルをスキップして処理を継続します。失敗したファイルは削除後に `N file(s) skipped: /path: Permission denied` と表示されます。

## インストール

```bash
cargo install sasurahime
```

Rust 1.70+ と macOS (arm64 / x86_64) が必要です。

## 実装のハイライト

### バイナリサイズ 872KB

Rust で書かれているとはいえ、`clap`、`dialoguer`、`indicatif`、`comfy-table`、`toml`、`serde`、`trash`、`rayon`、`chrono` といくつかの依存関係があります。リリースプロファイルで以下の最適化を施し、最終的なバイナリサイズは **872KB** になりました。

```toml
[profile.release]
opt-level      = "z"
strip          = true
lto            = true
codegen-units  = 1
panic          = "abort"
```

### 並列スキャン

`scan` と `--yes` モードのスキャンは `rayon` による並列処理で実行されます。スキャン時間は最も遅いクリーナーに依存するため、逐次実行より大幅に高速です。

### トレイトによる拡張性

`Cleaner` トレイトのおかげで、新しいキャッシュ対象を追加するときは 1 ファイル + 数行の登録コードだけで完結します。実際に、最初のリリースから 10 日間でクリーナー数が 2 倍以上に増えました。

### 外部コマンドの注入

`std::process::Command` を直接呼ばず、`CommandRunner` トレイトを経由することで、テストではモックに差し替えられるようになっています。これにより、`brew` が入っていない CI でも BrewCleaner のテストが動きます。

### テスト

**442 tests, 0 failures**（288 unit + 154 integration/E2E、24 テストファイル）

全クリーナーに `detect()` + `clean()` + `dry_run` のテストがあります。設定ファイル、削除履歴、並列スキャン、TUI 選択ロジックといった横断的機能も単体テストでカバーされています。

## さいごに

sasurahime は v0.1.27 のプロジェクトです。ここまでの機能で日常的なキャッシュ掃除には十分対応できるようになりましたが、追加のリクエストがあれば Issue や PR を歓迎しています。

すぐに試したい方はこちらから：

```bash
cargo install sasurahime
sasurahime
```

スキャン結果を見て「こんなにあったのか」と驚くところから始まると思います。私がそうでした。
