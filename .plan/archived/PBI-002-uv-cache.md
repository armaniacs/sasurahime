# PBI-002: uv キャッシュクリーナー

## ユーザーストーリー
**uv を使う Python 開発者**として、使われていない Python パッケージキャッシュを安全に削除したい、なぜなら archive-v0 は気づかないうちに 20GB 超になるから。

## ビジネス価値
- 最大効果のクリーナー。今回のセッションでは 18.2GB を回収
- `uv cache prune` ラッパーとして提供することで安全性を担保

## BDD 受け入れシナリオ

```gherkin
Scenario: uv キャッシュの未使用パッケージを削除する
  Given ~/.cache/uv/archive-v0 が存在し未使用パッケージが含まれている
  When `sasurahime clean uv` を実行する
  Then 削除前のサイズと削除後のサイズが表示される
  And 削除されたファイル数と回収サイズが表示される

Scenario: --dry-run で削除内容を確認する
  Given ~/.cache/uv が存在する
  When `sasurahime clean uv --dry-run` を実行する
  Then 削除予定の内容とサイズが表示される
  And 実際には何も削除されない

Scenario: 旧インデックスキャッシュ（simple-v16 等）を削除する
  Given ~/.cache/uv/simple-v16 が存在する
  When `sasurahime clean uv` を実行する
  Then 現行バージョン（simple-v18, v21 等）より古いインデックスが削除される
  And 現行インデックスは削除されない

Scenario: uv がインストールされていない場合
  Given uv コマンドが PATH に存在しない
  When `sasurahime clean uv` を実行する
  Then "uv not found, skipping" と表示して正常終了する
```

## 受け入れ基準
- [ ] `uv cache prune --force` を内部実行して未使用パッケージを削除する
- [ ] simple-v{N} ディレクトリのうち最新以外を削除する
- [ ] `--dry-run` フラグで実際の削除をスキップできる
- [ ] 削除前後のサイズ差分を表示する
- [ ] uv 未インストールでもパニックしない

## t_wada スタイル テスト戦略
```
E2Eテスト:
- tmpdir に simple-v16 / simple-v17 / simple-v21 を作成し、clean 後に v16/v17 のみ消えることを確認
- --dry-run 実行後にファイルが残っていることを確認

統合テスト:
- UvCleaner::detect_old_indexes() が現行より古いバージョンのパスを返すことを確認
- uv コマンドが存在しない場合 Err(UvNotFound) を返すことを確認

単体テスト:
- parse_simple_version("simple-v16") => Some(16)
- is_older_than_current([16, 17], 21) => [16, 17]
```

## 実装アプローチ
- **Outside-In**: dry-run E2E テストから開始（実ファイル削除なしで検証可能）
- `std::process::Command` で `uv cache prune --force` を呼び出し
- インデックスバージョン検出は正規表現 `simple-v(\d+)` でパース

## 見積もり
2 ストーリーポイント

## 技術的考慮事項
- 依存: `uv` コマンドが存在する前提（なければスキップ）
- 安全性: archive-v0 は uv 自身に任せ、直接 rm しない

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] --dry-run が正しく機能する
- [ ] `cargo clippy` 警告ゼロ
