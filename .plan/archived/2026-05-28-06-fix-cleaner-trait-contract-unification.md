# PBI: LibraryLogsCleaner の Cleaner トレイト契約統一

## ユーザーストーリー
デベロッパーとして、`LibraryLogsCleaner` が他の全 cleaner と同様に `Cleaner` トレイト経由で操作できるようにしてほしい。なぜなら、現在 `clean_all()` という非トレイトメソッドが `main.rs` で特別扱いされており、トレイトの抽象化を破壊し、TUI や `--yes` モードから `--all` フラグ相当の動作を利用できないからである。

## ビジネス価値
- Cleaner トレイト契約の純化（LibraryLogs だけがルールを破っている）
- TUI / `--yes` モードからの `--all` 相当動作の利用が可能に
- 新 Cleaner 追加時の "特殊ケース" パターンの撲滅

## BDD受け入れシナリオ

```gherkin
Scenario: LibraryLogsCleaner.clean() が --all モードをサポートする
  Given LibraryLogsCleaner が複数のログディレクトリを管理しているとき
  When clean() に特別なオプション（--all）を渡す
  Then 全ログディレクトリが chflags + 削除される
  And 結果が CleanResult として返る

Scenario: トレイト経由で clean() が呼ばれる
  Given Cleaner トレイト経由で LibraryLogsCleaner を参照しているとき
  When clean(dry_run, reporter) を呼び出す
  Then 通常モード（対話的選択）で動作する

Scenario: CLI の --all フラグがトレイト経由で動作する
  Given sasurahime clean library-logs --all を実行するとき
  When コマンドがディスパッチされる
  Then 特別なディスパッチコード（main.rs L935-955）を経由せず、通常の dispatch_clean 経由で動作する
```

## 受け入れ基準
- [ ] `LibraryLogsCleaner::clean()` が `all: bool` 相当のパラメータをサポートする（または別の方法で clean_all の機能を提供する）
- [ ] `clean_all()` メソッドが削除される（内部のロジックが `clean()` に統合される）
- [ ] `main.rs` の `CleanTarget::LibraryLogs` 特別ケースが削除される
- [ ] TUI / `--yes` モードでも `--all` 相当の動作が利用可能

## テスト戦略（t_wadaスタイル）

### E2Eテスト（2）
- `sasurahime clean library-logs --dry-run --all` が全エントリを表示する
- `sasurahime clean library-logs --dry-run`（--all なし）が対話的選択モードで動作する

### 統合テスト（3）
- `LibraryLogsCleaner::clean()` に `--all` フラグを渡した場合の動作
- `dispatch_clean` 経由で LibraryLogs が正常に動作する
- TUI の `--all` 相当のオプションが正しく渡される

### 単体テスト（4）
- Test Experts 追加済み: `clean_all_processes_all_entries_without_selection`（clean_all が全エントリ処理）
- Test Experts 追加済み: `clean_via_trait_processes_entries_with_interactive`（trait 経由 dry-run）
- 必要に応じて追加: `clean()` 内の `all` パラメータによる分岐
- 必要に応じて追加: `scan()` の結果が正しく統合される

## 実装アプローチ
- **オプション1**（推奨）: `Cleaner` トレイトに `fn clean_with_opts(&self, dry_run: bool, reporter: &dyn ProgressReporter, opts: &CleanOpts) -> Result<CleanResult>` を追加（デフォルト実装は `clean()` を呼ぶ）。`LibraryLogsCleaner` のみオーバーライド。
- **オプション2**: `LibraryLogsCleaner` の `clean()` に `--all` を常時チェックするロジックを追加（グローバルフラグ方式だが AtomicBool 追加が必要）
- **オプション3**: `clean_all()` のロジックを `clean()` に統合し、scan 結果が1件かつ確認不要なら自動的に全削除する

## 見積もり
3 SP（トレイト設計変更、特殊ケース削除、テスト含む、3〜5日）

## 技術的考慮事項
- 依存関係: なし
- テスタビリティ: 高い（MockRunner 経由）
- **オプション1を推奨**: トレイトにオプション引数用の enum/struct を追加するのが最もクリーン。`#[non_exhaustive]` で将来の拡張性も確保

## 実装者向け注記

### 現状コードの確認
```bash
# main.rs の特殊ケース
grep -n "LibraryLogs\|clean_all" src/main.rs

# LibraryLogsCleaner のトレイト実装
grep -n "impl Cleaner\|fn clean\|fn clean_all" src/cleaners/library_logs.rs
```

### 現状の問題
- `main.rs` L930-956: `CleanTarget::LibraryLogs` が特別扱いされ、`all` フラグの有無で `clean_all()` と `clean()` を呼び分けている
- `dispatch_clean`（マクロ生成）は `LibraryLogs` に到達しない（`_` → `unreachable!()` で落ちる）

### 修正方針（オプション1の詳細）
```rust
// src/cleaner.rs に追加
#[non_exhaustive]
pub struct CleanOptions {
    pub all: bool,
    // 将来的に追加可能: pub recursive: bool, pub force: bool, ...
}

impl Default for CleanOptions {
    fn default() -> Self { Self { all: false } }
}

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &'static str;
    fn detect(&self) -> ScanResult;
    fn clean(&self, dry_run: bool, reporter: &dyn ProgressReporter) -> Result<CleanResult>;
    
    // 追加: オプション付きクリーン
    fn clean_with_opts(&self, dry_run: bool, reporter: &dyn ProgressReporter, opts: &CleanOptions) -> Result<CleanResult> {
        // デフォルト: opts を無視して clean() を呼ぶ
        self.clean(dry_run, reporter)
    }
}
```

### 落とし穴
- `clean_with_opts` のデフォルト実装が opts を無視するため、`LibraryLogsCleaner` 以外の cleaner には影響なし
- TUI から opts を渡すには `dialoguer::MultiSelect` の結果を `CleanOptions` に変換する処理が必要
- `main.rs` の `dispatch_clean` も opts を受け取れるよう修正すること

## Definition of Done
- [ ] `LibraryLogsCleaner::clean_all()` が削除され、代わりに `clean_with_opts` のオーバーライドで機能が提供される
- [ ] `main.rs` の `CleanTarget::LibraryLogs` 特殊ケースが削除される
- [ ] `dispatch_clean` が opts を受け取れるよう拡張される
- [ ] TUI / `--yes` モードでも `--all` 相当の動作が利用可能
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
