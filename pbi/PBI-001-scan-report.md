# PBI-001: スキャン & サマリーレポート

## ユーザーストーリー
**macOS 開発者**として、既知のキャッシュディレクトリを一括スキャンしてサイズと削除候補を確認したい、なぜなら何がディスクを圧迫しているか把握してから判断したいから。

## ビジネス価値
- ディスク逼迫に気づいたとき、何が原因かを数秒で把握できる
- 測定: スキャン完了まで 5 秒以内（一般的な開発環境）

## BDD 受け入れシナリオ

```gherkin
Scenario: 標準スキャンでサマリーを表示する
  Given macOS 開発者のホームディレクトリが存在する
  When `sasurahime scan` を実行する
  Then 各カテゴリ（uv / brew / mise / playwright / bun / go / pip）のサイズが表示される
  And 合計削除候補サイズが表示される
  And 削除候補がない項目は "clean" と表示される

Scenario: 対象ディレクトリが存在しない場合
  Given ~/.cache/uv が存在しない
  When `sasurahime scan` を実行する
  Then uv の行は "not found" として表示される
  And エラーで終了しない

Scenario: 読み取り権限のないディレクトリがある場合
  Given ~/Library/Application Support/CloudDocs が Permission denied になる
  When `sasurahime scan` を実行する
  Then 権限エラーのディレクトリはスキップされ警告が表示される
  And 他のスキャンは継続される
```

## 受け入れ基準
- [ ] `sasurahime scan` でカテゴリ別サイズ一覧が表示される
- [ ] 合計削除候補サイズが GB 単位で表示される
- [ ] 対象ディレクトリが存在しない場合でも正常終了する
- [ ] Permission denied のディレクトリはスキップ & 警告表示
- [ ] macOS Apple Silicon (arm64) で動作する

## t_wada スタイル テスト戦略
```
E2Eテスト:
- sasurahime scan をプロセス起動し stdout をパースして各カテゴリが出力されることを確認

統合テスト:
- tmpdir にフィクスチャディレクトリ（既知サイズのファイル群）を作成し Scanner がサイズを正確に返すことを確認

単体テスト:
- format_bytes(1_073_741_824) => "1.0 GB" などの表示フォーマット
- ディレクトリ非存在時の ScanResult が NotFound variant になること
- 権限エラー時の ScanResult が PermissionDenied variant になること
```

## 実装アプローチ
- **Outside-In**: E2E テスト（stdout 検証）を先に書き、次に Scanner trait を定義
- **Red-Green-Refactor**: format_bytes から単体テスト駆動で実装開始
- **データ構造**: `ScanResult { category, path, size_bytes, status }` を定義して各クリーナーが使い回せるよう設計

## 見積もり
3 ストーリーポイント

## 技術的考慮事項
- 依存: `walkdir` または `std::fs::metadata` でサイズ集計
- パフォーマンス: rayon で並列スキャンも検討
- 出力: tabled / comfy-table で表形式

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] 単体・統合・E2E テストが CI で green
- [ ] `cargo clippy` 警告ゼロ
- [ ] README にコマンド例を記載
