# sasurahime

<details open>
<summary><strong>🇺🇸 English</strong></summary>

A macOS developer cache cleaner written in Rust.

sasurahime scans well-known cache locations — uv, Homebrew, mise runtimes, Playwright/Puppeteer browsers, bun, Go, pip, and more — shows you what's taking up space, and lets you choose what to remove. Safely.

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

## Usage

```bash
# Scan and report (no deletion)
sasurahime scan

# List all supported clean targets
sasurahime targets

# Show version
sasurahime --version

# Clean everything interactively
sasurahime

# Clean specific targets
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs

# Preview without deleting
sasurahime clean uv --dry-run

# Non-interactive (CI / scripting)
sasurahime --yes

# Permanently delete (bypass Trash)
sasurahime clean uv --permanent
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

## License

Apache-2.0

## Contributing

Issues and PRs are welcome. See [HOWTO-ADD-target.md](docs/HOWTO-ADD-target.md) for details.

For feature proposals, please open an issue first so we can discuss the scope before diving into code.

</details>

<details>
<summary><strong>🇯🇵 日本語</strong></summary>

macOS 開発者向けキャッシュクリーナー（Rust 製）。

uv・Homebrew・mise のランタイム・Playwright/Puppeteer のブラウザバイナリ・bun・Go・pip など、開発ツールが積み上げたキャッシュをスキャンし、何がどれだけ使っているかを表示したうえで、選択して削除できます。安全に。

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

## 使い方

```bash
# スキャンして一覧表示（削除しない）
sasurahime scan

# 対応している削除対象を一覧表示
sasurahime targets

# バージョン表示
sasurahime --version

# インタラクティブに選択して削除
sasurahime

# 対象を指定して削除
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean xcode
sasurahime clean caches          # bun / go / pip / node-gyp / npm / yarn / pnpm
sasurahime clean logs

# 削除せずに確認だけ（dry-run）
sasurahime clean uv --dry-run
sasurahime scan --dry-run

# 確認なしで全削除（CI・スクリプト向け）
sasurahime --yes

# 完全削除（ゴミ箱を経由しない）
sasurahime clean uv --permanent
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

## ライセンス

Apache-2.0

## コントリビュート

Issue・PR ともに歓迎します。詳細は [HOWTO-ADD-target.md](docs/HOWTO-ADD-target.md) をご覧ください。

機能の提案はコードを書く前に Issue を立てて相談してください。</details>