# PBI: main.rs の Cleaner 登録システムをリファクタリング

## ユーザーストーリー
デベロッパーとして、新しい cleaner を追加するときに `define_cleaners!`、`cmd_name!`、`extra_targets()` の3箇所を修正するのをやめて、**1箇所の定義**で完結させたい。なぜなら、現状の3重管理は新 Cleaner 追加時の修正漏れリスクが高く、`LibraryLogsCleaner` のような特殊ケースがスプリントレビューでの見落とし原因になるからである。

## ビジネス価値
- 新 Cleaner 追加のコスト削減（3箇所 → 1箇所）
- `cmd_name!` マクロの30+バリアント重複を解消
- 特殊ケースパターンの撲滅（PBI-06 と合わせて main.rs の純化を完了）

## BDD受け入れシナリオ

```gherkin
Scenario: Cleaner 追加が1箇所の定義で完結する
  Given 新しい cleaner "FooCleaner" を追加するとき
  When define_cleaners! マクロに1行追加する
  Then コマンド名、説明文、dispatch_clean、targets 一覧が自動生成される
  And cmd_name! マクロの個別定義が不要
  And extra_targets() への追加も不要

Scenario: 既存の全 cleaner が引き続き動作する
  Given リファクタリング後も
  When 既存の全サブコマンドを実行する
  Then 全 cleaner が変更前と同じ動作をする

Scenario: targets サブコマンドの出力が変わらない
  Given sasurahime targets を実行するとき
  When リファクタリング後
  Then 43個の全 target が同じ順序・説明で表示される
```

## 受け入れ基準
- [ ] `cmd_name!` マクロが削除され、`define_cleaners!` に統合される
- [ ] `extra_targets()` 関数が削除または統合される
- [ ] 全 43+ cleaner のコマンド名が変更前と同じである
- [ ] `sasurahime targets` の出力が変更前と同一である
- [ ] `dispatch_clean` が正しく全 cleaner をディスパッチする

## テスト戦略（t_wadaスタイル）

### E2Eテスト（3）
- `sasurahime targets` の出力がリファクタリング前後で同一
- 既存の全 cleaner の CLI 呼び出しが正常に動作する
- 新 Cleaner 追加テスト（モック）で1行追加のみで完了することを確認

### 統合テスト（2）
- `cmd_name!` の削除後も `dispatch_command_name()` が正しい名前を返す
- `SUPPORTED_TARGETS` の内容が全 cleaner を含む

### 単体テスト（2）
- マクロの展開結果の検証（コンパイル時に保証されるため実質不要だが、リグレッション検出用に1件）
- 特殊 variant のディスパッチカバレッジ

## 実装アプローチ
- **安全第一**: リファクタリングは段階的に行う。まず `cmd_name!` 情報を `define_cleaners!` に吸収し、その後 `extra_targets()` の統合を検討
- **Outside-In**: E2E テストの出力をゴールデンファイルとして保存し、リファクタリング前後で diff を取る

## 見積もり
5 SP（マクロの大規模リファクタリング、8〜13日）

## 技術的考慮事項
- 依存関係: なし。純粋なコード再構成
- リスク: マクロの展開結果が複雑で、テストなしではリグレッションを見つけにくい
- PBI-06（LibraryLogs トレイト契約）の完了を前提とする（または並行作業可能）

## 実装者向け注記

### 現状の3重管理
```rust
// 1. define_cleaners! 内のバリアント定義（L210-300+）
// 2. cmd_name! マクロの個別定義（各バリアントに対応）
// 3. extra_targets() 関数のパターンマッチ（特殊 variant の手動ハンドリング）
```

### 修正方針
`cmd_name!` の定義を `define_cleaners!` マクロ内で生成する:
```rust
macro_rules! define_cleaners {
    ($(
        $(#[$variant_meta:meta])*
        $variant:ident : $cli_name:literal => $desc:expr ;
        ($factory:expr)
    ),+ $(,)?
    ) => {
        // CleanTarget enum（従来通り）
        // dispatch_clean（従来通り）
        // SUPPORTED_TARGETS（従来通り）
        
        // cmd_name! を排除し、コマンド名を直接マクロ内で定義
        impl CleanTarget {
            fn dispatch_command_name(&self) -> &'static str {
                match self {
                    $( CleanTarget::$variant { .. } => $cli_name, )*
                    _ => unreachable!()
                }
            }
        }
    };
}
```

### 付帯作業A: `exit_code() != 0` の重複を解消
Refactoring Evangelist の指摘: `main.rs` に `if result.exit_code() != 0 { std::process::exit(1); }` が **9回** 繰り返されている。

```bash
grep -c "exit_code() != 0" src/main.rs
# → 9
```

対応: `run_clean_target` に exit_code チェックを内包したラッパーを導入する:
```rust
fn run_and_exit_on_failure<F>(
    label: &str,
    cleaner_fn: F,
    dry_run: bool,
    reporter: &dyn ProgressReporter,
) -> anyhow::Result<CleanResult>
where
    F: FnOnce(bool, &dyn ProgressReporter) -> anyhow::Result<CleanResult>,
{
    let result = run_clean_target(label, cleaner_fn, dry_run, reporter)?;
    if result.exit_code() != 0 {
        std::process::exit(1);
    }
    Ok(result)
}
```

### 付帯作業B: `clean_cli_or_fallback` stderr 喪失
SRE/Ops Specialist の指摘: `src/cleaners/generic.rs:772-777` で huggingface-cli / pre-commit の非ゼロ終了時に stderr を捨てている。`bail!()` の前に stderr を出力する。詳細は PBI-05 の付帯作業A を参照。

### 落とし穴
- `cmd_name!` を使用している他の箇所がないか、コンパイルエラーで発見する（安全な削除方法）
- 特殊 variant（`LibraryLogs`, `Xcode`, `Ollama`, `DeviceSupport`, `Trash` など）は `dispatch_clean` から溢出している。これらは `dispatch_special` のような別関数にまとめるか、PBI-06 で処理する
- `run_and_exit_on_failure` 導入時は、既存の9箇所すべてを漏れなく置き換えること
- リファクタリング中は頻繁に `cargo build` してコンパイルが通ることを確認する

## Definition of Done
- [ ] `cmd_name!` マクロが削除されている
- [ ] `define_cleaners!` マクロがコマンド名も生成する
- [ ] `extra_targets()` またはその代替機構が整理されている
- [ ] `sasurahime targets` の出力が変更前と同一
- [ ] 全既存テストがパスする
- [ ] `cargo clippy -- -D warnings` が通る
- [ ] コードレビュー完了
