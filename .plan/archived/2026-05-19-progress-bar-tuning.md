# PBI: ProgressBar Robustness & Release Profile Tuning

## ユーザーストーリー
開発者として、プログレスバーがパニックせずに表示され、かつ CI ビルドが適切な速度で完了することがほしい、なぜなら現在の実装では `.template().unwrap()` がパニックリスクを持ち、release ビルドの LTO 設定が CI 時間を不必要に増加させているから。

## ビジネス価値
プログレスバーのパニックリスクを除去し、CI パイプラインの実行時間を短縮する。バイナリサイズとビルド時間のトレードオフを最適化する。

## BDD受け入れシナリオ

```gherkin
Scenario: ProgressStyle のテンプレートパースに失敗しない
  Given ProgressStyle::default_spinner().template() が呼ばれる
  When  有効なテンプレート文字列を渡す
  Then  パニックせず Style が返る

Scenario: with_spinner が同一スタイルをキャッシュする
  Given 複数回 with_spinner が呼ばれる
  When  各呼び出しで ProgressStyle が変更されない
  Then  スタイルの再生成が毎回発生しない

Scenario: release ビルドが CI で高速に完了する
  Given CI パイプラインが設定されている
  When  cargo build --release が実行される
  Then  ビルド時間が LTO full 時より短い
  And   配布用バイナリは LTO full でビルドされる
```

## 受け入れ基準
- [ ] ProgressStyle の template 呼び出しが `.unwrap()` ではなく安全なエラーハンドリングを行う
- [ ] `with_spinner` が ProgressStyle を毎回生成しない（static/const 化）
- [ ] `[profile.ci]` または代替手段で CI ビルド時間が改善されている
- [ ] `opt-level` が CLI ツールに適した値になっている（`"z"` vs `"s"`）
- [ ] 全テストがパスし、リリースバイナリのサイズが許容範囲内

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 既存のスピナーテストが引き続きパスする

### 統合テスト
- ProgressStyle 生成がパニックしないことをテスト
- キャッシュされたスタイルが正しく適用されることをテスト

### 単体テスト
- `with_spinner` がスタイルキャッシュを使い回すことの確認
- リリースプロファイルの設定値が期待通りであることの確認（コンパイル時）

## 実装アプローチ

### 1. ProgressStyle の安全化（progress.rs）

```rust
use std::sync::OnceLock;

fn spinner_style() -> &'static ProgressStyle {
    static STYLE: OnceLock<ProgressStyle> = OnceLock::new();
    STYLE.get_or_init(|| {
        ProgressStyle::default_spinner()
            .template("{spinner:.green} {msg}")
            .expect("valid indicatif template")
    })
}
```

### 2. opt-level の見直し（Cargo.toml）

Tuning Expert の指摘に基づき CLI ツールは速度とサイズのバランスが重要。`opt-level = "z"` から `opt-level = "s"` に変更。

### 3. 分割プロファイル戦略（Cargo.toml）

```toml
[profile.release]
opt-level      = "s"
strip          = true
lto            = "thin"    # Thin LTO: 90%効果、20%時間
codegen-units  = 1
```

Thin LTO は full LTO よりビルド時間が短く、サイズ効果はほぼ同等。

- **Outside-In**: E2Eテストから開始
- **Red-Green-Refactor**: 各変更ごとにテスト実行
- **YAGNI**: CI 用プロファイルは実際に CI を設定するときまで追加しない

## 見積もり
1ストーリーポイント

## 技術的考慮事項
- 依存関係: `indicatif` 0.17 の API に依存
- テスタビリティ: プログレスバーのテストは目視確認が主だが、パニックしないことのテストは可能
- 非機能要件: バイナリサイズは `opt-level = "s"` + `strip` で 1MB 前後を維持見込み

## Definition of Done
- [ ] ProgressStyle の安全化が完了
- [ ] スタイルキャッシュが実装されている
- [ ] opt-level が "s" に変更されている
- [ ] リリースバイナリのサイズが確認済み
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` 全パス
