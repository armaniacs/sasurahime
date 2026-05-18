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

| ターゲット    | 削除内容                                                   |
|------------|----------------------------------------------------------|
| `uv`       | 古い `simple-vN` インデックス + `uv cache prune --force`      |
| `brew`     | `brew cleanup -s --prune=all`                             |
| `mise`     | 未使用のランタイムバージョン（`~/.local/share/mise/installs/` 内） |
| `browsers` | 古い Puppeteer Chrome / Playwright（`ms-playwright*`）のビルド |
| `bun`      | `bun pm cache rm`                                         |
| `go`       | `go clean -cache`                                         |
| `pip`      | `pip cache purge`                                         |
| `node-gyp` | `~/.cache/node-gyp/` ディレクトリを削除                       |
| `npm`      | `npm cache clean --force`                                 |
| `yarn`     | `yarn cache clean`                                        |
| `pnpm`     | `pnpm store prune`                                        |
| `caches`   | 上記全て（bun, go, pip, node-gyp, npm, yarn, pnpm）        |
| `logs`     | 指定日数より古いログファイル（`--keep-days` 参照）          |
| `xcode`    | Xcode DerivedData プロジェクトディレクトリ                   |

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

### `--keep-days`（logs のみ）

ログファイルの保持期間を上書きします。

```bash
# 14日より新しいログだけ残し、それより古いものを削除
sasurahime clean logs --keep-days 14
```

デフォルトは 7 日です（設定ファイルで変更可能）。

---

## 設定ファイル

`~/.config/sasurahime/config.toml` が存在すれば読み込みます。
ファイルは必須ではありません。デフォルト値は日常使いに適した設定です。

### 例: ログ保持期間を 30 日に変更

```toml
[logs]
keep_days = 30
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
| `keep_days`    | 整数         | `7`    | ログ保持日数                 |
| `targets`      | テーブルの配列  | `[]`   | 追加で監視するログディレクトリ      |
| `targets[].name` | 文字列     | 必須    | 表示名                     |
| `targets[].path` | 文字列     | 必須    | パス（`~` 展開対応）         |
| `targets[].exclude` | 文字列配列 | `[]`   | 絶対に削除しないファイル名       |

---

## 安全機能

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
これはディレクトリを削除する全てのクリーナー（mise, generic node-gyp 等）
に適用されます。

### Xcode 実行中検出

`xcode` ターゲット実行時に Xcode が起動している場合、
DerivedData 削除前に確認を求めます。`--yes` モードでは
確認はスキップされます。

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

# 完全自動化（cron 向け）
sasurahime --yes

# インタラクティブ選択
sasurahime
```
