# PBI: GradleCleaner / JetBrainsCleaner の Trash モード対応

## ユーザーストーリー
デベロッパーとして、`sasurahime clean gradle` や `sasurahime clean jetbrains` を実行したときに、他の cleaner と同様に macOS Trash 経由で削除してほしい。なぜなら、Gradle/JetBrains だけが `fs::remove_dir_all` で永久削除しており、グローバルの `trash_mode` 設定を尊重していないからである。

## ビジネス価値
- 全 cleaner 間での Trash モード一貫性の確保
- 誤削除時の復元可能性を Gradle/JetBrains にも提供
- Red Team / Blue Team / Data Integrity Expert の3名重複指摘に対応

## BDD受け入れシナリオ

```gherkin
Scenario: Gradle キャッシュ削除が Trash に移動される
  Given ~/.gradle/caches に古いバージョンキャッシュが存在する
  When sasurahime clean gradle を実行する（--permanent なし）
  Then キャッシュが macOS Trash に移動される
  And CleanResult.uses_trash が true を返す

Scenario: Gradle キャッシュ削除で --permanent が指定された場合
  Given ~/.gradle/caches に古いバージョンキャッシュが存在する
  When sasurahime clean gradle --permanent を実行する
  Then キャッシュが永久削除される（Trash を経由しない）
  And CleanResult.uses_trash が false を返す

Scenario: JetBrains キャッシュ削除が Trash に移動される
  Given ~/Library/Caches/JetBrains に古い IDE キャッシュが存在する
  When sasurahime clean jetbrains を実行する（--permanent なし）
  Then キャッシュが macOS Trash に移動される
  And CleanResult.uses_trash が true を返す

Scenario: dry-run では Trash にも永久削除にも影響しない
  Given どのようなキャッシュ状態でも
  When sasurahime clean gradle --dry-run を実行する
  Then ファイルは削除されない
  And メッセージは dry-run であることを示す
```

## 受け入れ基準
- [ ] 既存の "tool not found" / "nothing to clean" パスは変更不要（早期 return のため）
- [ ] `uses_trash: true` のテストが既存テストを置き換える（現在 `uses_trash: false` をアサート）
- [ ] `clean()` 内の `fs::remove_dir_all` が `crate::trash::delete_path` に置き換わる
- [ ] `set_trash_mode(true)` が Gradle/JetBrains の削除に反映される

## テスト戦略（t_wadaスタイル）

### E2Eテスト（1）
- `sasurahime clean gradle --dry-run` が正しく dry-run として動作する

### 統合テスト（3）
- GradleCleaner が `trash::delete_path` を呼び出すことの検証（MockRunner + 依存性注入）
- JetBrainsCleaner が `trash::delete_path` を呼び出すことの検証
- `uses_trash` が `set_trash_mode` によって切り替わることの検証

### 単体テスト（6 = Test Experts 追加済み）
- `gradle_detect_does_not_delete`: detect 実行後もファイルが存在する
- `gradle_clean_uses_trash_true`（現在の `_false` から置き換え）: uses_trash が true
- `gradle_clean_dry_run_does_not_delete`: dry-run で削除されない
- JetBrains 同等3件

## 実装アプローチ
- **Outside-In**: E2Eテスト（期待動作の確認）→ 単体テスト（既存）→ 実装変更
- **Red-Green-Refactor**: 既存テストが `uses_trash: false` を期待しているので、まずアサーションを反転して Red を確認、実装変更後に Green

## 見積もり
1 SP（1人日未満の小規模変更）

## 技術的考慮事項
- 依存関係: なし（`crate::trash::delete_path` は既存）
- テスタビリティ: 高い。`crate::trash::delete_path` はグローバルモード（`set_trash_mode`）で動作
- 注意: Test Experts が `uses_trash: false` のテストを追加済み。これらを `true` に変更する必要がある

## 実装者向け注記

### 現状コードの確認
```bash
grep -n "remove_dir_all\|uses_trash" src/cleaners/gradle.rs
```

### 変更箇所
`src/cleaners/gradle.rs`:
- L92: `uses_trash: false` → `crate::trash::is_trash_mode()`
- L110: `fs::remove_dir_all(path)?;` → `crate::trash::delete_path(path)?;` + エラーハンドリング既存パターンに合わせる
- L118: `uses_trash: false` → `crate::trash::is_trash_mode()`
- L223, L241, L249: JetBrainsCleaner も同様に3箇所

### 既存パターン（他 cleaner の実装例: cargo.rs L97-105）:
```rust
if let Err(e) = crate::trash::delete_path(&reg) {
    if crate::cleaner::is_skippable_error(&e) {
        skipped.push(crate::cleaner::SkippedEntry {
            path: reg.to_path_buf(),
            reason: format!("{e:#}"),
        });
    } else {
        return Err(e);
    }
} else {
    freed += size;
}
```

### Trash モードの仕組み
Trash モードは `src/trash.rs` のグローバル `TRASH_MODE: AtomicBool` で制御される。
- `crate::trash::set_trash_mode(bool)` で設定
- `crate::trash::is_trash_mode()` で参照
- `crate::trash::delete_path(path)` はこのグローバルモードを自動的に参照する（Trash または permanent を切り替える）
- 設定ファイル `config.toml` の `trash_mode` は `main.rs` 起動時に `set_trash_mode()` に反映される

よって Gradle/JetBrains の修正は単に `fs::remove_dir_all` → `crate::trash::delete_path` に置き換えるだけで OK。`delete_path` がグローバルモードを自動反映する。

### テスト修正
- `tests/gradle.rs` 内の `*clean_uses_trash_false` テストを `*clean_uses_trash_true` に変更
- アサーション: `assert!(!result.uses_trash)` → `assert!(result.uses_trash)`
- `trash_mode = true` の状態でテストすること（デフォルト）

### 落とし穴
- `fs::remove_dir_all` の代わりに `trash::delete_path` を使うと、削除に失敗した場合のエラーハンドリングパターンが異なる。cargo.rs のパターン（`is_skippable_error` による分岐）に統一すること
- `set_trash_mode(false)` でも `delete_path` は動作し、permanent 削除になることを確認
- `crate::context::is_trash_mode()` は存在しない。正しい API は `crate::trash::is_trash_mode()` である

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `cargo test` 全511+テストがパスする
- [ ] GradleCleaner と JetBrainsCleaner の両方が Trash モードをサポートする
- [ ] 既存テストの `uses_trash: false` アサーションが適切に更新されている
- [ ] コードレビュー完了
