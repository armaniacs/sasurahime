# sasurahime

<details open>
<summary><strong>🇺🇸 English</strong></summary>

Scan, select, and remove stale caches from 50+ developer tools including uv, Homebrew, mise, Docker, Cargo, Go, and pip, all from a single command on macOS.

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
docker images        12.0 GB    unavailable
──────────────────────────────────────────
Total reclaimable    44.2 GB
```

> **Note on sizes**: The numbers above are from one developer's machine.
> Your environment will differ ― these are examples, not guarantees.
>
> `unavailable` means the target exists but could not be measured
> (permission denied or the tool's command failed). Those rows are
> excluded from the total instead of being counted as 0 B.

## Install

With cargo (from this checkout):

```bash
cargo install --path .
# Update after pulling new code:
cargo install --path . --force
```

`--force` is required to replace an existing binary. The binary lands in
`~/.cargo/bin/sasurahime`; if a Homebrew copy is also installed, the first
one on `PATH` wins (`which sasurahime`), so remove one or adjust your `PATH`.

With Homebrew:

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
sasurahime                       # interactive ― pick targets from a menu
sasurahime clean uv              # clean a specific target
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs
sasurahime clean orphan-containers   # leftover app containers (interactive)
sasurahime clean library-caches      # per-app Library/Caches (interactive)
sasurahime clean vm-swap             # /private/var/vm swap size + uptime (report only)
sasurahime clean project-artifacts   # stale node_modules / target / .build with manifests
sasurahime auto                      # unattended policy apply ([policy] in config.toml)
sasurahime clean apfs-snapshot --thin 10GB   # thin local snapshots toward 10GB free (tmutil)

# --- Options ---
sasurahime stats                   # show deletion history and statistics
sasurahime history                 # alias for stats
sasurahime clean uv --dry-run      # preview without deleting
sasurahime --yes                   # non-interactive clean of the unattended target set (CI / scripting)
sasurahime clean uv --permanent    # permanently delete (bypass Trash)
sasurahime --version
```

## Unattended policy

Write a `[policy]` section in `~/.config/sasurahime/config.toml`, then run
`sasurahime auto` (or `sasurahime auto --dry-run` first):

```toml
[policy]
older_than_days = 30   # only entries older than 30 days
larger_than_mb = 100   # only dirs of at least 100 MB
include_manual = false # Moderate targets need an explicit opt-in
```

`auto` cleans only what the policy allows. `Risky` targets (ios-backup,
apfs-snapshot, ...) never run unattended; skipped targets are listed with the
explicit `clean <target>` guidance. Results land in `history.json`.

To run it periodically with launchd, save this as
`~/Library/LaunchAgents/app.sasurahime.auto.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>app.sasurahime.auto</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/env</string>
    <string>/opt/homebrew/bin/sasurahime</string>
    <string>auto</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict><key>Hour</key><integer>10</integer><key>Minute</key><integer>0</integer></dict>
</dict>
</plist>
```

Then `launchctl load ~/Library/LaunchAgents/app.sasurahime.auto.plist`.
Adjust the binary path (`which sasurahime`; `~/.cargo/bin/sasurahime` when
installed with cargo); no resident daemon is required.

## Safety first

- Every cleaner supports `--dry-run` ― nothing is deleted until you confirm.
- **Trash mode is on by default** ― removed files go to macOS Trash, so you can
  restore them from Finder. Use `--permanent` to permanently delete.
- Trash runs show two axes: `Logical freed: X / Physical freed: 0 B (in Trash)`,
  plus a restore pointer (`open ~/.Trash`). `--permanent` runs keep a minimal
  audit trail (`Permanently deleted: no Trash copy was kept.`) without
  `--verbose`, and `history.json`/`stats` record the trash vs delete method.
- `clean trash` is report-only: it shows the Trash size and how to empty it in
  Finder; sasurahime never empties the Trash itself.
- `clean apfs-snapshot --thin 10GB` thins local Time Machine snapshots toward
  a free-space target by running `tmutil thinlocalsnapshots / 10000000000 2`
  (decimal bytes, as tmutil expects). Urgency defaults to 2; `--urgency 4`
  interrupts running backups and needs an extra confirmation. Thinning never
  calls sudo: if it needs elevation, macOS reports the error and sasurahime
  prints how to re-run. Freed space is reported as the before/after free-space
  delta; purgeable space is updated lazily by macOS.
- `clean project-artifacts` detects stale `node_modules` + `package.json`,
  `target` + `Cargo.toml` and `.build` + `Package.swift` pairs under the
  configured roots (default `~/Playground`, depth 5, `[project-artifacts]` in
  config.toml). Projects updated within 7 days are excluded. **Protection is
  mtime only: uncommitted changes are NOT checked.** Review `git status`
  before cleaning.
- `clean all` is an explicit full clean: `Safe` targets run without extra
  flags; `Moderate` / `Risky` / `Manual` targets need `--include-manual` and
  are listed as skipped with their classification until then. Global
  `--older-than <days>` / `--larger-than <MB>` filters apply to dir-based
  cleaners during `clean all` (the stricter of the global and per-cleaner
  value wins), and out-of-condition entries are shown with a skip reason.
  Command-delegating cleaners (brew, docker, ...) use their own tool
  semantics; global filters cannot restrict them, so `[cleaner.*]` settings
  for those names keep the "no effect" warning. `--yes` is unaffected: it
  still runs only `Safe` targets in the TUI-driven path.
- `scan` keeps a persistent cache (`~/.local/share/sasurahime/scan-cache.json`)
  keyed by directory mtime, so unchanged directories skip the expensive walk.
  Invalidation is conservative: any mtime mismatch, hourly expiry, filesystem
  error or corrupt cache file falls back to a full walk. Deletion paths never
  consult the cache. Use `--no-cache` to always walk.
- mise runtime removal checks both global and per-project `.mise.toml` before deleting.
- macOS immutable flags (`uchg`) are handled automatically.
- `--yes` cleans only `Safe` targets. Targets classified `Moderate` / `Risky` /
  `Manual` are listed as skipped with their classification and require an
  explicit choice (the picker or `clean <target>`).

### Risk classification

One classification per target decides `--yes` behavior, the TUI initial
selection and the scan display (`scan`, `scan --json`). New targets default to
`Moderate`, so an unclassified target never runs unattended.

| Risk | Meaning | Behavior under `--yes` | Examples |
|------|---------|------------------------|----------|
| `Safe` | Regenerable developer caches | cleaned | `uv`, `brew`, `mise`, `browsers`, `xcode`, `logs`, `act`, `huggingface`, `pre-commit`, `library-logs`, `ollama`, `device-support` |
| `Moderate` | Large or partially user-visible data | skipped (explicit choice) | `docker`, `cargo`, `npm`, `bun`, `go`, `pip`, `rustup`, `simulator`, ... |
| `Risky` | Destructive or irreversible | never | `ios-backup`, `apfs-snapshot`, `colima` |
| `Manual` | User data | never | `downloads`, `app-support`, `library-caches`, `orphan-containers`, `project-artifacts` |

## Name

*sasurahime* is named after **速佐須良比売（Hayasasurahi-me）**, a goddess who appears in the *Oharae-no-Kotoba* (Great Purification Words) of Shinto tradition.

In Japanese mythology, impurities swept from the world are carried downstream ― from river to sea, and finally to the depths of the underworld (*Ne-no-Kuni*). It is Hayasasurahi-me who receives them at the very end and makes them vanish without a trace.

She is, in other words, the ultimate destructor.

The name was chosen from the world of the *Kojiki* and *Nihon Shoki* for its role ― purging accumulated impurities (stale caches, orphaned files) completely and finally ― and for how cleanly it reads as a command name in a terminal.

## Privacy

sasurahime operates entirely locally on your machine. It does **not**:
- Send any data over the network
- Collect telemetry or usage statistics
- Store or transmit personal information

### Data access

sasurahime reads the following directories to identify cleanable cache data:
- `~/.cache/`, `~/Library/Caches/`, `~/.local/share/`, `~/Library/Application Support/`
- `~/Library/Application Support/MobileSync/Backup/` (iOS backups ― only with `sasurahime clean ios-backup`)
- `~/Library/Containers/com.docker.docker/Data/` and `~/.colima/` (size of the virtual disk image files only, shown by `sasurahime scan`)
- `~/Library/Containers/` and `~/Library/Group Containers/` (folder names and sizes only; `sasurahime clean orphan-containers`)

The tool reads file and directory metadata (names, sizes, modification times) to determine what can be cleaned. It reads the contents of only the following files, and only to decide what can be cleaned:

- mise configuration files (`~/.config/mise/config.toml` and every `.mise.toml` within 5 directory levels of your home directory), to find runtime versions that are still in use so that they are not deleted
- the `Info.plist` of installed apps (`.app` bundles in `/Applications`, `~/Applications` and `/System/Applications`, and one folder level below them); only the bundle ID and the app name are taken from it, to tell leftovers of uninstalled apps from the data of installed apps

Values read from these files are not written to logs or to the deletion history. sasurahime also reads its own configuration file (`~/.config/sasurahime/config.toml`, or the file given with `--config`) and its deletion history (`~/.local/share/sasurahime/history.json`). It does **not** read the contents of any other file.

### Full Disk Access (FDA)

Some protected paths (`~/Library/Mail`, `~/Library/Application Support/MobileSync/Backup/`, `~/.Trash`) need Full Disk Access to be listed. sasurahime `doctor` probes them with `read_dir` only (never their contents) and shows which ones could not be read, with granting instructions. `scan` notes unreadable protected paths so the reported total is not silently too small. Paths that do not exist are never reported. sasurahime never changes any configuration on its own.

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

uv・Homebrew・mise・Docker・Cargo・Go・pip など 50 以上のツールの古いキャッシュを、スキャン・選択・削除。macOS 開発者向けキャッシュクリーナー（Rust 製）。

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
docker images        12.0 GB    unavailable
──────────────────────────────────────────
Total reclaimable    44.2 GB
```

> **サイズについて**: 上記の数値は開発者一人の環境での実測値です。
> 環境によって大きく異なります。目安としてご覧ください。
>
> `unavailable` は対象が存在するものの計測できなかった行です
> （権限不足、ツールのコマンド失敗など）。0 B として合計に加算されず、
> 合計から除外されます。

## インストール

このリポジトリから cargo で入れる場合:

```bash
cargo install --path .
# 新しいコードを取り込んだあとの更新:
cargo install --path . --force
```

既存のバイナリを置き換えるには `--force` が必要です。バイナリは
`~/.cargo/bin/sasurahime` に入ります。Homebrew 版も入れている場合は
`PATH` の先頭にある方が優先されます（`which sasurahime` で確認）。

Homebrew で入れる場合:

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
sasurahime                       # インタラクティブモード ― メニューから選択
sasurahime clean uv              # 対象を指定して削除
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs
sasurahime clean orphan-containers   # アンインストール済みアプリの残骸（対話的）
sasurahime clean library-caches      # アプリ別 Library/Caches（対話的）
sasurahime clean vm-swap             # /private/var/vm のスワップサイズと稼働日数（表示のみ）
sasurahime clean project-artifacts   # マニフェスト付きの古い node_modules / target / .build
sasurahime auto                      # ポリシーに基づく無人実行（config.toml の [policy]）
sasurahime clean apfs-snapshot --thin 10GB   # ローカルスナップショットを10GB目標で間引き (tmutil)

# --- オプション ---
sasurahime stats                   # 削除履歴と統計を表示
sasurahime history                 # stats のエイリアス
sasurahime clean uv --dry-run      # 削除せずに確認だけ（dry-run）
sasurahime --yes                   # 確認なしで無人実行対象のみ削除（CI・スクリプト向け）
sasurahime clean uv --permanent    # 完全削除（ゴミ箱を経由しない）
sasurahime --version
```

## ポリシーによる無人実行

`~/.config/sasurahime/config.toml` に `[policy]` セクションを書き、
`sasurahime auto`（まずは `sasurahime auto --dry-run`）を実行します:

```toml
[policy]
older_than_days = 30   # 30日より古いエントリのみ
larger_than_mb = 100   # 100MB以上のディレクトリのみ
include_manual = false # Moderate は明示的なオプトインが必要
```

`auto` はポリシーが許すものだけを掃除します。`Risky` ターゲット
（ios-backup、apfs-snapshot など）は無人では動かず、スキップ対象は
`clean <target>` の明示実行の導線と共に表示されます。結果は
`history.json` に残ります。

launchd で定期実行するには、以下を
`~/Library/LaunchAgents/app.sasurahime.auto.plist` として保存します:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>app.sasurahime.auto</string>
  <key>ProgramArguments</key>
  <array>
    <string>/usr/bin/env</string>
    <string>/opt/homebrew/bin/sasurahime</string>
    <string>auto</string>
  </array>
  <key>StartCalendarInterval</key>
  <dict><key>Hour</key><integer>10</integer><key>Minute</key><integer>0</integer></dict>
</dict>
</plist>
```

その後 `launchctl load ~/Library/LaunchAgents/app.sasurahime.auto.plist`。
バイナリのパスは `which sasurahime` で確認してください（cargo で入れた場合は
`~/.cargo/bin/sasurahime`）。常駐デーモンは不要です。

## 安全性について

- すべてのクリーナーは `--dry-run` に対応しています。確認するまで何も削除しません。
- **Trash モードがデフォルトで有効**です。削除したファイルは macOS のゴミ箱に移動されるため、Finder から復元できます。完全に消去したい場合は `--permanent` フラグを使用してください。
- mise のランタイム削除は、グローバル設定とプロジェクト固有の `.mise.toml` を両方チェックしてから実行します。
- macOS の immutable フラグ（`uchg`）は自動的に解除してから削除します。
- Trash モードの実行は2軸で表示されます: `Logical freed: X / Physical freed: 0 B (in Trash)`
  に加えて復元導線（`open ~/.Trash`）を表示します。`--permanent` 実行は
  `--verbose` なしでも最低限の監査証跡（`Permanently deleted: no Trash copy was kept.`）
  を残し、`history.json` と `stats` に trash と delete の区別が記録されます。
- `clean trash` は表示のみです: ゴミ箱のサイズと Finder での空にする手順を示します。
  sasurahime 自身がゴミ箱を空にすることはありません。
- `clean apfs-snapshot --thin 10GB` は `tmutil thinlocalsnapshots / 10000000000 2`
  （tmutil が期待する10進バイト）を実行して、ローカルスナップショットを空き容量
  目標に間引きます。緊急度の既定は2で、`--urgency 4` は実行中のバックアップを
  中断するため追加確認が必要です。間引きは sudo を呼びません: 昇格が必要な場合は
  macOS がエラーを返し、sasurahime が再実行手順を案内します。解放量は前後の空き
  容量差で報告され、パージ可能領域は macOS が遅延更新します。
- `clean project-artifacts` は設定したルート（既定 `~/Playground`、深さ5、
  config.toml の `[project-artifacts]`）配下で、マニフェストと組になった古い
  `node_modules` + `package.json`、`target` + `Cargo.toml`、`.build` +
  `Package.swift` を検出します。7日以内に更新されたプロジェクトは除外されます。
  **保護は mtime のみで、未コミット変更の有無は見ません。** クリーニング前に
  `git status` を確認してください。
- `clean all` は明示的な全消去です: `Safe` ターゲットは追加フラグなしで実行され、
  `Moderate` / `Risky` / `Manual` は `--include-manual` が必要で、それまでは
  分類付きでスキップとして表示されます。グローバルな `--older-than <日数>` /
  `--larger-than <MB>` フィルタは `clean all` 実行時にディレクトリベースの
  クリーナーに適用され（グローバル値とクリーナー固有値の厳しい方が優先）、
  条件外のエントリはスキップ理由と共に表示されます。外部コマンド委譲系の
  クリーナー（brew、docker など）はツール自身の意味論に従うため、グローバル
  フィルタでは絞り込めません。そのため、それらの名前への `[cleaner.*]` 設定は
  引き続き「効果なし」警告が出ます。`--yes` は影響を受けません。
- `scan` は永続キャッシュ（`~/.local/share/sasurahime/scan-cache.json`）を
  ディレクトリの mtime をキーに保持し、変更のないディレクトリは高コストな
  walk をスキップします。無効化は保守的です: mtime の不一致・1時間の期限切れ・
  ファイルシステムエラー・キャッシュ破損のいずれかでフルスキャンにフォールバックします。
  削除経路はキャッシュを参照しません。常にwalkしたい場合は `--no-cache` を使用してください。
- `--yes` が削除するのは `Safe` 分類のターゲットのみです。`Moderate` /
  `Risky` / `Manual` のターゲットは分類付きでスキップとして表示され、
  明示的な選択（ピッカーまたは `clean <target>`）が必要です。

### リスク分類

ターゲットごとの分類が1つ、`--yes` の挙動・TUI の初期選択・scan の表示
（`scan`、`scan --json`）を決めます。新規ターゲットの既定値は `Moderate` で、
分類の付け忘れで無人実行に入ることはありません。

| 分類 | 意味 | `--yes` での挙動 | 例 |
|------|------|------------------|-----|
| `Safe` | 再生成される開発キャッシュ | 削除する | `uv`, `brew`, `mise`, `browsers`, `xcode`, `logs`, `act`, `huggingface`, `pre-commit`, `library-logs`, `ollama`, `device-support` |
| `Moderate` | 大きい、または一部ユーザーデータを含む | スキップ（明示的な選択） | `docker`, `cargo`, `npm`, `bun`, `go`, `pip`, `rustup`, `simulator` など |
| `Risky` | 破壊的・不可逆 | 動かない | `ios-backup`, `apfs-snapshot`, `colima` |
| `Manual` | ユーザーデータ | 動かない | `downloads`, `app-support`, `library-caches`, `orphan-containers`, `project-artifacts` |

## 名前の由来

**速佐須良比売（ハヤサスラヒメ）**は、神道の「大祓詞」に登場する祓戸の女神です。

神話の世界では、世界中から集まった罪や穢れは川から海へ、そして海の底へと流れていきます。その最後の最後に待ち受けて、すべてを根の国・底の国へ持ち去り、跡形もなく消し去るのがハヤサスラヒメです。

究極のデストラクタ。

コマンド名としての音の響き、そして「溜まった不要なものを完全に消し去る」という役割 ― 『古事記』『日本書紀』の神々の中から、このツールにぴったりの名を選びました。

## プライバシー

sasurahime はあなたのマシン上で完全にローカルに動作します。以下のことは一切行いません:
- ネットワーク経由でデータを送信する
- テレメトリや利用統計を収集する
- 個人情報を保存または送信する

### アクセスするデータ

sasurahime は以下のディレクトリを読み取り、クリーニング可能なキャッシュデータを特定します:
- `~/.cache/`、`~/Library/Caches/`、`~/.local/share/`、`~/Library/Application Support/`
- `~/Library/Application Support/MobileSync/Backup/`（iOS バックアップ ― `sasurahime clean ios-backup` でのみアクセス）
- `~/Library/Containers/com.docker.docker/Data/` と `~/.colima/`（仮想ディスクイメージのファイルサイズのみ。`sasurahime scan` で表示）
- `~/Library/Containers/` と `~/Library/Group Containers/`（フォルダ名とサイズのみ。`sasurahime clean orphan-containers` で使用）

ツールはファイルとディレクトリのメタデータ（名前、サイズ、更新日時）を読み取り、削除できるものを判断します。ファイルの内容を読み取るのは次のファイルだけで、目的は削除できるものの判断に限ります:

- mise の設定ファイル（`~/.config/mise/config.toml` と、ホームディレクトリから 5 階層以内にあるすべての `.mise.toml`）。使用中の runtime のバージョンを特定し、削除しないようにするため
- インストール済みアプリの `Info.plist`（`/Applications`、`~/Applications`、`/System/Applications` の直下と 1 階層下にある `.app`）。取り出すのは bundle ID とアプリ名だけで、アンインストール済みアプリの残骸とインストール済みアプリのデータを見分けるため

これらのファイルから読み取った値は、ログにも削除履歴にも書き出しません。このほか、sasurahime 自身の設定ファイル（`~/.config/sasurahime/config.toml`、または `--config` で指定したファイル）と削除履歴（`~/.local/share/sasurahime/history.json`）を読み取ります。それ以外のファイルの内容は読み取りません。

### フルディスクアクセス（FDA）

一部の保護パス（`~/Library/Mail`、`~/Library/Application Support/MobileSync/Backup/`、`~/.Trash`）を一覧するにはフルディスクアクセスが必要です。`sasurahime doctor` は `read_dir` だけで調べ（中身は読まない）、読めなかったパスと付与手順を表示します。`scan` も読めなかった保護パスを注記するため、合計が黙って小さく見えることはありません。存在しないパスは問題として扱いません。sasurahime が設定を自動で変更することはありません。

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