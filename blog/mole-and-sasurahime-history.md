---
title: "sasurahimeをHomebrewにしたら、Moleと出会ってhistoryをつけることにした"
emoji: "🧹"
type: "tech"
topics: ["sasurahime", "Mole", "CLI", "macOS", "cleaner"]
published: false
---

sasurahimeをHomebrewでインストールできるようにしたのが、昨晩のことです。

```bash
brew tap armaniacs/sasurahime
brew install sasurahime
```

その流れでbrewのリポジトリを眺めていると、[tw93/Mole](https://github.com/tw93/Mole) というディスククリーナーを知りました。Go製で、キャッシュや一時ファイル、大きなファイルを幅広く掃除してくれるツールです。

正直、最初は「これがあればsasurahimeなくてもいいんじゃないか」と思いました。sasurahimeは自分がRustで作っている開発者向けクリーナーですが、Moleの方が圧倒的に手軽に見えたので。

ところが実際に使ってみると、ぜんぜんそういう話ではありませんでした。

## Moleを入れて実行する

インストールはHomebrewで一発です。

```bash
brew install mole
```

そのあと `mo clean` を実行しました。特に何も考えずに動かせるのが気楽です。

しばらく待つと、こんな結果が出ました。

```
➤ System caches
  ✓ VS Code webview cache 58 items, 3.45GB
  ✓ Chrome caches 12 items, 1.21GB
  ✓ Xcode derived data 8 items, 0.85GB
  ...

➤ Large files
  ◎ LM Studio models (review only): 6.40GB, Path: /Users/yaar/.lmstudio/models

Cleanup complete
Tracked cleanup: 16.72GB | Items cleaned: 3580 | Categories: 79
```

16GB以上が一瞬で消えました。カテゴリが79もあるのには驚きます。

## 衝撃だったのはVS Codeのwebviewキャッシュ

Moleの結果で特に目を引いたのは、普段意識していないファイルたちでした。

```
✓ VS Code webview cache 58 items, 3.45GB
```

VS Codeのwebviewキャッシュだけで3.45GBもあるのは予想外でした。他にもLM Studioのモデルが6.40GBあったりと、開発環境を使っていると自然に溜まる大きなものが可視化されます。

`mo history` で履歴も確認できます。

```
~/Downloads ❯ mo history

Mole History

Recent sessions
  optimize   2026-06-24 04:53:57, 22 items, 0B
             no file actions, ended 2026-06-24 04:54:20
  clean      2026-06-24 04:46:09, 3580 items, 16.72GB
             removed 4611, skipped 121, failed 18, ended 2026-06-24 04:52:25
```

どれだけ消したか、いつ消したかが記録されているのは地味に便利です。

## それでもsasurahimeを動かしたらまだ7GBあった

Moleで16GBも削減した直後に、sasurahimeを動かしてみました。

```
~/Downloads ❯ sasurahime
sasurahime v0.2.1
Scan complete [OK]
Select caches to clean  [space to toggle, enter to confirm]:
> [ ] uv  (5.7 GB)
  [ ] brew  (21.9 MB)
  [ ] mise  (11.7 MB)
  [ ] logs  (50.0 MB)
  [ ] huggingface  (1.1 GB)
```

まだ合計で7GB弱、クリーニング可能なものが残っていました。

これを見たとき、「やはりターゲットが違う」と実感しました。

## Moleとsasurahimeの棲み分け

両方を使ってみて、次のように整理できました。

| 項目 | Mole | sasurahime |
|---|---|---|
| 対象 | キャッシュ、一時ファイル、大きなファイルなど幅広く | 開発ツール固有のキャッシュ |
| 強み | 手軽に大量に削減できる | uv、Homebrew、mise、huggingfaceなど詳細に対応 |
| イメージ | 部屋全体の掃除機 | 工具箱の整理 |

Moleは「どこに何が溜まっているかわからない」状態から一気に解放してくれます。一方、sasurahimeは開発ツールが作るキャッシュを個別に扱うのが得意です。

例えばuvのキャッシュは、単純に削除するのではなく `uv cache prune` を使うべきですし、miseのランタイムは `.mise.toml` と照らし合わせてから削除する必要があります。これらはsasurahimeが担当する領域です。

## だからsasurahime historyでも起動するようにする

この流れで、sasurahimeを作ったのは無駄じゃないと確信しました。むしろ、Moleがカバーしない層を引き続き担当する存在として必要だと思います。

そこで次に取り組むのが、`sasurahime history` でも同じ画面が開くようにすることです。現在は `sasurahime stats` コマンドで履歴を表示できます。

```
~/Downloads ❯ sasurahime stats

  ╔═══════════════════════════════════╗
  ║  sasurahime Statistics                   ║
  ║  Total freed:  6.8 GB                    ║
  ║  Runs:         3                         ║
  ╚═══════════════════════════════════╝

Recent cleanups:
    #  Date              Cleaner        Size
    1  2026-06-24T05:10  uv             5.7 GB
    2  2026-06-24T05:08  huggingface    1.1 GB
    3  2026-06-24T05:05  logs           50.0 MB
```

内部的には `~/.local/share/sasurahime/history.json` に、クリーナー名・解放バイト数・タイムスタンプが記録されています。`sasurahime stats` も `sasurahime history` も、同じデータを同じ形式で見せるだけです。

`history` という名前を追加するだけなので大きな変更ではありませんが、`mo history` と同じ感覚で「いつ・何を・どれだけ消したか」を振り返れるのは自然だと思います。

履歴が見えるようになると、次のような使い方が自然になります。

- 定期実行して「先月どれだけ削減できたか」を確認する
- 特定のクリーナーがどのくらい効いているか把握する
- 誤って消しすぎたかどうか、あとから振り返る

## まとめ

Moleは手軽で強力なクリーナーです。普段使っている人なら、まずMoleで広く掃除するのがよいと思います。

ただ、開発者が使うツール特有のキャッシュはsasurahimeの担当領域として残ります。今回、Moleで16GB削減したあとでもsasurahimeで7GB弱見つかったのは、その差をはっきり示しています。

`sasurahime stats` または `sasurahime history` も活用ください。

## 追記：v0.2.1 をリリースしました

2026-06-24、本記事で書いた `sasurahime history` を `sasurahime stats` のエイリアスとして追加し、v0.2.1 としてリリースしました。

```bash
brew tap armaniacs/sasurahime
brew update
brew install sasurahime
```

すでにインストール済みの場合は `brew upgrade sasurahime` で更新できます。v0.2.1 では `sasurahime history` と `sasurahime stats` が同じ画面を表示します。