---
title: "sasurahime で Mac 開発キャッシュをサッと掃除する"
emoji: "🗑️"
type: "tech"
topics: ["rust", "macos", "cli", "キャッシュクリーン"]
published: false
---

## なにができるのか

`uv` のアーカイブキャッシュ、Homebrew のダウンロード残骸、`mise` が取り残した古い Node ランタイム、Playwright や Puppeteer がダウンロードしたブラウザの過去バージョン──。

そういった開発ツールのキャッシュを検出し、**選んで削除**できる CLI ツールです。

名前は **sasurahime**（速佐須良比売）。日本神話の大祓詞に出てくる、穢れを奈落の底で跡形もなく消し去る女神から来ています。

## インストール

GitHub Releases からプリビルドバイナリをダウンロードします。

```bash
curl -LO https://github.com/armaniacs/sasurahime/releases/download/v0.1.5/sasurahime-aarch64-apple-darwin.tar.gz
tar xzf sasurahime-x86_64-apple-darwin.tar.gz
sudo mv sasurahime /usr/local/bin/
```

Rust の環境があるなら：

```bash
cargo install sasurahime
```

## 使い方：たったの3ステップ

### 1. 実行する

ターミナルで `sasurahime` と打つだけです。

```bash
sasurahime
```

バージョン表示のあと、対応しているキャッシュを片っ端からスキャンします。

```
sasurahime v0.1.5
Scanning uv... [OK]
Scanning brew... [OK]
Scanning mise... [OK]
Scanning browsers... [OK]
Scanning xcode... [OK]
Scanning logs... [OK]
Scanning act... [OK]
Scanning huggingface... [OK]
Scanning pre-commit... [OK]
Scanning library-logs... [OK]
Scanning colima... [OK]
Scanning ollama... [OK]
Scanning device-support... [OK]
```

### 2. 掃除するものを選ぶ

一覧が表示されたら、スペースキーで削除したい項目を選びます。

```
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

選んだら Enter を押すと、解放できる容量が表示されて確認プロンプトが出ます。

```
Selected: brew (75.1 MB), logs (43.5 MB)
Will free approximately 118.6 MB.
Proceed? [y/N]
```

### 3. 確認して削除

`y` を入力すると掃除が始まります。スピナーと一緒に、何がどのくらい削除されたかがリアルタイムに表示されます。

```
Cleaning brew... [OK]              Freed: 54.5 MB
Cleaning logs... [OK]              Removed 2 log files

Total freed: 98.0 MB
```

これだけです。

## ワンライナーで全部掃除

いちいち選ぶのが面倒なときは `--yes` フラグを渡します。確認プロンプトは出ますが、全ターゲットを一気に掃除します。

```bash
sasurahime --yes
```

## 削除前に試す

`--dry-run` をつければ、実際には何も削除せずに「これだけ減らせるよ」というレポートだけ表示します。

```bash
sasurahime clean brew --dry-run
```

## セーフティ

- 削除されたファイルは **デフォルトで macOS の Trash に移動** されます。Finder から復元できます。
- 完全に消したい場合は `--permanent` フラグを指定します。
- どのサブコマンドも `--dry-run` に対応しており、削除前に結果を確認できます。

---

中身や各クリーナーの詳細に興味がある方は、**「sasurahime: Rust 製 macOS キャッシュクリーナーの哲学と全機能」**（続編）をご覧ください。
