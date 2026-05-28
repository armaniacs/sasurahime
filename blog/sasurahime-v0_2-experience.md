---
title: "sasurahime v0.2 を使うと何が変わるか"
emoji: "🔍"
type: "tech"
topics: ["sasurahime", "CLI", "macOS"]
published: false
---

## sasurahime とは

uv、Homebrew、mise、Docker、Cargo、Go など 40 以上のツールの古いキャッシュを、スキャン・選択・削除する macOS 向け CLI ツールです。削除するのはキャッシュや古いバージョンだけで、現在使用中のランタイムやパッケージには触れません。

https://armaniacs.github.io/sasurahime/ja/

```bash
cargo install sasurahime
sasurahime scan
```

`scan` で何がどれだけディスクを使っているか確認できます。削除の前に全体像を把握するところから始めてください。

## v0.2 で追加された target

`sasurahime targets` を実行すると、v0.2 から3つ増えています。

| target | 実行されるコマンド | 対象 |
|--------|-----------------|------|
| `gem` | `gem cleanup` | 古い gem バージョン |
| `bundle` | `bundle clean` | Bundler のキャッシュ |
| `dotnet` | `dotnet nuget locals all --clear` | NuGet のキャッシュ全般 |

Ruby や .NET を使っていなければ、この3つは何もしません。コマンドが PATH にないときは `NotFound` として処理が進み、エラーにはなりません。

初めて実行するときは `--dry-run` をつけると安心です。

```bash
sasurahime clean gem --dry-run
```

実際には何も削除せず、削除対象だけを表示します。

## iOS バックアップを削除するときの注意

`sasurahime clean ios-backup` は他の target と少し違います。実行しようとすると、次のような警告が表示されます。

```
⚠  iOS backups contain personal data (contacts, messages, photos, etc.)
    iOS バックアップには個人データ（連絡先・メッセージ・写真など）が含まれており、
    削除後は復元できません。
```

v0.2 で「個人データが含まれる」という説明を追加しました。連絡先・メッセージ・写真が入っているので、削除の前に一度止まって考えてほしいという意図です。

ただし、実際にはゴミ箱に移動されます。「ゴミ箱を空にする」するまで完全には消えないので、間違えてもすぐ取り戻せます。

## 動きがおかしいと感じたら

`sasurahime clean uv` が何も削除しない、想定より少ない、といった場面では環境変数 `RUST_LOG=debug` をつけてみてください。

```bash
RUST_LOG=debug sasurahime clean uv
```

内部の動作（ファイルの検出、`chflags` の結果、config の読み込み状態など）が標準エラーに出力されます。普段は出ないので邪魔になりません。デフォルトのログレベルは `warn`、つまり警告とエラーだけです。

## プライバシーについて

インストールを迷っている人に向けて、README に Privacy セクションが追加されています（日本語版もあります）。

- sasurahime が読むディレクトリの一覧
- データを外部に送信しないこと
- `history.json` に何が記録されるか

「このツールが何をやっているか確認したい」という人は、インストール前にそこを読んでみてください。

## まず試すなら

```bash
cargo install sasurahime
sasurahime scan
```

scan の結果を見て、削除してもよさそうなものだけ `clean` に渡すのが安全な使い方です。一度に全部やろうとせず、`--dry-run` で確認してから進めてください。
