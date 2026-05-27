---
title: "sasurahime で Mac 開発キャッシュをサッと掃除する"
emoji: "🗑️"
type: "tech"
topics: ["rust", "macos", "cli", "キャッシュクリーン", "sasurahime"]
published: false
---

## なにができるのか

`uv` のアーカイブキャッシュ、Homebrew のダウンロード残骸、`mise` が取り残した古い Node ランタイム、Playwright や Puppeteer がダウンロードしたブラウザの過去バージョン──。

そういった開発ツールのキャッシュを検出し、**選んで削除**できる CLI ツールです。

名前は **sasurahime**（速佐須良比売）。日本神話の大祓詞に出てくる、穢れを奈落の底で跡形もなく消し去る女神から来ています。

## インストール

```bash
cargo install sasurahime
```

Rust 1.70+ と macOS (arm64 / x86_64) が必要です。

## 使い方：たったの3ステップ

### 1. 実行する

ターミナルで `sasurahime` と打つだけです。

```bash
sasurahime
```

バージョン表示のあと、対応しているキャッシュを片っ端からスキャンします。

```
sasurahime v0.1.27
Scanning... (12/32) [▓▓▓▓░░░░░░░░░░░░]
```

従来の逐次スキャンより高速な**並列スキャン**で、待ち時間を短縮しています。

### 2. 掃除するものを選ぶ

一覧が表示されたら、スペースキーで削除したい項目を選びます。

```
Select caches to clean  [space to toggle, enter to confirm]:
> [ ] uv                   3.6 GB
  [ ] brew                 75.1 MB
  [ ] xcode > DerivedData  15.3 GB
  [ ] xcode > Archives     5.2 GB
  [ ] logs                 43.5 MB
  [ ] huggingface          1.1 GB
  [ ] colima               9.3 GB
```

Xcode のように複数のキャッシュを持っているものは、サブカテゴリに展開されて個別に選べます。

選んだら Enter を押すと、解放できる容量が表示されて確認プロンプトが出ます。

```
Will free approximately 15.4 GB.
Proceed? [y/N]
```

### 3. 確認して削除

`y` を入力すると掃除が始まります。スピナーと一緒に、何がどのくらい削除されたかがリアルタイムに表示されます。

```
Cleaning brew... [OK]              Freed: 54.5 MB
Cleaning xcode > DerivedData [OK]  Freed: 15.0 GB
Cleaning logs... [OK]              Removed 2 log files

Total freed: 15.1 GB
```

これだけです。

## ワンライナーで全部掃除

いちいち選ぶのが面倒なときは `--yes` フラグを渡します。

```bash
sasurahime --yes
```

cron や CI での定期実行にも最適です。設定ファイルで除外したいクリーナーを指定しておけば、安全な範囲だけ自動掃除できます。

## 削除前に試す

`--dry-run` をつければ、実際には何も削除せずに「これだけ減らせるよ」というレポートだけ表示します。

```bash
sasurahime clean brew --dry-run
```

## 削除履歴を確認する

`sasurahime stats` で、累計の削除量と実行履歴を表示できます。

```bash
$ sasurahime stats
Total freed:  12.5 GB
Runs:         15

Recent cleanups:
  #  Date                Cleaner        Size
  1  2026-05-27 10:30   uv             500.0 MB
  2  2026-05-26 22:15   brew           1.2 GB
```

## 設定ファイル

`~/.config/sasurahime/config.toml` に設定を書けます。

```toml
# スキャンから除外するクリーナー
exclude = ["huggingface"]

# 自分でキャッシュディレクトリを追加
[[custom]]
name = "my-project"
path = "~/work/.cache"

# クリーナーごとのフィルタ
[cleaner.act]
older_than_days = 30
```

## セーフティ

- 削除されたファイルは **デフォルトで macOS の Trash に移動** されます。Finder から復元できます。
- 完全に消したい場合は `--permanent` フラグを指定します。
- どのサブコマンドも `--dry-run` に対応しており、削除前に結果を確認できます。
- 設定ファイルの `exclude` にクリーナー名を書けば、そのクリーナーはスキャン一覧に表示されなくなります（削除したいときに消せないわけではなく、直接 `sasurahime clean <target>` すれば実行できます）。

## 対応している主なターゲット

言語ランタイム： uv, mise, rustup, go, deno, flutter, sbt, maven, gradle, spm
パッケージマネージャ： brew, pip, pipx, npm, yarn, pnpm, bun, poetry, conda, cocoapods
ブラウザ： browsers (Puppeteer / Playwright), playwright
IDE： xcode (DerivedData / Archives), device-support, vscode-extensions, jetbrains
コンテナ： docker, orbstack, colima
その他： logs, library-logs, ios-backup, huggingface, ollama, pre-commit, tree-sitter, cargo, node-gyp, torrent, downloads, trash, explorer, apfs-snapshot, act, terraform, simulator, git, volta

40 以上のターゲットに対応しています。`sasurahime targets` でいつでも一覧を確認できます。

---

**リポジトリ:** https://github.com/armaniacs/sasurahime
