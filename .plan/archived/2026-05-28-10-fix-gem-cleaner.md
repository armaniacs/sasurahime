# PBI: RubyGems キャッシュクリーナー (gem)

## ユーザーストーリー
Ruby デベロッパーとして、`sasurahime clean gem` で古い gem のバージョンを削除したい。なぜなら、`gem cleanup` を手動で実行するのを忘れがちで、`~/.gem/` に使っていない gem の古いバージョンが溜まり続けるからである。

## ビジネス価値
- 新たな clean target の追加（全 target 数を44に増加）
- `sasurahime scan` で gem キャッシュの使用量を可視化
- `gem cleanup` のラッパーとして統一的な CLI 操作を提供

## BDD受け入れシナリオ

```gherkin
Scenario: gem がインストールされている場合に scan でサイズが表示される
  Given gem CLI がインストールされている
  And ~/.gem/ ディレクトリが存在する
  When sasurahime scan を実行する
  Then 出力に "gem" 行が含まれる
  And ステータスが pruneable または clean である

Scenario: gem がインストールされていない場合に scan で NotFound と表示される
  Given gem CLI がインストールされていない
  When sasurahime scan を実行する
  Then 出力の "gem" 行のステータスが n/a である

Scenario: gem cleanup が正常に実行される
  Given gem CLI がインストールされている
  When sasurahime clean gem を実行する
  Then gem cleanup が実行される
  And 終了コード 0 で成功する

Scenario: gem がインストールされていない場合に clean がスキップされる
  Given gem CLI がインストールされていない
  When sasurahime clean gem を実行する
  Then "skipping" または "not found" が表示される
  And 終了コード 0 で成功する
```

## 受け入れ基準
- [ ] `sasurahime scan` に "gem" 行が追加される
- [ ] `sasurahime clean gem` が `gem cleanup` を実行する
- [ ] gem 未インストール時は `NotFound` を返す（エラーにしない）
- [ ] `sasurahime targets` に "gem" が表示される
- [ ] 全既存テストがパスする

## テスト戦略（t_wadaスタイル）

### E2Eテスト（3）
- `sasurahime clean gem --dry-run` が終了コード0（gem がなくても）
- `sasurahime clean gem` が `gem cleanup` を呼び出す（MockRunner で検証）
- `sasurahime scan` に "gem" が含まれる

### 統合テスト（2）
- `GenericCleaner::gem()` が正しい `CleanMethod::Command` を生成する
- `is_available()` が gem CLI の有無を正しく返す

### 単体テスト（1）
- `command_cleaner` の引数が期待通りであること

## 実装アプローチ
- **Outside-In**: E2E テスト → 統合テスト → 実装
- `GenericCleaner::command_cleaner` パターンを使用（bun, go, pip と同様）

## 見積もり
1 SP（1日未満、既存パターンの適用のみ）

## 技術的考慮事項
- 依存関係: なし
- テスタビリティ: `MockRunner` で `gem cleanup` の呼び出しを検証可能

## 実装者向け注記

### 現状コードの確認
```bash
grep -rn "gem\|gem cleanup" src/ --include="*.rs"
# → 現状 gem 関連の cleaner は存在しない
```

### 変更箇所
1. `src/cleaners/generic.rs`: `gem()` factory メソッドを追加
```rust
pub fn gem(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("gem", "gem", &["cleanup"], runner)
}
```

2. `src/main.rs`: `define_cleaners!` マクロに1行追加
```rust
Gem : "gem" => "RubyGems old gem versions cleanup";
(|_home, _config| cleaners::generic::GenericCleaner::gem(Box::new(SystemCommandRunner))),
```

### 落とし穴
- `gem cleanup` はユーザーの `~/.gem/` にある全 gem の古いバージョンを削除する。削除前に確認プロンプトは出ない
- `gem cleanup --dry-run` で削除予定の gem を事前確認可能。`dry_run` モードでの対応は既存の `Command` パターンが自動処理する

## Definition of Done
- [ ] `sasurahime targets` に "gem" が表示される
- [ ] `sasurahime clean gem` が `gem cleanup` を実行する
- [ ] gem 未インストール時にエラーではなくスキップする
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
