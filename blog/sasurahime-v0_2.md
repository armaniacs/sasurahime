---
title: "sasurahime v0.2 をどう直したか — Checking Team レビューで見えたもの"
emoji: "🧹"
type: "tech"
topics: ["Rust", "CLI", "sasurahime"]
published: false
---

Claude にレビューを頼んだら、自分では見えていなかった問題が出てきた。全部で12件、25 SP。作業記録として残しておく。

https://armaniacs.github.io/sasurahime/ja/

## 全体像: 12 PBI の分類

PBI 01-12 は大きく5つのカテゴリに分かれます。

| カテゴリ | PBI | SP | 内容 |
|---------|:---:|:--:|------|
| セキュリティ・安全性 | 01, 02, 09 | 3 | エラー誤検出防止、Trash 対応、デッドコード削除 |
| テスト品質 | 03 | 2 | rstest パラメータ化、VerboseGuard 移行 |
| パフォーマンス | 04 | 3 | detect/clean の二重 walk をキャッシュで排除 |
| アーキテクチャ | 06, 07 | 8 | Cleaner トレイト契約統一、main.rs マクロ整理 |
| 運用・文書 | 05, 08, 10/11/12 | 9 | 構造化ログ、プライバシー文書、新 target 追加 |

## PBI-01: is_skippable_error の誤検出を潰す

`is_skippable_error` は削除失敗エラーを「スキップ可能か」判定する関数です。`PermissionDenied` や `Resource busy` を部分文字列マッチ（`contains`）で判定していたため、エラーメッセージ中にこれらの単語が偶然含まれると本来スキップすべきでないエラーまで抑制されていました。

修正は単純で、`contains` を `starts_with` に変えるだけです。

```rust
// Before: 誤検出のリスクあり
msg.contains("Permission denied")
    || msg.contains("Resource busy")

// After: 先頭一致で正確に判定
msg.starts_with("Permission denied")
    || msg.starts_with("Resource busy")
```

`"trash failed"` だけは `contains` のまま維持しました。自分たちで制御できる内部プレフィックスだからです。

## PBI-02: Gradle/JetBrains だけ Trash を bypass していた問題

全 cleaner の中で Gradle と JetBrains だけが `fs::remove_dir_all` を直接呼び、Trash に移動せず永久削除していました。確認すると単なる実装漏れです。`crate::trash::delete_path` に差し替え、`is_skippable_error` でエラーハンドリングするパターンに統一しました。

## PBI-03: 17件の重複テストを rstest でパラメータ化

`sasurahime clean deno` が tool-not-found でエラーになるテストが、17個のほぼ同一関数として並んでいました。`rstest` を導入し、1つのパラメータ化テストに統合しました。

```rust
#[rstest]
#[case("bun", "/usr/bin:/bin", true)]
#[case("deno", "/usr/bin:/bin", true)]
#[case("sbt", "/usr/bin:/bin", false)]
// ... 全17ケース
fn clean_tool_not_found_skips(#[case] tool: &str, #[case] path: &str, #[case] check_stdout: bool) {
    // 1つのロジックで17ケースをカバー
}
```

`check_stdout` フラグが必要だった理由は、`DeleteDirs` ベースの cleaner（sbt, volta など）は CLI の有無をチェックせず、ディレクトリが存在しないだけで何も出力せずに終了するからです。

## PBI-04: detect/clean の二重 walk を OnceLock でキャッシュ

`CargoCleaner` と `MiseCleaner` は `detect()` と `clean()` の両方で同じ walkdir を実行していました。`std::sync::OnceLock` で結果をキャッシュし、初回のみ計算するようにしました。

```rust
pub struct CargoCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    target_cache: std::sync::OnceLock<Vec<(PathBuf, u64)>>,
}

impl CargoCleaner {
    fn get_target_dirs(&self) -> &Vec<(PathBuf, u64)> {
        self.target_cache
            .get_or_init(|| Self::find_target_dirs(&self.home))
    }
}
```

`OnceLock` は `OnceCell` のスレッドセーフ版です。Rust 1.70 から標準ライブラリにあります。`&self` で共有可能なのでトレイトとの相性が良いです。

## PBI-06: Cleaner トレイトに clean_with_opts を追加

`LibraryLogsCleaner` だけが `clean_all()` という非トレイトメソッドを持ち、`main.rs` で特別扱いされていました。

`CleanOptions` 構造体を定義し、トレイトのデフォルトメソッドとして `clean_with_opts` を追加。`LibraryLogsCleaner` だけオーバーライドします。

```rust
#[non_exhaustive]
pub struct CleanOptions {
    pub all: bool,
}

pub trait Cleaner: Send + Sync {
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;

    fn clean_with_opts(
        &self,
        dry_run: bool,
        reporter: &dyn ProgressReporter,
        _opts: &CleanOptions,
    ) -> Result<CleanResult> {
        // デフォルト: opts を無視
        self.clean(dry_run, reporter)
    }
}
```

`#[non_exhaustive]` をつけておくと、将来 `recursive` や `force` オプションを追加しても後方互換を壊しません。

## PBI-07: main.rs から cmd_name! マクロを消した

`define_cleaners!` は既に `$cli_name:literal` で CLI 名を持っているのに、別途 `cmd_name!` マクロが同名の定義をしていました。二重管理を解消し、127行削除しました。

ついでに `exit_code() != 0` のチェックが9回コピペされていたのもラッパー関数にまとめました。

## PBI-05: env_logger 導入

最後の PBI は構造化ログです。`log` + `env_logger` を追加し、`eprintln!` で分散していた警告やエラー出力を `log::warn!` / `log::error!` に置き換えました。

```rust
fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn")
    )
    .format_timestamp(None)
    .init();
}
```

`RUST_LOG=debug` で詳細ログが出ます。ユーザー向けの出力（`println!`, progress bar）はそのままです。ログは `eprintln!` 相当の扱いで、標準出力を汚しません。

## その他の PBI

| PBI | 内容 | ポイント |
|:---:|------|---------|
| 08 | README に Privacy セクション追加 | 日本語版も |
| 09 | `#[allow(dead_code)]` 一掃 | 14箇所、半分は実際に使用されていた |
| 10/11/12 | gem/bundle/dotnet target 追加 | `command_cleaner` パターンで3行ずつ |

## ここから先にやること

指摘された12件を振り返ると、「一度は気になっていたけど後回しにした」ものばかりでした。後回しにしたツケは、外部レビューという形で必ず返ってきます。

全12 PBI で 25 SP、コミット数は 20 強でした。テストは 346 → 346（増減なし、書き換えのみ）、clippy 警告0を維持しています。

次のスプリントでは PBI-008（インタラクティブ TUI）に入ります。`dialoguer::MultiSelect` で cleaner を選ばせる UI を追加し、`sasurahime` をサブコマンドなしで起動したときに動くようにします。`clean_with_opts` で整えたトレイト設計が、ここで使えるはずです。
