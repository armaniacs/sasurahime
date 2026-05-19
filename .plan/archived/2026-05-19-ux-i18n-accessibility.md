# PBI: Interactive Mode UX/i18n/Accessibility Improvements

## ユーザーストーリー
日本語ユーザーとスクリーンリーダー利用者として、確認プロンプトが日本語でも動作し、Unicodeチェックマークが適切に読み上げられることがほしい、なぜなら現在は英語 "y/N" のみ対応で、チェックマーク ✓ はスクリーンリーダーで適切に読まれないから。

## ビジネス価値
日本語環境を含む多言語ユーザーがストレスなく使用できる。アクセシビリティ対応により、スクリーンリーダー利用者も安心して操作できる。`--yes` モードで Xcode の動作状態が通知されることで、予期せぬ動作を防げる。

## BDD受け入れシナリオ

```gherkin
Scenario: 日本語環境で "yes" 確認が "y" だけでなく "Y" でも動作する
  Given 端末が日本語ロケールに設定されている
  When  "y" または "Y" が入力される
  Then  処理が続行される
  And   英語と同じ挙動になる

Scenario: スクリーンリーダーが完了メッセージを正しく読み上げる
  Given スクリーンリーダーが有効な環境
  When  with_spinner が完了する
  Then  出力に "チェックマーク" ではなく完了した処理内容が含まれる

Scenario: --yes モードで Xcode が動作中の場合に通知される
  Given Xcode が動作している
  When  sasurahime --yes が実行される
  Then  stderr に "Xcode is running" の通知が表示される
  And   処理は中断されずに続行される
```

## 受け入れ基準
- [ ] 確認プロンプトの "y" 判定がロケールに依存しない（`eq_ignore_ascii_case` は既に対応済み、i18n の指摘は README への記載のみで良いか確認）
- [ ] 完了メッセージにスクリーンリーダー互換の表現が含まれる
- [ ] `--yes` モードで Xcode が動作中の場合、stderr に注意喚起が表示される
- [ ] TUI の端末チェックがより緩和されている（`is_terminal()` に依存しない代替手段）

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `--yes` モードで Xcode 動作中の通知が stderr に出力されることの確認

### 統合テスト
- 確認プロンプトの YES/NO 判定のテスト
- Xcode 動作検出のモックテスト

### 単体テスト
- アクセシビリティ対応の出力フォーマットテスト

## 実装アプローチ

### 1. Xcode 動作中の --yes 通知（Ethics & Bias 対応）
`src/main.rs` の `CleanTarget::Xcode` アーム内で、`is_xcode_running()` が true かつ `--yes` モードの場合に stderr へ注意喚起を表示する。`dry_run` フラグと `cli.yes` フラグは別なので、`--yes` かどうかを関数に伝える必要がある → `CleanTarget::Xcode` に `yes: bool` を追加するか、`main.rs` の match アーム内で判定する。

```rust
CleanTarget::Xcode { dry_run } => {
    let cleaner = cleaners::xcode::XcodeCleaner::new(&home, Box::new(SystemCommandRunner));
    // --yes mode: notify if Xcode is running (Ethics & Bias)
    if cleaner.is_xcode_running() {
        eprintln!("Note: Xcode is running. Cleaning DerivedData anyway (--yes mode).");
    }
    let result = crate::progress::with_spinner("Cleaning xcode...", || cleaner.clean(dry_run))?;
    println!("Freed: {}", format::format_bytes(result.bytes_freed));
}
```

### 2. アクセシビリティ対応（Accessibility Advocate 対応）
`src/progress.rs` の完了メッセージを `✓` から `"(done)"` または `"[OK]"` などスクリーンリーダーが読み上げ可能な表現に変更。

```rust
// Before: eprintln!("{msg} ✓");
// After:  eprintln!("{msg} [OK]");
```

または、`indicatif` の finish_with_message を使い、スピナーが端末に描画される場合は ✓ を、パイプの場合は `[OK]` を表示する。

シンプルに `[OK]` 固定とする（12名中2名の指摘であり、✓ 視認性を重視するユーザーよりアクセシビリティを優先）。

### 3. TUI 端末チェックの改善（Accessibility Advocate 対応）
現在 `run_interactive()` は `is_terminal()` でチェックし、非 TTY だと終了する。アクセシビリティ環境（スクリーンリーダーによる仮想端末）では `is_terminal()` が false になることがある。

→ `is_terminal()` チェックを維持しつつ、エラーメッセージに `--yes` を使うよう案内するテキストを追加する（現在のメッセージに既に含まれているので十分）。

- **Outside-In**: E2Eテストから開始
- **Red-Green-Refactor**: 各修正ごとにテスト追加・実行

## 見積もり
2ストーリーポイント

## 技術的考慮事項
- 依存関係: なし
- テスタビリティ: Xcode 検出はモック Runner でテスト可能（既存テストにパターンあり）
- 注意点: ✓ → [OK] の変更は既存テストのアサーションにも影響するので、修正後に `cargo test` を必ず実行

## Definition of Done
- [ ] `--yes` + Xcode 動作中の通知が実装されている
- [ ] 完了メッセージがスクリーンリーダー互換になった
- [ ] TUI 端末チェックのエラーメッセージが改善されている
- [ ] 全テストパス
- [ ] clippy / fmt clean
