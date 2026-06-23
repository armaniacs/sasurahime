# sasurahime

<details open>
<summary><strong>🇺🇸 English</strong></summary>

Scan, select, and remove stale caches from 40+ developer tools — uv, Homebrew, mise, Docker, Cargo, Go, pip, and more — all from a single command on macOS.

sasurahime only removes caches and unused old versions. It never touches runtimes or packages that are currently in use. Every deletion goes to macOS Trash by default, so you can restore anything with a single click.

**What 44 GB looks like:**

```
$ sasurahime scan

Category             Size       Status
──────────────────────────────────────────
uv (archive)         18.2 GB    pruneable
Homebrew downloads   16.6 GB    stale
bun cache             5.5 GB    clearable
mise / node (old)     3.4 GB    unused
Playwright (old)      0.5 GB    stale
──────────────────────────────────────────
Total reclaimable    44.2 GB
```

> **Note on sizes**: The numbers above are from one developer's machine.
> Your environment will differ — these are examples, not guarantees.

## Install

```bash
brew tap armaniacs/sasurahime
brew install sasurahime
```

## Usage

```bash
# --- Step 1: see what's reclaimable (no deletion) ---
sasurahime scan
sasurahime targets               # list all supported clean targets

# --- Step 2: clean what you want ---
sasurahime                       # interactive — pick targets from a menu
sasurahime clean uv              # clean a specific target
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs

# --- Options ---
sasurahime stats                   # show deletion history and statistics
sasurahime history                 # alias for stats
sasurahime clean uv --dry-run      # preview without deleting
sasurahime --yes                   # non-interactive full clean (CI / scripting)
sasurahime clean uv --permanent    # permanently delete (bypass Trash)
sasurahime --version
```

## Safety first

- Every cleaner supports `--dry-run` — nothing is deleted until you confirm.
- **Trash mode is on by default** — removed files go to macOS Trash, so you can
  restore them from Finder. Use `--permanent` to permanently delete.
- mise runtime removal checks both global and per-project `.mise.toml` before deleting.
- macOS immutable flags (`uchg`) are handled automatically.

## Name

*sasurahime* is named after **速佐須良比売（Hayasasurahi-me）**, a goddess who appears in the *Oharae-no-Kotoba* (Great Purification Words) of Shinto tradition.

In Japanese mythology, impurities swept from the world are carried downstream — from river to sea, and finally to the depths of the underworld (*Ne-no-Kuni*). It is Hayasasurahi-me who receives them at the very end and makes them vanish without a trace.

She is, in other words, the ultimate destructor.

The name was chosen from the world of the *Kojiki* and *Nihon Shoki* for its role — purging accumulated impurities (stale caches, orphaned files) completely and finally — and for how cleanly it reads as a command name in a terminal.

## Privacy

sasurahime operates entirely locally on your machine. It does **not**:
- Send any data over the network
- Collect telemetry or usage statistics
- Store or transmit personal information

### Data access

sasurahime reads the following directories to identify cleanable cache data:
- `~/.cache/`, `~/Library/Caches/`, `~/.local/share/`, `~/Library/Application Support/`
- `~/Library/Application Support/MobileSync/Backup/` (iOS backups — only with `sasurahime clean ios-backup`)

The tool reads directory metadata (names, sizes, modification times) to determine what can be cleaned. It does **not** read the contents of your files.

### Deletion history

Every successful clean operation appends a record to
`~/.local/share/sasurahime/history.json` with the cleaner name, freed bytes, and
timestamp. This data stays on your local machine. You can control the maximum
number of entries via `[history].max_entries` in `config.toml` (default: 1000).

## License

Apache-2.0

## Contributing

Issues and PRs are welcome. See [HOWTO-ADD-target.md](docs/HOWTO-ADD-target.md) for details.

For feature proposals, please open an issue first so we can discuss the scope before diving into code.

</details>

<details>
<summary><strong>🇯🇵 日本語</strong></summary>

uv・Homebrew・mise・Docker・Cargo・Go・pip など 40 以上のツールの古いキャッシュを、スキャン・選択・削除。macOS 開発者向けキャッシュクリーナー（Rust 製）。

削除するのはキャッシュや古いバージョンのみです。現在使用中のランタイムやパッケージには一切触れません。また、削除したファイルはデフォルトで macOS のゴミ箱に移動されるため、いつでも Finder から復元できます。

**44 GB が積み上がるとこうなる:**

```
$ sasurahime scan

Category             Size       Status
──────────────────────────────────────────
uv (archive)         18.2 GB    pruneable
Homebrew downloads   16.6 GB    stale
bun cache             5.5 GB    clearable
mise / node (old)     3.4 GB    unused
Playwright (old)      0.5 GB    stale
──────────────────────────────────────────
Total reclaimable    44.2 GB
```

> **サイズについて**: 上記の数値は開発者一人の環境での実測値です。
> 環境によって大きく異なります。目安としてご覧ください。

## インストール

```bash
brew tap armaniacs/sasurahime
brew install sasurahime
```

## 使い方

```bash
# --- Step 1: まず何があるか確認する（削除しない）---
sasurahime scan
sasurahime targets               # 対応している削除対象を一覧表示

# --- Step 2: 削除したいものを片付ける ---
sasurahime                       # インタラクティブモード — メニューから選択
sasurahime clean uv              # 対象を指定して削除
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs

# --- オプション ---
sasurahime stats                   # 削除履歴と統計を表示
sasurahime history                 # stats のエイリアス
sasurahime clean uv --dry-run      # 削除せずに確認だけ（dry-run）
sasurahime --yes                   # 確認なしで全削除（CI・スクリプト向け）
sasurahime clean uv --permanent    # 完全削除（ゴミ箱を経由しない）
sasurahime --version
```

## 安全性について

- すべてのクリーナーは `--dry-run` に対応しています。確認するまで何も削除しません。
- **Trash モードがデフォルトで有効**です。削除したファイルは macOS のゴミ箱に移動されるため、Finder から復元できます。完全に消去したい場合は `--permanent` フラグを使用してください。
- mise のランタイム削除は、グローバル設定とプロジェクト固有の `.mise.toml` を両方チェックしてから実行します。
- macOS の immutable フラグ（`uchg`）は自動的に解除してから削除します。

## 名前の由来

**速佐須良比売（ハヤサスラヒメ）**は、神道の「大祓詞」に登場する祓戸の女神です。

神話の世界では、世界中から集まった罪や穢れは川から海へ、そして海の底へと流れていきます。その最後の最後に待ち受けて、すべてを根の国・底の国へ持ち去り、跡形もなく消し去るのがハヤサスラヒメです。

究極のデストラクタ。

コマンド名としての音の響き、そして「溜まった不要なものを完全に消し去る」という役割 — 『古事記』『日本書紀』の神々の中から、このツールにぴったりの名を選びました。

## プライバシー

sasurahime はあなたのマシン上で完全にローカルに動作します。以下のことは一切行いません:
- ネットワーク経由でデータを送信する
- テレメトリや利用統計を収集する
- 個人情報を保存または送信する

### アクセスするデータ

sasurahime は以下のディレクトリを読み取り、クリーニング可能なキャッシュデータを特定します:
- `~/.cache/`、`~/Library/Caches/`、`~/.local/share/`、`~/Library/Application Support/`
- `~/Library/Application Support/MobileSync/Backup/`（iOS バックアップ — `sasurahime clean ios-backup` でのみアクセス）

ツールはディレクトリのメタデータ（名前、サイズ、更新日時）のみを読み取り、ファイルの内容を読み取ることはありません。

### 削除履歴

クリーン操作が成功するたびに、`~/.local/share/sasurahime/history.json` に
クリーナー名・解放バイト数・タイムスタンプが記録されます。このデータは
ローカルマシンに留まります。`config.toml` の `[history].max_entries` で
最大記録件数を制御できます（デフォルト: 1000）。

## ライセンス

Apache-2.0

## コントリビュート

Issue・PR ともに歓迎します。詳細は [HOWTO-ADD-target.md](docs/HOWTO-ADD-target.md) をご覧ください。

機能の提案はコードを書く前に Issue を立てて相談してください。</details>