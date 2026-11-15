<details{% if page.lang == 'ja' %} open{% endif %} markdown="1" data-lang="ja" lang="ja">
<summary markdown="0"><strong>🇯🇵 日本語</strong></summary>

<div class="doc-body" markdown="1">

**sasurahime** は macOS 開発者向けキャッシュクリーナーです。既知のキャッシュ
ロケーションをスキャンし、ディスク使用量を報告し、古くなったデータを選択して削除します。

---

## インストール

### Homebrew 経由（推奨）

```bash
brew tap armaniacs/sasurahime
brew install sasurahime
```

### Cargo 経由

```bash
cargo install sasurahime
```

Rust 1.70+ と macOS（arm64 または x86_64）が必要です。Linux/Windows はサポートしていません。

---

## クイックスタート

1. **スキャン — 何がディスクを使っているか確認（削除なし）:**
   ```bash
   sasurahime scan
   ```

2. **削除前にプレビュー:**
   ```bash
   sasurahime clean brew --dry-run
   ```

3. **対象を指定してクリーン:**
   ```bash
   sasurahime clean brew
   ```

以上です。ファイルはデフォルトで macOS のゴミ箱に移動されます — Finder から復元できます。

---

## コマンド

### `sasurahime scan`

すべてのキャッシュターゲットをスキャンし、サマリテーブルを表示して、何も削除せずに終了します。

テーブル表示後、`~/Library/Caches/`・`~/Library/Application Support/`・`~/Library/Logs/` を調べ、起動中アプリ（Edge、VSCode、Slack、Claude、Obsidian 等）に属する大きなキャッシュを検出します。しきい値を超えるエントリが見つかると **Tip** ブロックに手動実行コマンドを表示し、安全に自動終了できるアプリについては「終了→キャッシュ削除→再起動」を代わりに行うか確認します。

```bash
$ sasurahime scan

Category               Size       Status
────────────────────────────────────────
uv (archive)          18.2 GB    pruneable
Homebrew downloads    16.6 GB    stale
bun cache              5.5 GB    clearable
mise / node (old)      3.4 GB    unused
────────────────────────────────────────
Total reclaimable     43.7 GB

────────────────────────────────────────────────────────────
 Tip: The following caches can be freed manually:
────────────────────────────────────────────────────────────
  Claude (desktop)           428 MB  [running — quit first]
    $ rm -rf ~/Library/Application\ Support/Claude/Cache
  VSCode caches              693 MB  [running — quit first]
    $ rm -rf ~/Library/Application\ Support/Code/Cache
    $ rm -rf ~/Library/Application\ Support/Code/CachedExtensionVSIXs
    $ rm -rf ~/Library/Application\ Support/Code/CachedData
  Microsoft Edge             1.8 GB  [running — quit first]
    $ rm -rf ~/Library/Caches/Microsoft\ Edge
────────────────────────────────────────────────────────────

Quit Claude (desktop) and clear cache? (428 MB will be freed, delete ~/Library/Application Support/Claude/Cache) [y/N]
```

`y` と答えると `osascript` でアプリを終了し、キャッシュを削除し、適切であれば再起動します。`n`（または Enter）でスキップします。自動再起動しないアプリ（Slack、Zoom、Claude）は手動で再起動してください。

Tip ブロックはサイズ降順で最大 **5 件** 表示します。64 MB 未満（ログは 1 MB 未満）のエントリは非表示。GeoServices など OS 管理のキャッシュは常に除外されます。

### `sasurahime clean <target>`

単一のキャッシュターゲットをクリーンします。`<target>` を以下のいずれかの名前に置き換えてください。

| ターゲット      | 削除対象                                                             |
|-----------------|----------------------------------------------------------------------|
| `act`           | `~/.cache/act/`（GitHub Actions ランナー）                             |
| `apfs-snapshot` | APFS ローカル Time Machine スナップショット（tmutil deletelocalsnapshot）|
| `app-support`   | `~/Library/Application Support/` 内の既知アプリのキャッシュ(対話的のみ) |
| `brew`          | `brew cleanup -s --prune=all`                                        |
| `browsers`      | 古い Puppeteer Chrome / Playwright（`ms-playwright*`）ビルド           |
| `bun`           | `bun pm cache rm`                                                    |
| `cargo`         | Cargo レジストリキャッシュ + `target/` ディレクトリ                      |
| `cocoa-pods`    | `pod cache clean --all`                                              |
| `colima`        | Colima VM ディスクキャッシュ（`colima prune --all --force`）            |
| `conda`         | `conda clean --all -y`                                               |
| `caches`        | すべての汎用キャッシュ（bun, go, pip, node-gyp, npm, yarn, pnpm）     |
| `deno`          | `deno cache -r`                                                      |
| `device-support`| 古い Xcode DeviceSupport ディレクトリ（最新Nバージョン保持）             |
| `docker`        | `docker system prune -f`                                             |
| `downloads`     | `~/Downloads` のファイル                                              |
| `flutter`       | `dart pub cache clean`                                                |
| `go`            | `go clean -cache`                                                    |
| `gradle`        | Gradle の古いバージョンキャッシュ                                      |
| `huggingface`   | Hugging Face モデルキャッシュ（`hub/`）CLI またはフォールバック          |
| `ios-backup`    | iOS デバイスバックアップ（インタラクティブのみ — --yes では非実行）     |
| `jetbrains`     | JetBrains IDE キャッシュ（古いバージョン）                              |
| `library-caches` | `~/Library/Caches/` のアプリ別一覧。既知アプリのキャッシュのみ削除可能（対話的のみ） |
| `library-logs`  | `~/Library/Logs/` — インタラクティブヒューリスティックスキャン         |
| `logs`          | N 日より古いログファイル（`--keep-days` を参照）                        |
| `maven`         | Maven ローカルリポジトリ（`mvn dependency:purge-local-repository`）     |
| `mise`          | `~/.local/share/mise/installs/` 下の未使用ランタイムバージョン          |
| `node-gyp`      | `~/.cache/node-gyp/` を削除                                          |
| `npm`           | `npm cache clean --force`                                            |
| `ollama`        | Ollama モデルキャッシュ — 対話的選択またはディレクトリ削除                |
| `orphan-containers` | アンインストール済みアプリの `~/Library/Containers` / `~/Library/Group Containers` の残骸（対話的のみ） |
| `orbstack`      | `orb prune`                                                          |
| `pip`           | `pip cache purge`                                                    |
| `pipx`          | `pipx cache purge`                                                   |
| `pnpm`          | `pnpm store prune`                                                   |
| `poetry`        | `poetry cache clear --all`                                           |
| `pre-commit`    | pre-commit フック環境キャッシュ（CLI またはフォールバック）               |
| `rustup`        | 未使用の Rust ツールチェーン                                           |
| `bundle`        | Bundler キャッシュ（`bundle clean`）                                   |
| `dotnet`        | .NET NuGet キャッシュ（`dotnet nuget locals all --clear`）             |
| `gem`           | RubyGems 古い gem バージョン（`gem cleanup`）                           |
| `sbt`           | Scala/sbt ビルドキャッシュ + Ivy キャッシュ                             |
| `terraform`     | Terraform プラグインキャッシュ（`~/.terraform.d/plugin-cache/`）        |
| `tree-sitter`   | tree-sitter パーサーコンパイルキャッシュ                                |
| `uv`            | 古い `simple-vN` インデックスディレクトリ + `uv cache prune --force`    |
| `volta`         | Volta Node.js マネージャキャッシュ（`~/.volta/cache/`）                 |
| `vscode-extensions` | VS Code 拡張機能キャッシュ（`~/.vscode/extensions/`）              |
| `xcode`         | Xcode DerivedData プロジェクトディレクトリ                              |
| `yarn`          | `yarn cache clean`                                                   |

**実行例:**

```bash
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean logs
```

### `sasurahime explore`

OmniDiskSweeper 風のディスク探索コマンドです。よく知られたキャッシュ・データディレクトリをスキャンし、第1レベルのサブディレクトリ（＝アプリ名）ごとに使用量をまとめて、発見したものに対してその場でアクションを取れます。

`scan` と異なり、`explore` は sasurahime がクリーナーを登録していないアプリのフォルダも含め **すべて** カバーします。「ディスクを食っているのは誰か？」を犯人不明の状態から調べるときに最適です。

```bash
$ sasurahime explore --top 5

━━━ Managed by sasurahime ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

┌──────────────────────────────────────┬──────────┬──────────────────────────────┐
│ Path                                 │ Size     │ Clean with                   │
├──────────────────────────────────────┼──────────┼──────────────────────────────┤
│ ~/Library/Caches/Homebrew            │  9.1 GB  │ sasurahime clean brew        │
│ ~/.local/share/mise                  │  5.4 GB  │ sasurahime clean mise        │
│ ~/.cache/uv                          │  1.8 GB  │ sasurahime clean uv          │
└──────────────────────────────────────┴──────────┴──────────────────────────────┘

Select managed entries to clean (space to toggle, enter to confirm):
> [ ] ~/Library/Caches/Homebrew   9.1 GB
  [ ] ~/.local/share/mise         5.4 GB
  [ ] ~/.cache/uv                 1.8 GB

━━━ Not managed by sasurahime ━━━━━━━━━━━━━━━━━━━━━━━━━━

┌──────────────────────────────────────┬──────────┐
│ Path                                 │ Size     │
├──────────────────────────────────────┼──────────┤
│ ~/Library/Application Support/Adobe  │ 22.3 GB  │
│ ~/Library/Application Support/Spotify│  3.2 GB  │
└──────────────────────────────────────┴──────────┘

Select unmanaged entries to inspect:
> [ ] ~/Library/Application Support/Adobe    22.3 GB
  [ ] ~/Library/Application Support/Spotify   3.2 GB
```

出力は2つのセクションに分かれています：

- **Managed（管理済み）** — sasurahime がクリーン方法を知っているパス。エントリを選択すると即座に `sasurahime clean <target>` を実行します。クリーン後はテーブルが更新されたサイズで再表示されます。
- **Not managed（未管理）** — それ以外のすべて。エントリを選択するとフルパスを表示（コピー用）し、Finder でフォルダを開くか確認します。

デフォルトのスキャン対象：`~/Library/Application Support/`・`~/Library/Caches/`・`~/.cache/`・`~/.local/share/`。サイズ 0（空）のディレクトリは表示されません。

TTY（ターミナル）が必要です。

**オプション：**

| フラグ | デフォルト | 説明 |
|--------|------------|------|
| `--top N` | 20 | セクションごとに上位 N 件を表示 |
| `--all` | — | すべて表示（`--top` を上書き） |
| `--dir PATH` | — | デフォルトの代わりにこのルートをスキャン（繰り返し可、デフォルトを完全置換） |

**実行例：**

```bash
# デフォルト：4つのデフォルトルートからセクションごと上位 20 件
sasurahime explore

# セクションごとに上位 5 件のみ表示
sasurahime explore --top 5

# 単一ディレクトリをスキャンしてすべて表示
sasurahime explore --dir ~/Library/Application\ Support --all

# 実際に削除せずにクリーン対象をプレビュー
sasurahime explore --dry-run
```

### `sasurahime`（引数なし）

チェックボックスリスト付きのインタラクティブ TUI を起動します。クリーンするキャッシュターゲットを選択して確認するだけです。

クリーン完了後、`scan` と同じ Tip ブロックと自動終了プロンプトが表示されます。

TTY（ターミナル）が必要です。CI やスクリプトでは代わりに `--yes` を使用してください。

### `sasurahime --yes`

非インタラクティブモード — 確認なしですべての削除可能ターゲットをクリーンします。
削除されるのは無人実行対象のみです。docker、npm、cargo、downloads などは scan に表示されますが、--yes では実行されません。

```bash
# すべてクリーン、確認なし
sasurahime --yes
```

ファイルはデフォルトでゴミ箱に移動されます。`--yes` と `--permanent` を組み合わせると、完全削除の前に確認プロンプトが表示されます。

クリーン後、Tip ブロックと自動終了プロンプトが表示されます（プロンプトにはインタラクティブ入力が必要なので、完全に非インタラクティブなパイプラインでは `< /dev/null` で stdin を抑制してください）。

cron ジョブ、CI パイプライン、メンテナンススクリプトに最適です。

### `sasurahime stats`

削除履歴と統計情報を表示します。

```bash
$ sasurahime stats
╔══════════════════════════════════════╗
║  sasurahime Statistics              ║
╠══════════════════════════════════════╣
║  Total freed:  12.5 GB              ║
║  Runs:         15                   ║
╚══════════════════════════════════════╝

Recent cleanups:
  #  Date                Cleaner        Size
  1  2026-05-26 10:30   uv             500.0 MB
  2  2026-05-25 22:15   brew           1.2 GB
```

履歴は `sasurahime clean` がディスク容量を解放するたびに自動記録され、
`~/.local/share/sasurahime/history.json` に保存されます。

```bash
# 直近 5 件のみ表示
sasurahime stats --last 5
```

履歴ファイルがない場合や破損している場合は、親切なメッセージとともに
終了コード 0 で正常終了します。

---

## 共通フラグ

### `--dry-run`

実際に削除せずに、削除予定の内容をプレビューします。

```bash
sasurahime clean uv --dry-run
sasurahime clean brew --dry-run
sasurahime clean logs --dry-run
```

すべての `clean` サブコマンドでサポートされています。副作用ゼロを保証します。

### `--all`（library-logs のみ）

インタラクティブプロンプトをスキップし、提案されたすべてのエントリを削除します。

```bash
sasurahime clean library-logs --all
```

`--all` なしの場合、`library-logs` は各クリーン可能エントリとその理由（`[large]`、`[stale N days]`）を表示するインタラクティブ選択を開きます。

### `--keep-days`（logs のみ）

ログファイルのデフォルト保持期間を上書きします。

```bash
# 14 日以内のログを保持、それより古いものを削除
sasurahime clean logs --keep-days 14
```

デフォルトは 7 日です（または設定ファイルの値）。

### `--permanent`

ゴミ箱を経由せず、ファイルを完全に削除します。デフォルトではすべてのクリーナーは安全のため削除ファイルを macOS のゴミ箱に送ります。

```bash
# uv キャッシュを完全に削除（ゴミ箱を経由しない）
sasurahime clean uv --permanent
```

`--yes` と組み合わせると、完全削除の前に確認プロンプトが表示されます。

```bash
# 完全一括削除の前に確認を表示
sasurahime --yes --permanent
```

### `--config <path>`

設定ファイルのパスを上書きします。デフォルトでは `~/.config/sasurahime/config.toml` を
読み込みます。このフラグで別のファイルを指定できます。

```bash
# カスタム設定ファイルを使用
sasurahime scan --config /tmp/my-config.toml
```

指定したファイルが存在しない場合は警告が表示され、デフォルト値が使用されます。

---

## 設定ファイル

sasurahime は `~/.config/sasurahime/config.toml` が存在すれば読み込みます。このファイルはオプションです — デフォルト値は日常的な使用に適しています。

### 例：ログ保持期間を 30 日に変更

```toml
[logs]
keep_days = 30
```

### 例：ゴミ箱を完全に無効化

```toml
trash_mode = false
```

### 例：追加ログディレクトリを設定

```toml
[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

### 例：クリーナーをスキャン/TUI から除外

```toml
exclude = ["huggingface", "ollama"]
```

除外されたクリーナーは `scan` 出力とインタラクティブ TUI に表示されません。
`sasurahime clean <target>` で直接指定すれば実行できます。

### 例：カスタムキャッシュディレクトリを追加

```toml
[[custom]]
name = "my-project"
path = "~/work/.cache"
```

カスタムターゲットは `scan` や TUI で通常のクリーナーと同様に表示されます。
`sasurahime clean my-project` で個別にクリーンできます。ディレクトリの
中身のみ削除され（ルートディレクトリは維持）、`uchg` フラグは自動処理されます。

### 例：クリーナーごとのフィルタ

```toml
[cleaner.act]
older_than_days = 30

[cleaner.downloads]
larger_than_mb = 500

[cleaner.logs]
older_than_days = 14
```

フィルタはディレクトリスキャン方式（`DeleteDirs`）のクリーナーに適用されます。
`older_than_days` は指定日数以内に変更されたエントリをスキップし、
`larger_than_mb` は閾値未満のエントリをスキップします。
`logs` が受け付けるのは `older_than_days` だけです。

効かないフィルタは起動時に stderr へ 1 回だけまとめて表示されます。対象は、
コマンドベースのクリーナー（uv, brew, bun 等）、対話型のクリーナー
（orphan-containers, library-caches, app-support）、`logs` の
`larger_than_mb`、存在しないクリーナー名です。例:

    [config] some [cleaner.*] settings have no effect:
      - [docker] older_than_days: this cleaner has no per-cleaner filters

### 完全な設定例

```toml
# ~/.config/sasurahime/config.toml

# グローバル設定
trash_mode = true

# スキャン/TUI から除外するクリーナー
exclude = ["huggingface"]

# カスタムキャッシュターゲット
[[custom]]
name = "my-project"
path = "~/work/.cache"

# クリーナーごとのフィルタ
[cleaner.act]
older_than_days = 30

[cleaner.downloads]
larger_than_mb = 500

# ログ設定
[logs]
keep_days = 30

[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

| フィールド                   | 型               | デフォルト | 説明                             |
|------------------------------|------------------|------------|----------------------------------|
| `trash_mode`                 | boolean          | `true`     | 削除ファイルをデフォルトでゴミ箱へ |
| `exclude`                    | string[]         | `[]`       | スキャン/TUI から除外するクリーナー |
| `[[custom]]`                 | array of tables  | `[]`       | ユーザー定義キャッシュディレクトリ  |
| `custom[].name`              | string           | 必須       | カスタムターゲットの表示名         |
| `custom[].path`              | string           | 必須       | パス（`~` 展開対応）              |
| `[cleaner.<name>]`           | table            | —          | クリーナーごとのフィルタ設定       |
| `cleaner.<name>.older_than_days` | integer      | 未設定     | N 日より古いエントリのみ削除       |
| `cleaner.<name>.larger_than_mb`  | integer      | 未設定     | N MB より大きいエントリのみ削除    |
| `[logs]`                     | table            | —          | ログ保持設定                      |
| `logs.keep_days`             | integer          | `7`        | グローバルログ保持期間            |
| `[[logs.targets]]`           | array of tables  | `[]`       | スキャンする追加ログディレクトリ   |
| `logs.targets[].name`        | string           | 必須       | 表示名                            |
| `logs.targets[].path`        | string           | 必須       | パス（`~` 展開対応）              |
| `logs.targets[].exclude`     | string[]         | `[]`       | 削除しないファイル名              |

---

## 安全性

### ゴミ箱モード（デフォルト）

デフォルトでは、削除されたすべてのファイルは macOS のゴミ箱に送られます。以下を行わない限り、完全に消去されることはありません：

- `--permanent` フラグを渡す
- `~/.config/sasurahime/config.toml` で `trash_mode = false` を設定する

これにより安全網が確保されます — 誤って削除したキャッシュは Finder から復元できます。

### まず `--dry-run`

すべての `clean` サブコマンドは `--dry-run` をサポートしています。削除前に必ずプレビューしましょう。`--dry-run` 使用時の副作用ゼロは CI で検証済みです。

### `.mise.toml` のピン留め

mise ランタイムの削除は、バージョンを削除する前にグローバルの `~/.config/mise/config.toml` と HOME 下のすべての `.mise.toml`（最大深さ 5）をクロスチェックします。これらのファイルのいずれかに固定 (pinned) されているバージョンは決して削除されません。

### macOS 不変フラグ（`uchg`）

ディレクトリに macOS 不変フラグ（`uchg`）が設定されている場合 — パッケージマネージャやシステムキャッシュで一般的 — sasurahime は削除前に自動的に `chflags -R nouchg` を実行します。これはディレクトリを削除するすべてのクリーナーに適用されます。

### Xcode 実行中検出

`sasurahime clean xcode` 実行時に Xcode が実行中の場合は、DerivedData をクリーンする前に確認を求められます。`--yes` モードではプロンプトはバイパスされ、チェックはスキップされます。

### `~/Library/Logs/` の安全性

`library-logs` クリーナーはスキャン結果から常に `CrashReporter` と `DiagnosticReports` を除外します。ドットファイル（`.DS_Store` など）はスキップされます。ヒューリスティックルール（サイズ > 100 MB または最終変更 > 90 日前）に該当しないエントリは非表示になります。

---

## 終了コード

| コード | 意味                                        |
|--------|---------------------------------------------|
| 0      | 成功（または削除するものがない）              |
| 1      | 設定のパースエラー / ターミナルではない       |

---

## 実行例

```bash
# 回収可能な容量の概要を表示
sasurahime scan

# brew キャッシュをクリーン（まずプレビュー）
sasurahime clean brew --dry-run
sasurahime clean brew

# すべての汎用キャッシュを一度にクリーン
sasurahime clean caches

# 古いブラウザビルドを削除
sasurahime clean browsers

# 30 日より古いログをクリーン
sasurahime clean logs --keep-days 30

# ~/Library/Logs/ のインタラクティブヒューリスティックスキャン＋選択
sasurahime clean library-logs

# ~/Library/Logs/ の全候補を一括削除（プロンプトスキップ）
sasurahime clean library-logs --all

# 完全自動化（cron 用）
sasurahime --yes

# インタラクティブ選択
sasurahime
```

---

📖 関連ドキュメント: [対応ターゲット一覧]({% if page.lang == 'ja' %}{{ '/ja/SUPPORTED' | relative_url }}{% else %}{{ '/SUPPORTED' | relative_url }}{% endif %}) · [ターゲット追加方法]({% if page.lang == 'ja' %}{{ '/ja/HOWTO-ADD-target' | relative_url }}{% else %}{{ '/HOWTO-ADD-target' | relative_url }}{% endif %})

</div>

</details>
