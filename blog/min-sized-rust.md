---
title: "Rust のバイナリサイズを 35% 削減するまで"
emoji: "📦"
type: "tech"
topics: ["Rust", "binary-size", "LTO", "performance"]
published: false
---

## バイナリが 1.3MB もある

macOS のキャッシュクリーナーを Rust で書いています。clap・dialoguer・comfy-table といった UI 系のクレートも使っているので、ある程度のサイズになるのは仕方ないと思っていました。

とはいえ 1.3MB。キャッシュを掃除するだけの CLI ツールにしてはちょっと大きい。

何かできることはないかと調べていたところ、[johnthagen/min-sized-rust](https://github.com/johnthagen/min-sized-rust) というリポジトリを見つけました。Rust のバイナリサイズを小さくする方法論が網羅的にまとまっています。

「よし、全部試そう」と思い立ち、実際に適用してみた結果、**1,349,184 bytes（1.3MB）→ 872,640 bytes（852KB）** になりました。削減率は **35%** です。

この記事では、どのテクニックをどの順番で適用したか、そして最後にちょっとした罠にはまった話を書きます。

## 適用前の状態

プロジェクトの `[profile.release]` はこうなっていました。

```toml
[profile.release]
opt-level      = "s"      # Balance size and speed for CLI
strip          = true     # Strip symbols
lto            = "thin"   # Thin LTO: ~90% benefit, ~20% time
codegen-units  = 1        # Single codegen unit
```

すでに最小限の配慮はしてあります。`strip = true` でシンボル削除、`codegen-units = 1` で単一コード生成ユニット、`lto = "thin"` でリンク時最適化も有効。これ以上削る余地はなさそうに見えます。

ところが、まだやれることはありました。

## Step 1: Full LTO + Panic Abort

最初に効いたのはこの2つです。

### Full LTO（Link Time Optimization）

`lto = "thin"` から `lto = true` への変更です。Thin LTO はコンパイル時間とサイズ削減のバランスを取った設定で、実用的な選択ではあります。しかし最終的にバイナリサイズを極めたいなら Full LTO のほうが効果を発揮します。

そもそも LTO の仕組みは、コンパイル単位ごとに閉じていた最適化の範囲をリンク時に拡大するというものです。関数のインライン化やデッドコード除去がモジュールやクレートの境界を越えて行われるようになります。Thin LTO はインポートテーブルを使って最低限のクロスモジュール最適化を行いますが、Full LTO はすべての関数の呼び出し関係を考慮して、より積極的にコードを削ります。

### Panic Abort

Rust はデフォルトで、`panic!()` が発生したときにスタックを巻き戻してバックトレースを表示します。この巻き戻し処理（アンワインド）には `libunwind` などのランタイムコードが必要で、それがバイナリのかなりの領域を占めます。

`panic = "abort"` を指定すると、パニック時に即座にプロセスが終了します。バックトレースは出なくなりますが、CLI ツールであれば実用上の問題はほとんどありません。エラーハンドリングは `Result` で行うのが Rust の標準的なパターンであり、パニックに頼るコードはそもそもごく一部です。

```toml
# Step 1 の設定
[profile.release]
opt-level      = "s"
strip          = true
lto            = true       # thin → true
codegen-units  = 1
panic          = "abort"    # ← 追加
```

この2つの変更で、**1,349,184 bytes → 1,002,496 bytes（979KB）**。削減率 **26%** です。この時点で 300KB 以上削れました。

## Step 2: opt-level を "s" から "z" へ

`opt-level` はコンパイラの最適化の方向性を決めるフラグです。デフォルトの `3` は実行速度優先。`"s"` はサイズを抑えつつ速度もある程度保つバランス型。`"z"` はサイズに振り切った設定になります。

公式ドキュメントにはこうあります。

> It is recommended to experiment with different levels to find the right balance for your project. There may be surprising results, such as ... the `"s"` and `"z"` levels not being necessarily smaller.

つまり、「`"s"` より `"z"` が常に小さいとは限らないから、自分のプロジェクトで測ってみてね」ということです。

実際に測ってみました。

| opt-level | サイズ | 
|-----------|--------|
| `"s"` | 1,002,496 bytes |
| `"z"` | **872,640 bytes** |

このプロジェクトでは `"z"` のほうが明らかに小さくなりました。ただし、速度が犠牲になるトレードオフはあります。キャッシュクリーナーは I/O バウンドな処理が中心なので体感できる差は出ませんでしたが、CPU バウンドなアプリケーションの場合は `"z"` による速度低下を考慮する必要があります。

累積で **35% 削減**。十分な成果です。

## 最終的な Cargo.toml

```toml
[profile.release]
opt-level      = "z"      # Aggressive size optimization
strip          = true     # Strip symbols
lto            = true     # Full LTO for max size reduction
codegen-units  = 1        # Single codegen unit enables more optimizations
panic          = "abort"  # Abort on panic removes unwinding code
```

たった4行の変更です。

## おまけ: UPX の罠

ここでもうひと押しできないかと、**UPX**（Ultimate Packer for eXecutables）というバイナリ圧縮ツールを試しました。

```bash
$ upx --best --lzma --force-macos target/release/sasurahime

        File size         Ratio      Format      Name
   --------------------   ------   -----------   -----------
    872640 ->    360464   41.31%   macho/arm64   sasurahime
```

872KB → 360KB。圧縮率 **59%**。すごい。

ところが、圧縮したバイナリを実行しようとすると、何も出力されずに終了コード 137（SIGKILL）で殺されてしまいました。

```bash
$ ./sasurahime --help
（何も出力されない）
$ echo $?
137
```

原因は macOS のセキュリティ機構です。UPX でパックされた Mach-O バイナリは、コード署名の情報が壊れるため Gatekeeper や XProtect にブロックされます。arm64 の macOS では特にこの挙動が顕著で、`--force-macos` フラグがあっても実用に耐えません。

UPX 自体は Linux 向けのバイナリでは広く使われている手法なので、Linux へのクロスコンパイルを視野に入れるなら検討の価値はあります。

## テストの安全性確認

`panic = "abort"` はアプリケーションの動作を変える設定です。もしコード内で `catch_unwind` を使ったパニック回収を行っていると、abort に変更したことで正しく動かなくなります。

プロジェクト内を検索したところ、`catch_unwind` の使用箇所はゼロ。実際に全 226 のテストがパスすることを確認しました。

```bash
$ cargo test
...
test result: ok. 226 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## 振り返り

今回の取り組みで学んだことをまとめます。

- **Full LTO + Panic Abort** が一番効く。この2つでサイズの 1/4 以上が削れる
- **opt-level は "s" と "z" で実測して決める**。一概に "z" が小さいとは限らない
- **UPX は macOS では使えない**。Linux 向けや配布パッケージには有効
- **panic = "abort" を入れる前に catch_unwind の有無を確認する**

Rust のバイナリサイズ削減に興味がある方は、[johnthagen/min-sized-rust](https://github.com/johnthagen/min-sized-rust) を一度読んでみてください。今回紹介した以外にも、Nightly 限定の `build-std` や `no_std` による極小バイナリ化のテクニックが載っています。冒頭の 8KB バイナリはまさにその成果物です。

私のプロジェクトでは stable Rust をターゲットにしているため今回は見送りましたが、より aggressive な削減を目指す方はそちらも選択肢に入ります。
