# sasurahime — 使い方

**sasurahime** は macOS 向けの開発者キャッシュクリーナーです。
既知のキャッシュディレクトリをスキャンし、使用量を表示し、不要なデータを
選択的に削除します。

---

## インストール

```bash
cargo install sasurahime
```

Rust 1.70 以上、macOS（arm64 / x86_64）が必要です。Linux / Windows には対応していません。

---

## コマンド一覧

### `sasurahime scan`

全てのキャッシュターゲットをスキャンして容量を表示します。削除は行いません。

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
```

### `sasurahime clean <target>`

指定したターゲットのキャッシュを削除します。

| ターゲット        | 削除内容                                                       |
|----------------|--------------------------------------------------------------|
| `act`          | `~/.cache/act/`（GitHub Actions ランナー）                      |
| `brew`         | `brew cleanup -s --prune=all`                                 |
| `browsers`     | 古い Puppeteer Chrome / Playwright ビルド                       |
| `bun`          | `bun pm cache rm`                                             |
| `cargo`        | Cargo registry キャッシュ + `target/` ディレクトリ               |
| `cocoa-pods`   | `pod cache clean --all`                                       |
| `conda`        | `conda clean --all -y`                                        |
| `caches`       | 全ジェネリックキャッシュ（bun, go, pip, node-gyp, npm, yarn, pnpm） |
| `deno`         | `deno cache -r`                                               |
| `docker`       | `docker system prune -f`                                      |
| `downloads`    | `~/Downloads` の古いファイル                                     |
| `go`           | `go clean -cache`                                             |
| `gradle`       | 古い Gradle バージョンキャッシュ                                    |
| `huggingface`  | Hugging Face モデルキャッシュ（CLI 優先、なければ fallback）        |
| `jetbrains`    | JetBrains IDE キャッシュ（旧バージョン）                            |
| `library-logs` | `~/Library/Logs/` — ヒューリスティックスキャン＋対話的選択          |
| `logs`         | 指定日数より古いログファイル（`--keep-days` 参照）                 |
| `mise`         | 未使用のランタイムバージョン（`~/.local/share/mise/installs/` 内） |
| `node-gyp`     | `~/.cache/node-gyp/` ディレクトリを削除                           |
| `npm`          | `npm cache clean --force`                                     |
| `orbstack`     | `orb prune`                                                   |
| `pip`          | `pip cache purge`                                             |
| `pipx`         | `pipx cache purge`                                            |
| `pnpm`         | `pnpm store prune`                                            |
| `poetry`       | `poetry cache clear --all`                                    |
| `pre-commit`   | pre-commit hook 環境キャッシュ（CLI 優先、なければ fallback）      |
| `rustup`       | 未使用の Rust ツールチェーン                                     |
| `spm`          | SwiftPM キャッシュディレクトリ                                    |
| `trash`        | `~/.Trash` の容量表示のみ（削除は Finder で）                    |
| `uv`           | 古い `simple-vN` インデックス + `uv cache prune --force`         |
| `xcode`        | Xcode DerivedData プロジェクトディレクトリ                         |
| `yarn`         | `yarn cache clean`                                            |

**実行例:**

```bash
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean logs
```

### `sasurahime`（引数なし）

インタラクティブ TUI が起動します。`dialoguer::MultiSelect` による
チェックボックスリストから削除するターゲットを選択します。

TTY（ターミナル）が必要です。CI やスクリプトでは `--yes` を使用してください。

### `sasurahime --yes`

非対話モード。確認なしで全ての削除可能なターゲットを掃除します。

```bash
# 全て掃除、確認なし
sasurahime --yes
```

ファイルはデフォルトでゴミ箱（Trash）に移動されます（後述の Trash モード参照）。
`--yes` を `--permanent` と組み合わせた場合、完全削除の前に確認プロンプトが表示されます。

cron ジョブや CI パイプラインに最適です。

---

## 共通フラグ

### `--dry-run`

実際には削除せず、何が削除されるかをプレビューします。

```bash
sasurahime clean uv --dry-run
sasurahime clean brew --dry-run
sasurahime clean logs --dry-run
```

全ての `clean` サブコマンドで使用可能です。副作用は一切ありません。

### `--all`（library-logs のみ）

対話的選択をスキップし、提案されたエントリを全て削除します。

```bash
sasurahime clean library-logs --all
```

`--all` なしの場合は、各エントリに理由（`[large]`、`[stale N days]`）を
表示して選択式で削除します。

### `--keep-days`（logs のみ）

ログファイルの保持期間を上書きします。

```bash
# 14日より新しいログだけ残し、それより古いものを削除
sasurahime clean logs --keep-days 14
```

デフォルトは 7 日です（設定ファイルで変更可能）。

### `--permanent`

ゴミ箱（Trash）を経由せず、ファイルを完全に削除します。
デフォルトでは全てのクリーナーは削除したファイルを macOS のゴミ箱に移動します。

```bash
# uv キャッシュを完全削除（ゴミ箱を経由しない）
sasurahime clean uv --permanent
```

`--yes` と組み合わせると、完全削除の前に確認プロンプトが表示されます。

```bash
# 完全削除前に確認を表示
sasurahime --yes --permanent
```

---

## 設定ファイル

`~/.config/sasurahime/config.toml` が存在すれば読み込みます。
ファイルは必須ではありません。デフォルト値は日常使いに適した設定です。

### 例: ログ保持期間を 30 日に変更

```toml
[logs]
keep_days = 30
```

### 例: ゴミ箱移動を無効化

```toml
trash_mode = false
```

### 例: 追加のログディレクトリを監視

```toml
[[logs.targets]]
name = "my-app"
path = "~/.local/share/my-app/logs"
exclude = ["access.log"]
```

| フィールド        | 型          | デフォルト | 説明                      |
|----------------|------------|--------|-------------------------|
| `trash_mode`   | 真偽値       | `true` | 削除したファイルをゴミ箱に移動する     |
| `keep_days`    | 整数         | `7`    | ログ保持日数                 |
| `targets`      | テーブルの配列  | `[]`   | 追加で監視するログディレクトリ      |
| `targets[].name` | 文字列     | 必須    | 表示名                     |
| `targets[].path` | 文字列     | 必須    | パス（`~` 展開対応）         |
| `targets[].exclude` | 文字列配列 | `[]`   | 絶対に削除しないファイル名       |

---

## 安全機能

### Trash モード（デフォルト）

削除されたファイルはデフォルトで macOS のゴミ箱（Trash）に移動されます。
以下のいずれかに該当する場合を除き、ファイルが完全に消去されることはありません。

- `--permanent` フラグを指定する
- `~/.config/sasurahime/config.toml` で `trash_mode = false` を設定する

誤って削除したキャッシュも Finder からゴミ箱で復元できる安全設計です。

### まず `--dry-run` で確認

全ての `clean` サブコマンドは `--dry-run` に対応しています。
削除前に必ずプレビューする習慣をつけましょう。
`--dry-run` 時は副作用ゼロであることが CI 上で保証されています。

### `.mise.toml` によるピン留め

mise ランタイムの削除時には、グローバルの
`~/.config/mise/config.toml` および HOME 以下の全ての
`.mise.toml`（深さ 5 まで）を横断チェックします。
これらのファイルでピン留めされたバージョンは削除されません。

### macOS イミュータブルフラグ（`uchg`）

ディレクトリに macOS のイミュータブルフラグ（`uchg`）が設定されている
場合 — パッケージマネージャーやシステムキャッシュでよく見られます —
`sasurahime` は削除前に自動で `chflags -R nouchg` を実行します。
これはディレクトリを削除する全てのクリーナーに適用されます。

### Xcode 実行中検出

`xcode` ターゲット実行時に Xcode が起動している場合、
DerivedData 削除前に確認を求めます。`--yes` モードでは
確認はスキップされます。

### `~/Library/Logs/` の安全性

`library-logs` クリーナーは常に `CrashReporter` と `DiagnosticReports`
をスキャン結果から除外します。ドットファイル（`.DS_Store` 等）もスキップ
されます。ヒューリスティックルール（サイズ > 100 MB または
最終更新 > 90 日前）に合致しないエントリは非表示になります。

---

## 終了コード

| コード | 意味                        |
|------|---------------------------|
| 0    | 成功（または削除対象なし）        |
| 1    | 設定ファイルのパースエラー / TTY 不在 |

---

## 使用例

```bash
# 削除可能な容量をざっと確認
sasurahime scan

# brew キャッシュを削除（事前にプレビュー）
sasurahime clean brew --dry-run
sasurahime clean brew

# 全ジェネリックキャッシュを一括削除
sasurahime clean caches

# 古いブラウザビルドを削除
sasurahime clean browsers

# 30日より古いログを削除
sasurahime clean logs --keep-days 30

# ~/Library/Logs/ をヒューリスティックスキャン＋対話的選択
sasurahime clean library-logs

# 提案された ~/Library/Logs/ エントリを一括削除（確認スキップ）
sasurahime clean library-logs --all

# 完全自動化（cron 向け）
sasurahime --yes

# インタラクティブ選択
sasurahime
```
