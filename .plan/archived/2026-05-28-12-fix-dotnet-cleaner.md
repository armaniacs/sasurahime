# PBI: .NET SDK NuGet キャッシュクリーナー (dotnet)

## ユーザーストーリー
.NET デベロッパーとして、`sasurahime clean dotnet` で NuGet キャッシュを削除したい。なぜなら、`~/.nuget/packages/` に古いパッケージバージョンが蓄積し、開発環境のディスク容量を圧縮するからである。

## ビジネス価値
- 新たな clean target の追加
- NuGet キャッシュの一元管理（dotnet CLI 経由）
- `sasurahime scan` で NuGet キャッシュ使用量の可視化

## BDD受け入れシナリオ

```gherkin
Scenario: dotnet CLI がインストールされている場合に clean が実行される
  Given dotnet CLI がインストールされている
  When sasurahime clean dotnet を実行する
  Then dotnet nuget locals all --clear が実行される
  And 終了コード 0 で成功する

Scenario: dotnet がインストールされていない場合に clean がスキップされる
  Given dotnet CLI がインストールされていない
  When sasurahime clean dotnet を実行する
  Then "skipping" または "not found" が表示される
  And 終了コード 0 で成功する

Scenario: scan で dotnet の状態が正しく表示される
  Given dotnet CLI がインストールされている
  And ~/.nuget/ ディレクトリが存在する
  When sasurahime scan を実行する
  Then 出力に "dotnet" 行が含まれる

Scenario: dry-run で削除されない
  Given dotnet CLI がインストールされている
  When sasurahime clean dotnet --dry-run を実行する
  Then 終了コード 0 で成功する
```

## 受け入れ基準
- [ ] `sasurahime scan` に "dotnet" 行が追加される
- [ ] `sasurahime clean dotnet` が `dotnet nuget locals all --clear` を実行する
- [ ] dotnet 未インストール時はスキップする
- [ ] `sasurahime targets` に "dotnet" が表示される
- [ ] 後方互換性が維持される（既存の全テストがパスする）

## テスト戦略（t_wadaスタイル）

### E2Eテスト（3）
- `sasurahime clean dotnet --dry-run` が終了コード0
- `sasurahime clean dotnet` が `dotnet nuget locals all --clear` を呼び出す（MockRunner で検証）
- `sasurahime scan` に "dotnet" が含まれる

### 統合テスト（2）
- `GenericCleaner::dotnet()` が正しい `CleanMethod::Command` を生成する
- `is_available()` が dotnet CLI の有無を正しく返す

## 実装アプローチ
- **Outside-In**: E2E テスト → 統合テスト → 実装
- `GenericCleaner::command_cleaner` パターンを使用

## 見積もり
1 SP（1日未満、既存パターンの適用のみ）

## 技術的考慮事項
- 依存関係: なし
- `dotnet nuget locals all --clear` は以下のキャッシュを全て削除する: http-cache, packages-cache, temp, plugins-cache
- 削除後、次回の `dotnet restore` でパッケージが再ダウンロードされる点に注意

## 実装者向け注記

### 現状コードの確認
```bash
grep -rn "dotnet\|nuget" src/ --include="*.rs"
# → 現状 dotnet 関連の cleaner は存在しない
```

### 変更箇所
1. `src/cleaners/generic.rs`: `dotnet()` factory メソッドを追加
```rust
pub fn dotnet(runner: Box<dyn CommandRunner>) -> Self {
    Self::command_cleaner("dotnet", "dotnet", &["nuget", "locals", "all", "--clear"], runner)
}
```

2. `src/main.rs`: `define_cleaners!` マクロに1行追加
```rust
Dotnet : "dotnet" => ".NET NuGet cache clear";
(|_home, _config| cleaners::generic::GenericCleaner::dotnet(Box::new(SystemCommandRunner))),
```

### 落とし穴
- `dotnet nuget locals all --clear` は確認プロンプトなしで全ての NuGet キャッシュを削除する。`--dry-run` 相当のオプションは `dotnet nuget locals all --list`（削除せずに表示のみ）
- `dotnet` CLI がインストールされている環境とされていない環境の両方でテストすること
- `~/.nuget/packages/` のサイズは大規模プロジェクトで数GBに達することがある。`detect()` では `dir_size` による実測ではなく `Command` パターンの標準動作（CLI の有無のみ確認）となる点に注意。`CommandWithDetectDir` に変更する場合は `detect_dir: home.join(".nuget/packages")` を追加

## Definition of Done
- [ ] `sasurahime targets` に "dotnet" が表示される
- [ ] `sasurahime clean dotnet` が `dotnet nuget locals all --clear` を実行する
- [ ] dotnet 未インストール時にエラーではなくスキップする
- [ ] 全既存テストがパスする
- [ ] コードレビュー完了
