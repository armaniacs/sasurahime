# PBI: CleanTarget Match Arm Refactoring

## ユーザーストーリー
メンテナーとして、新しいクリーンターゲットを追加する際に14箇所の変更を必要としない仕組みがほしい、なぜなら現在の重複が追加コストを増やし、バグの原因になっているから。

## ビジネス価値
新しいキャッシュターゲットの追加工数が削減される。追加時のミス（あるターゲットだけスピナーが漏れる等）が防止される。コードの可読性が向上する。

## BDD受け入れシナリオ

```gherkin
Scenario: 既存の全14ターゲットがリファクタリング後も同じ動作をする
  Given リファクタリング前の全14ターゲットの clean サブコマンドが存在する
  When  リファクタリングを適用する
  Then  各ターゲットの clean 動作が変わらない
  And   全テストがパスする

Scenario: 新しいターゲットを追加する場合の変更箇所が1箇所だけになる
  Given 新しいツール "my-tool" の Cleaner 実装が存在する
  When  CleanTarget に MyTool バリアントを追加する
  Then  match アームを1つ追加するだけで clean が機能する
  And   スピナー表示と freed 表示が自動的に適用される
```

## 受け入れ基準
- [ ] `src/main.rs` の14個の CleanTarget match arm が共通関数に抽出されている
- [ ] 全テストがリファクタリング前後で同一結果になる
- [ ] `cargo clippy -- -D warnings` が通る
- [ ] `cargo fmt --check` が通る

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 全14ターゲットの dry-run がリファクタリング後も動作する（既存テストでカバー済み）

### 統合テスト
- 共通関数 `run_clean_target` が Cleaner の clean() を正しく呼び出し、結果を返すことを確認

### 単体テスト
- 共通関数の戻り値テスト（with_spinner でラップした結果が正しく返る）
- エラーハンドリングのテスト（clean() がエラーを返した場合の伝播）

## 実装アプローチ

### 抽出する共通関数

```rust
/// Runs a single-target clean with spinner and prints freed bytes.
fn run_clean_target<F>(label: &str, cleaner_fn: F, dry_run: bool) -> Result<()>
where
    F: FnOnce(bool) -> Result<CleanResult>,
{
    let result = crate::progress::with_spinner(
        &format!("Cleaning {label}..."),
        || cleaner_fn(dry_run),
    )?;
    println!("Freed: {}", format::format_bytes(result.bytes_freed));
    Ok(())
}
```

### 各アームの呼び出し例（14個→1行ずつ）

```rust
CleanTarget::Uv { dry_run } => {
    let cleaner = || cleaners::uv::UvCleaner::new(&home, Box::new(SystemCommandRunner)).clean;
    run_clean_target("uv", cleaner, dry_run)
}
```

- **Outside-In**: 既存の E2E test が green であることを確認してからリファクタリング
- **Red-Green-Refactor**: 共通関数を抽出後、全テストが green であることを確認
- **リファクタリング**: 非機能変更のみ。振る舞いは一切変えない

## 見積もり
1〜2ストーリーポイント

## 技術的考慮事項
- 依存関係: なし（main.rs のみの変更）
- テスタビリティ: 既存の E2E テストがリグレッションを検出する
- 非機能要件: パフォーマンス影響なし（コンパイル時に解決）

## Definition of Done
- [ ] 全14アームが共通関数を使用している
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` 全パス
- [ ] `cargo fmt --check` 全パス
- [ ] コードレビュー完了
