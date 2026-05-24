# PBI-D: Xcode サブカテゴリ別部分削除UI

## ユーザーストーリー

macOS 開発者として、Xcode のキャッシュを種類ごとに選択して削除したい。なぜなら、DerivedData は消したいが Archives（過去のビルド成果物）や Simulators（再インストールに時間がかかる）は残したい、というケースがよくあるから。

## ビジネス価値

- Xcode キャッシュは最大で数十GB に達する最大の回収源のひとつ
- サブカテゴリ選択により「消しすぎ」を防ぎ、再ビルド・再インストール時間のロスを減らせる
- ユーザーがリスクなくより積極的にクリーンを実行できるようになる

## BDD受け入れシナリオ

```gherkin
Scenario: TUI でサブカテゴリを個別選択できる
  Given Xcode の DerivedData, Archives, Simulators が全て存在する
  When インタラクティブ TUI で Xcode を選択する
  Then DerivedData / Archives / Simulators が個別チェックボックスで展開される
  And 各項目のサイズが表示される
  And ユーザーは任意の組み合わせを選択できる

Scenario: DerivedData のみ削除する
  Given DerivedData: 15GB, Archives: 5GB, Simulators: 20GB が存在する
  When TUI で DerivedData のみチェックして実行する
  Then DerivedData のみが削除される
  And Archives と Simulators は保持される
  And "Freed: 15.0 GB" が表示される

Scenario: sasurahime clean xcode でサブカテゴリを引数指定できる
  Given DerivedData, Archives, Simulators が存在する
  When sasurahime clean xcode --sub derived-data を実行する
  Then DerivedData のみが削除される

Scenario: サブカテゴリが存在しない場合はスキップされる
  Given Simulators ディレクトリが存在しない
  When Xcode のサブカテゴリ一覧を表示する
  Then Simulators は "not found" として表示される
  And チェックボックスは disabled（選択不可）になる
```

## 受け入れ基準

- [ ] TUI で Xcode 選択時にサブカテゴリが展開表示される
- [ ] DerivedData / Archives / Simulators の各サイズを個別に表示する
- [ ] 任意の組み合わせを選択して削除できる
- [ ] `sasurahime clean xcode --sub <derived-data|archives|simulators>` でコマンドライン指定可能
- [ ] 存在しないサブカテゴリは無効化表示（選択不可）
- [ ] `--dry-run` でサブカテゴリ単位のドライラン動作

## t_wada スタイル テスト戦略

```
E2Eテスト:
- tempdir に DerivedData/Archives/Simulators を作成し
  sasurahime clean xcode --sub derived-data を実行
- Archives/Simulators が残っていることと freed bytes を検証

統合テスト:
- XcodeCleaner::detect_subcategories() が各パスのサイズを正しく返すことをテスト
- XcodeCleaner::clean(sub: &[XcodeSubcategory]) が指定カテゴリのみ削除することをテスト

単体テスト:
- XcodeSubcategory::path(&self, home: &Path) -> PathBuf の純関数テスト
- サブカテゴリが存在しない場合の ScanResult::NotFound テスト
```

## 実装アプローチ

- **Outside-In**: `--sub derived-data` の E2E テストから開始
- **Red-Green-Refactor**:
  1. Red: E2E テストでサブカテゴリが削除されることを確認するテストを書く
  2. Green: `XcodeSubcategory` enum を追加し、`XcodeCleaner::clean` にサブカテゴリ引数を追加
  3. Refactor: TUI のチェックボックス展開ロジックを `SubcategorySelector` に抽出
- **データ構造**:
  ```rust
  pub enum XcodeSubcategory {
      DerivedData,
      Archives,
      Simulators,
  }
  ```

## 技術的考慮事項

- 依存関係: `dialoguer` の `MultiSelect` を入れ子にするか、フラット化して表示
- Simulators のパス: `~/Library/Developer/CoreSimulator/Devices/`
- Archives のパス: `~/Library/Developer/Xcode/Archives/`
- DerivedData のパス: `~/Library/Developer/Xcode/DerivedData/`
- `sasurahime scan` の表示でも Xcode 行をサブカテゴリに展開して表示するか検討（スコープ外にする選択肢もあり）

## 見積もり

**5 SP**

## Definition of Done

- [ ] 受け入れシナリオが全て通る
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` クリーン
- [ ] `cargo fmt --check` クリーン
- [ ] コードレビュー完了
