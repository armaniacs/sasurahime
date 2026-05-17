# PBI-010: Xcode DerivedData クリーナー

## ユーザーストーリー
**iOS / macOS アプリを開発する開発者**として、Xcode のビルドキャッシュ（DerivedData）をクリーンアップしたい、なぜなら複数プロジェクトのビルドが積み重なって数十GBになることがあるから。

## ビジネス価値
- DerivedData は再ビルドすれば再生成される純粋なキャッシュ
- Xcode の UI から手動で消す手順を覚えなくてよくなる

## BDD 受け入れシナリオ

```gherkin
Scenario: DerivedData を削除する
  Given ~/Library/Developer/Xcode/DerivedData が存在し複数プロジェクトのビルドキャッシュがある
  When `sasurahime clean xcode` を実行する
  Then DerivedData 内のプロジェクトディレクトリ一覧と合計サイズが表示される
  And 確認後に削除され回収サイズが表示される

Scenario: --dry-run で削除対象を確認する
  When `sasurahime clean xcode --dry-run` を実行する
  Then 削除予定のディレクトリ一覧とサイズが表示される
  And 実際には削除されない

Scenario: DerivedData が存在しない
  Given ~/Library/Developer/Xcode/DerivedData が存在しない
  When `sasurahime clean xcode` を実行する
  Then "Xcode DerivedData: not found" と表示して正常終了する

Scenario: Xcode が起動中の場合に警告を表示する
  Given Xcode プロセスが実行中である
  When `sasurahime clean xcode` を実行する
  Then "Warning: Xcode is running. DerivedData deletion may cause issues." と警告する
  And 続行するか確認を求める
```

## 受け入れ基準
- [ ] `~/Library/Developer/Xcode/DerivedData/` を対象とする
- [ ] Simulator データ（`~/Library/Developer/CoreSimulator`）は対象外
- [ ] Xcode 実行中の場合は警告を表示して確認を求める
- [ ] `--dry-run` で削除せず一覧表示のみ
- [ ] DerivedData ディレクトリ不在時はスキップ

## t_wada スタイル テスト戦略
```
E2Eテスト:
- tmpdir に DerivedData/ProjectA-xxx / ProjectB-xxx を作成し clean xcode で全削除されることを確認
- --dry-run でファイルが残ることを確認

統合テスト:
- XcodeCleaner::detect() が DerivedData のサイズを正確に返すこと
- Xcode プロセス検出が pgrep 経由で正しく動作すること

単体テスト:
- is_xcode_running() が pgrep の結果を正しく解釈すること
```

## 実装アプローチ
- Xcode 実行チェック: `pgrep -x Xcode` で検出
- 削除: `std::fs::remove_dir_all` で DerivedData 以下を全削除

## 見積もり
2 ストーリーポイント

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] Xcode 実行中の警告が動作することを確認
- [ ] `cargo clippy` 警告ゼロ
