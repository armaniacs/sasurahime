# PBI: Bundler キャッシュクリーナー (bundle)

## ユーザーストーリー
Ruby デベロッパーとして、`sasurahime clean bundle` で Bundler のキャッシュを削除したい。なぜなら、`bundle clean` や `~/.bundle/cache/` のクリーンアップを定期的に実行するのを忘れがちだからである。

## ビジネス価値
- 新たな clean target の追加（全 target 数を45に増加）
- Ruby エコシステムのカバレッジ向上（gem + bundle の両方をサポート）
- プロジェクト横断的な Bundler キャッシュの一括削除

## BDD受け入れシナリオ

```gherkin
Scenario: bundle CLI がインストールされている場合に clean が実行される
  Given bundle CLI がインストールされている
  When sasurahime clean bundle を実行する
  Then bundle clean が実行される
  And 終了コード 0 で成功する

Scenario: bundle がインストールされていない場合に clean がスキップされる
  Given bundle CLI がインストールされていない
  When sasurahime clean bundle を実行する
  Then "skipping" または "not found" が表示される
  And 終了コード 0 で成功する

Scenario: scan で bundle の状態が表示される
  Given bundle CLI がインストールされている
  When sasurahime scan を実行する
  Then 出力に "bundle" 行が含まれる

Scenario: dry-run で削除されない
  Given bundle CLI がインストールされている
  When sasurahime clean bundle --dry-run を実行する
  Then 終了コード 0 で成功する
  And "dry-run" または "would remove" が表示される
```

## 受け入れ基準
- [ ] `sasurahime scan` に "bundle" 行が追加される
- [ ] `sasurahime clean bundle` が `bundle clean` を実行する
- [ ] bundle 未インストール時はスキップする
- [ ] `sasurahime targets` に "bundle" が表示される
- [ ] 全既存テストがパスする

## テスト戦略（t_wadaスタイル）

### E2Eテスト（3）
- `sasurahime clean bundle --dry-run` が終了コード0
- `sasurahime clean bundle` が `bundle clean` を呼び出す
- `sasurahime scan` に "bundle" が含まれる

### 統合テスト（2）
- `GenericCleaner::bundle()` が正しい `CleanMethod` を生成する
- `is_available()` が bundle CLI の有無を正しく返す

## 実装アプローチ
- **Outside-In**: E2E テスト → 統合テスト → 実装
- `GenericCleaner::command_cleaner` パターンを使用

## 見積もり
1 SP（1日未満、既存パターンの適用のみ）

## 技術的考慮事項
- 依存関係: なし
- `bundle clean` はカレントディレクトリの Gemfile に依存する場合がある。CLI ヘルプで `--dry-run` が利用可能であれば dry-run モードで活用する

## 実装者向け注記

### 現状コードの確認
```bash
grep -rn "bundle\|bundler" src/ --include="*.rs"
# → 現状 bundle 関連の cleaner は存在しない
```

### 変更箇所
1. `src/cleaners/generic.rs`: `bundle()` factory メソッドを追加
```rust
pub fn bundle(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("bundle", "bundle", &["clean"], runner)
}
```

2. `src/main.rs`: `define_cleaners!` マクロに1行追加
```rust
Bundle : "bundle" => "Bundler cache clean";
(|_home, _config| cleaners::generic::GenericCleaner::bundle(Box::new(SystemCommandRunner))),
```

### 落とし穴
- `bundle clean` はプロジェクトの Gemfile.lock がないと正しく動作しない場合がある（通常はプロジェクトルートで実行されることを想定）。とはいえ Bundler のキャッシュ自体はグローバルに削除可能
- `gem` cleaner と `bundle` cleaner は別々に作成する。`gem cleanup` はシステム gem の古いバージョンを削除、`bundle clean` は Bundler 管理下の gem を整理する。役割が異なる

## Definition of Done
- [ ] `sasurahime targets` に "bundle" が表示される
- [ ] `sasurahime clean bundle` が `bundle clean` を実行する
- [ ] bundle 未インストール時にエラーではなくスキップする
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
