# PBI: 構造化ログと監査証跡の導入

## ユーザーストーリー
デベロッパーとして、`sasurahime` が削除操作の監査証跡を提供してほしい。なぜなら、`--permanent` フラグ使用時にどのファイルが削除されたかのログがなく、誤操作時の原因特定が不可能だからである。また、SRE として、機械可読なログ形式がないため CI/CD パイプラインでの自動アラート発報が困難である。

## ビジネス価値
- `--permanent` 削除の追跡可能性（監査証跡）
- 機械可読ログ（JSON）による自動運用監視
- `println!` / `eprintln!` 混在による出力の不統一を解消

## BDD受け入れシナリオ

```gherkin
Scenario: clean 操作がログに記録される
  Given 任意の cleaner で sasurahime clean <target> を実行したとき
  When bytes_freed > 0 の削除が発生する
  Then ログファイルに INFO レベルで cleaner 名・削除サイズが記録される
  And --verbose 時は削除ファイルパスのリストも記録される

Scenario: --permanent 削除が特別な警告ログとして記録される
  Given --permanent フラグを使用して削除するとき
  When 削除が実行される
  Then ログに WARN レベルで "permanent delete" と記録される
  And 削除ファイルパスが個別に記録される

Scenario: エラーが適切なログレベルで記録される
  Given 削除中にスキップ可能なエラーが発生したとき
  When is_skippable_error が true を返す
  Then WARN レベルでスキップ理由が記録される
  And プログラムは継続する

Scenario: ログレベルが環境変数で制御可能
  Given RUST_LOG=debug が設定されているとき
  When 任意の操作を実行する
  Then DEBUG レベルの詳細ログが出力される
```

## 受け入れ基準
- [ ] `env_logger` または `tracing` が導入され、`println!` / `eprintln!` が適切なログマクロに置き換わる
- [ ] `--permanent` 削除時にファイルパス単位の監査証跡が残る
- [ ] `RUST_LOG` 環境変数でログレベルが制御可能
- [ ] 既存のユーザー向け出力（`println!` の scan 結果表など）は維持される（ログは stderr に出力し、stdout のユーザー出力と分離）
- [ ] 全既存テストがパスする

## テスト戦略（t_wadaスタイル）

### E2Eテスト（2）
- `--permanent` 削除時にログが出力されることの確認
- `RUST_LOG=debug` で詳細ログが出力されることの確認

### 統合テスト（3）
- `CleanResult` に削除ファイルパスリストが正しく設定される
- ログレベルによる出力制御
- 標準出力（ユーザー向け）とログ出力（stderr）の分離

### 単体テスト（4）
- ログメッセージのフォーマット
- 監査証跡エントリの生成
- ファイルパスリストの最大長制御（過剰なメモリ使用を防止）
- エラーログのレベル分類

## 実装アプローチ
- **段階的導入**: まず `log` crate を導入し `eprintln!` を置き換える。その後 `CleanResult` にパスリストを追加
- **リスク回避**: ログ導入による stdout 出力の重複を防ぐため、`println!`（scan 結果、freed 表示）は維持し、進行状況や警告のみログに移行

## 見積もり
5 SP（クレート追加、全 cleaner の出力書き換え、テスト含む、5〜8日）

## 技術的考慮事項
- 依存関係: `log = "0.4"` + `env_logger = "0.11"` を追加
- または `tracing = "0.1"` + `tracing-subscriber = "0.3"`（より高機能だがオーバーキルの可能性）
- stdout と stderr の使い分けを厳守:
  - stdout: `println!`（scan 結果表、freed バイト数、dry-run メッセージ）— 維持
  - stderr: `eprintln!` → `log::warn!` / `log::info!`（進行状況、警告、エラー）— 書き換え

## 実装者向け注記

### 現状の出力パターン
```bash
# 全 cleaner の出力パターンを把握
grep -rn "println!" src/ --include="*.rs" | grep -v "test\|#[cfg(test)" | wc -l
grep -rn "eprintln!" src/ --include="*.rs" | grep -v "test\|#[cfg(test)" | wc -l
```

### 分類基準（どの出力をログに移行するか）
| 出力先 | 内容 | アクション |
|--------|------|-----------|
| `println!` | dry-run の "would remove" 行 | 維持（ユーザー向け） |
| `println!` | scan 結果表 | 維持（comfy_table） |
| `println!` | "Freed: X B" | 維持 |
| `eprintln!` | "Warning: ..." | → `log::warn!` |
| `eprintln!` | "Note: ..." (iOS backup, Trash) | → `log::info!` または維持 |
| `eprintln!` | "Skipped: ..." | → `log::warn!` |
| `println!` | "[cleaner] removed: ..." | → `log::info!`（verbose時のみ `log::debug!`）|

### CleanResult へのパスリスト追加
```rust
pub struct CleanResult {
    pub name: &'static str,
    pub bytes_freed: u64,
    pub uses_trash: bool,
    pub skipped: Vec<SkippedEntry>,
    // 追加: 削除に成功したファイルパス（--permanent 時に必須）
    pub deleted_paths: Vec<PathBuf>,
}
```

### 付帯作業A: `clean_cli_or_fallback` の stderr 喪失を修正
SRE/Ops Specialist の指摘: `src/cleaners/generic.rs:772-777` の `clean_cli_or_fallback` は外部ツール（huggingface-cli, pre-commit）の非ゼロ終了時に `bail!()` で exit code のみ伝播し stderr を捨てている。一方、同じファイルの `CommandWithDetectDir` ブランチ（L571-582）では stderr を `eprintln!` している。

修正: L772-777 で `bail!()` する前に stderr をログに出力するよう変更する。
```rust
// 修正前:
bail!("{tool} exited with code {exit_code}");

// 修正後:
log::warn!("{} exited with code {}: {}", tool, exit_code, String::from_utf8_lossy(&output.stderr));
// bytes_freed: size_before の CleanResult を返す（処理継続可能にする）
```

### 付帯作業B: デッドコードのクリーンアップ（`#[allow(dead_code)]`）
Maintainability Guardian の指摘: 以下の公開アイテムが `#[allow(dead_code)]` で覆われており、意図が不明瞭。
- `src/cleaner.rs:12` — `ScanStatus::PermissionDenied`（未使用バリアント）
- `src/cleaner.rs:60,63` — `CleanResult.name`, `CleanResult.uses_trash`（未使用フィールド）
- `src/cleaner.rs:78-82` — `LARGE_TRASH_THRESHOLD_BYTES`, `format_trash_warning()`, `format_large_trash_warning()`

対応方針:
- 本当に不要なら削除する
- 将来の使用予定があるなら `#[allow(dead_code)]` にコメントで理由を明記（例: `// TODO: used by TUI in PBI-008`）
- `uses_trash` は `CleanResult` の公開フィールドとして外部クレートからの参照を想定して維持。コメントを追加

### 落とし穴
- `println!` を `log::info!` に置き換えると、`env_logger` のデフォルト設定では出力されない（`RUST_LOG=info` が必要）。既存の動作との互換性に注意
- scan 結果の comfy_table は stdout 出力が前提なので触らない
- テストのアサーションが stdout/stderr に依存している場合がある（`tests/interactive.rs` など）。テストの修正も合わせて行うこと

## Definition of Done
- [ ] `log` + `env_logger` が導入され、ビルドが通る
- [ ] 全 `eprintln!` が適切なログレベルに置き換わっている
- [ ] `RUST_LOG` によるレベル制御が動作する
- [ ] `CleanResult` に `deleted_paths` フィールドが追加されている
- [ ] `--permanent` + `--verbose` 時に削除ファイルパスがログに出力される
- [ ] `cargo test` 全テストがパスする
- [ ] コードレビュー完了
