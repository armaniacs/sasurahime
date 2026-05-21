---
title: "Rustのバイナリサイズ、2MB→917KBにしたった"
emoji: "📦"
type: "tech"
topics: ["Rust", "バイナリサイズ最適化", "Cargo"]
published: true
---

## 2MBのバイナリがデカく見えてきた

久しぶりに `ls -lh target/release/` を見たら、こんな感じでした。

```
$ ls -lh target/release/sasurahime
-rwxr-xr-x  1 yaar  staff   2.0M May 19 04:07 target/release/sasurahime
```

2MB。macOS のキャッシュを掃除するだけの CLI ツールとしては、ちょっと大きくないでしょうか。

パッケージマネージャーに依存は8つ。clap に indicatif に dialoguer に comfy-table ……よくある顔ぶれです。コード自体は大した量ではありません。なのにバイナリにすると2MB。

「もっと縮むはずだ」と思って調べてみました。

## まずは opt-level の確認

`Cargo.toml` を開くと、こうなっていました。

```toml
[profile.release]
# 何も書いてない
```

Rust のリリースビルドのデフォルトは `opt-level = 3`。速度最適化です。バイナリサイズはお構いなし。

ここを `opt-level = "z"` にすると、LLVM がサイズ優先でコードを生成します（`-Oz` 相当）。試してみましょう。

```toml
[profile.release]
opt-level = "z"
```

結果は……。

```
$ ls -lh target/release/sasurahime
-rwxr-xr-x  1 yaar  staff   2.0M
```

**変わりませんでした。** ほぼ誤差の範囲です。

Rust のリリースビルドはデフォルトで十分に最適化されているので、opt-level を 3 から z に変えても、単体では劇的な効果は出にくいようです。本当に効くのは、他の設定と組み合わせたときでした。

## 本命はこちら

以下の4つをセットで指定します。

```toml
[profile.release]
opt-level      = "z"     # サイズ最適化
strip          = true    # シンボルを削除
lto            = true    # リンク時最適化
codegen-units  = 1       # コード生成ユニットを1つに
```

一個ずつ見ていきます。

### strip = true

デバッグシンボルを削除します。

リリースビルドでもデフォルトではデバッグ情報が一部残ります。`strip = true` を指定すると、それらを一切含めなくなります。

効果は絶大で、これだけで**約700KB**減りました。Rust のジェネリクスは大量のシンボルを生成するので、その削減効果がそのまま効きます。

### lto = true

Link Time Optimization。クレートの境界を越えた最適化を行います。

Rust のコードは複数のクレートに分割されていますが、デフォルトではクレートごとに独立してコンパイルされます。`lto = true` にすると、リンク時にすべてのクレートのコードをまとめて最適化します。

- 使われていない関数が削除される
- インライン化の機会が増える
- 重複したコードが統合される

副作用としてビルド時間は増えます（体感で1.5〜2倍程度）。リリースビルドなので許容範囲です。

### codegen-units = 1

LLVM のコード生成を単一ユニットに制限します。

デフォルトでは Rust は16（または CPU コア数）のコード生成ユニットに分割して並列コンパイルします。これによりビルドは速くなりますが、ユニット間の最適化の機会が失われます。

`codegen-units = 1` にすると、単一ユニットとしてコード生成されるため、LLVM がより積極的に最適化できるようになります。`lto` との相乗効果が大きい設定です。

### opt-level = "z"

冒頭で単体では効かないと書きましたが、`lto` + `codegen-units = 1` と組み合わせると効いてきます。LLVM が関数のインライン展開やループの展開を「サイズを増やさない」基準で判断するようになるためです。

`opt-level = "s"`（速度とサイズのバランス）もありますが、CLI ツールで速度が問題になることは稀なので、最もサイズに効く `"z"` を選びました。

## 結果

```
$ ls -lh target/release/sasurahime
-rwxr-xr-x  1 yaar  staff   917K
```

**2.0MB → 917KB**。約54%削減です。

| 設定 | サイズ |
|---|---|
| デフォルト (opt-level=3) | 2,016 KB |
| opt-level = "z" のみ | 1,968 KB |
| 全部盛り | **917 KB** |

## まとめ

Rust のバイナリサイズを削減するには、**全部盛りが一番効きます**。

```toml
[profile.release]
opt-level      = "z"
strip          = true
lto            = true
codegen-units  = 1
```

どれか一つだけでは効果が限定的ですが、組み合わせることで相乗効果が出ます。特に `strip` と `lto` + `codegen-units = 1` の組み合わせが効きました。

「まだ最適化してなかった」という方は、ぜひ一度試してみてください。`Cargo.toml` に7行追加するだけで完了します。

ちなみに今回の対象プロジェクトは [sasurahime](https://github.com/yaar/sasurahime) という macOS のキャッシュクリーナーです。よろしければ README だけでもご覧ください。
