# PBI: CargoCleaner / MiseCleaner の detect/clean 結果キャッシュ

## ユーザーストーリー
デベロッパーとして、`sasurahime scan` 後の `sasurahime clean cargo` や `sasurahime clean mise` が二度目の HOME walkdir を実行しないでほしい。なぜなら、`detect()` で走査した結果（target ディレクトリ一覧や .mise.toml のピン情報）を `clean()` が再利用せず、同じ I/O 処理を重複して実行し、特に大規模プロジェクトで顕著なレイテンシを生むからである。

## ビジネス価値
- scan → clean フローでの I/O 半減（cargo: detect+clean の二重 walkdir、mise: detect+clean の二重 `mise ls --current` + walkdir）
- MacBook バッテリー消費の低減
- TUI レスポンス向上（scan 後すぐ clean するユーザーフローで恩恵）

## BDD受け入れシナリオ

```gherkin
Scenario: detect 後に clean が結果を再利用する
  Given CargoCleaner.detect() が target ディレクトリ一覧を計算したとき
  When CargoCleaner.clean() を呼び出す
  Then detect() と同じ target 一覧を使用する（再 walkdir しない）
  And 削除後の bytes_freed が正しく計算される

Scenario: キャッシュがない場合（clean() 単独実行）
  Given detect() が事前に呼ばれていないとき
  When CargoCleaner.clean() を呼び出す
  Then 従来通り walkdir を実行する（機能低下なし、後方互換性）

Scenario: MiseCleaner も同様にキャッシュを再利用する
  Given MiseCleaner.detect() が unused versions 一覧を計算したとき
  When MiseCleaner.clean() を呼び出す
  Then 再度 mise ls --current を実行せず detect の結果を使用する
```

## 受け入れ基準
- [ ] `detect()` 後の `clean()` で再 walkdir / 再 `mise ls --current` が発生しない
- [ ] `clean()` を単独で呼んだ場合（detect なし）は従来通り動作する
- [ ] キャッシュの有効期限（1回のプロセス内のみで OK）が適切に管理される
- [ ] 全既存テストがパスする
- [ ] parallel テストでキャッシュの競合が発生しない

## テスト戦略（t_wadaスタイル）

### E2Eテスト（2）
- clean cargo で二重 walkdir が発生しないことを walkdir の呼び出し回数で検証（MockRunner で計測）
- clean mise で mise ls --current が1回のみ呼ばれることを検証

### 統合テスト（4）
- CargoCleaner: detect 後に clean → find_target_dirs が1回だけ呼ばれる
- CargoCleaner: clean 単独 → find_target_dirs が1回呼ばれる（通常動作）
- MiseCleaner: detect 後に clean → scan_pinned_versions が1回だけ呼ばれる
- MiseCleaner: clean 単独 → scan_pinned_versions が1回呼ばれる

### 単体テスト（3）
- CachedWalker のキャッシュ有無の判定
- キャッシュクリアのタイミング（削除後はクリア）
- 並列アクセス安全性

## 実装アプローチ
- **ハッピーパス優先**: まず cargo の find_target_dirs キャッシュ、次に mise の scan_pinned_versions キャッシュ
- ただし Cleaner trait にキャッシュ用フィールドを追加するのは避け、各 cleaner 内部に `OnceCell` または `Option<Vec<...>>` で保持する

## 見積もり
3 SP（cleaner 個別対応、テスト含む、3〜5日）

## 技術的考慮事項
- 依存関係: なし（`std::cell::OnceCell` または `parking_lot::OnceCell` を使用）
- Cleaner trait への変更は最小限にする。内部キャッシュ戦略を推奨
- 注意: `#[cfg(test)]` の MockRunner を使うテストではキャッシュ機構が正しくモックと連動することを確認

## 実装者向け注記

### 現状コードの確認
```bash
grep -n "find_target_dirs" src/cleaners/cargo.rs
# L58 （detect）と L113（clean）で2回呼ばれている

grep -n "scan_pinned_versions\|mise.*ls.*current" src/cleaners/mise.rs
# L173-178（detect）と L208-218（clean）で mise ls --current が2回
# L181（detect）と L222（clean）で scan_pinned_versions が2回
```

### 修正方針
#### CargoCleaner
```rust
pub struct CargoCleaner {
    home: PathBuf,
    runner: Box<dyn CommandRunner>,
    // 追加: detect 結果のキャッシュ
    detect_cache: std::cell::OnceCell<Vec<(PathBuf, u64)>>,
}

impl CargoCleaner {
    // 新: キャッシュがあればそれを使う共通メソッド
    fn get_target_dirs(&self) -> &Vec<(PathBuf, u64)> {
        self.detect_cache.get_or_init(|| {
            Self::find_target_dirs(&self.home)
        })
    }
}
```
- `detect()` → `get_target_dirs()` を使用
- `clean()` → `get_target_dirs()` を使用（キャッシュがあれば再利用）
- `clean()` 内で削除後は `detect_cache.take()` でクリア

#### MiseCleaner
同様のパターンを `scan_pinned_versions` と `mise ls --current` の結果に適用。

### 落とし穴
- `OnceCell` はスレッドセーフではない（`&self` が `Send + Sync` を要求）。`parking_lot::OnceLock` または `std::sync::OnceLock`（Rust 1.70+）を使用すること
- キャッシュがある状態で `clean()` がエラーになった場合、次回 `scan` で古いキャッシュを使わないようクリアすること
- テストの並列実行でキャッシュが競合しないことを確認（各テストは独立した cleaner インスタンスを持つので問題ないはず）

## Definition of Done
- [ ] CargoCleaner と MiseCleaner の両方で detect → clean のキャッシュが実装されている
- [ ] キャッシュヒット時に walkdir / 外部コマンドが再実行されないことをテストで確認
- [ ] clean() 単独実行（キャッシュなし）のフォールバックが動作する
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
