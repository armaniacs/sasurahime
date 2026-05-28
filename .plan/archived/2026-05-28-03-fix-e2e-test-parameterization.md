# PBI: E2E テストのパラメータ化 — "tool not found" テスト重複の解消

## ユーザーストーリー
デベロッパーとして、`cargo test` がより短時間で完了してほしい。なぜなら、17件のほぼ同一な "tool not found" E2E テストがバイナリ spawn オーバーヘッド（各〜200ms）を積み重ね、CI のフィードバックループを遅らせているからである。

## ビジネス価値
- `cargo test` 実行時間の短縮（推定: 3〜5秒削減）
- テストコードの重複排除による保守性向上
- 新 cleaner 追加時のテスト追加コスト削減

## BDD受け入れシナリオ

```gherkin
Scenario: ツール未インストールの cleaner がエラー0で終了する
  Given 特定のツールが PATH に存在しないとき
  When sasurahime clean <target> を実行する
  Then 終了コード0で成功する
  And 標準出力に "not found" または "skipping" が含まれる

Scenario: パラメータ化テストが全ての cleaner をカバーする
  Given 17件の cleaner が "tool not found" テストを必要とするとき
  When パラメータ化テストが1つのテスト関数で全 cleaner をカバーする
  Then 全 cleaner に対して同じアサーションが実行される
  And 個別テスト関数が削除されてもカバレッジが維持される
```

## 受け入れ基準
- [ ] 17件の個別 "not found" テスト関数が1〜3件のパラメータ化テストに置き換わる
- [ ] 全 cleaner（act, brew, bun, cocoa-pods, colima, conda, deno, device-support, docker, flutter, go, huggingface, library-logs, maven, mise, npm, ollama, orbstack, pip, pipx, poetry, pre-commit, sbt, simulator, terraform, tree-sitter, uv, volta, vscode-extensions）がカバーされる
- [ ] `cargo test` 実行時間が短縮している
- [ ] 既存と同じアサーション品質が維持される

## テスト戦略（t_wadaスタイル）

### E2Eテスト（リファクタリング対象）
- 現在17件ある個別テストをパラメータ化テストに統合する

### 統合テスト（2）
- `GenericCleaner::clean()` の tool-not-found パスを `MockRunner` でテスト
- `is_available()` が false を返す cleaner の detect スキップを確認

### 単体テスト（2）
- `CommandRunner.exists()` の false パス
- `run_clean_target` の tool-not-found ディスパッチ

## 実装アプローチ
- **Outside-In**: パラメータ化テストを先に書き、個別テストが全て削除可能であることを確認
- **Red-Green-Refactor**: 新テスト（Red）→ 旧テスト削除（Green, ただし新テストが通るまで削除しない）

## 見積もり
2 SP（テストリファクタリング、2〜3日）

## 技術的考慮事項
- 依存関係: `rstest` crate を `[dev-dependencies]` に追加する必要あり
- テスタビリティ: 高い。E2E テストのみのリファクタリング
- リスク: E2E テストはバイナリ spawn 形式なので、パラメータ化してもテスト分離は問題ない

## 実装者向け注記

### 現状コードの確認
```bash
grep -rn "not_found_exits_zero\|not_found_skips" tests/ | grep "fn "
# 17件以上が見つかる
```

### 現状のテストパターン（tests/generic.rs）
```rust
#[test]
fn clean_deno_not_found_skips() {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", "deno"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}"
    );
}
```
このパターンが17回以上繰り返されている。ツール名のみが異なる。

### 修正方針
1. `Cargo.toml` の `[dev-dependencies]` に `rstest = "0.22"` を追加
2. `tests/generic.rs` にパラメータ化テストを1つ追加:
```rust
use rstest::rstest;

#[rstest]
#[case("deno")]
#[case("docker")]
#[case("pipx")]
// ... 全 cleaner を列挙
fn clean_tool_not_found_skips(#[case] tool: &str) {
    let tmp = TempDir::new().unwrap();
    let output = sasurahime(tmp.path())
        .env("PATH", "/usr/bin:/bin")
        .args(["clean", tool])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("skipping") || stdout.contains("not found"),
        "stdout: {stdout}"
    );
}
```
3. 個別のテスト関数を削除

### 注意点
- 全テストファイルにまたがるため、`tests/generic.rs` 内のものに絞ってリファクタリングする（他のテストファイルは cleaner 固有の追加アサーションがある可能性）
- `tests/generic.rs` 内の clean_volta_not_found, clean_sbt_not_found, clean_tree_sitter_not_found も同一パターンなので統合対象
- 異なるアサーションパターンがあるもの（例: stdout に "skipping" のみ期待するケース）は別パラメータ化テストにするか、個別に残す

### 付帯作業: VerboseGuard の未移行テストを修正
DX Advocate の指摘: `device_support.rs`, `generic.rs`, `cargo.rs` のテストで raw `TEST_LOCK` を使い `VerboseGuard` を使っていない箇所がある。
```bash
grep -n "TEST_LOCK" src/cleaners/device_support.rs src/cleaners/generic.rs src/cleaners/cargo.rs
```
該当箇所を `VerboseGuard::new()` に置き換える（`src/test_helpers.rs:174`）。このガードは自動的に TEST_LOCK を取得する。

## Definition of Done
- [ ] 全 cleaner の "not found" 動作がパラメータ化テストでカバーされている
- [ ] 個別の重複テスト関数が削除されている
- [ ] `cargo test` 全テストがパスする
- [ ] CI パイプラインのテスト時間が改善している
- [ ] コードレビュー完了
