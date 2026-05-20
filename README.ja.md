# sasurahime

macOS 開発者向けキャッシュクリーナー（Rust 製）。

uv・Homebrew・mise のランタイム・Playwright/Puppeteer のブラウザバイナリ・bun・Go・pip など、開発ツールが積み上げたキャッシュをスキャンし、何がどれだけ使っているかを表示したうえで、選択して削除できます。安全に。

```
$ sasurahime scan

Category             Size       Status
────────────────────────────────────────
uv (archive)         18.2 GB    pruneable
Homebrew downloads   16.6 GB    stale
bun cache             5.5 GB    clearable
mise / node (old)     3.4 GB    unused
Playwright (old)      0.5 GB    stale
────────────────────────────────────────
Total reclaimable    44.2 GB
```

> **サイズについて**: 上記の数値は開発者一人の環境での実測値です。
> 環境によって大きく異なります。目安としてご覧ください。

## 使い方

```bash
# スキャンして一覧表示（削除しない）
sasurahime scan

# インタラクティブに選択して削除
sasurahime

# 対象を指定して削除
sasurahime clean uv
sasurahime clean brew
sasurahime clean mise
sasurahime clean browsers
sasurahime clean caches          # bun / go / pip / node-gyp
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

MIT

## コントリビュート

Issue・PR ともに歓迎します。詳細は [CONTRIBUTING.md](CONTRIBUTING.md) をご覧ください。

機能の提案はコードを書く前に Issue を立てて相談してください。
