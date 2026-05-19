# PBI-003: Homebrew キャッシュクリーナー

## ユーザーストーリー
**Homebrew を使う開発者**として、古いボトルのダウンロードキャッシュを安全に削除したい、なぜなら `brew cleanup` だけでは downloads ディレクトリが消えず気づかぬうちに 15GB を超えるから。

## ビジネス価値
- 今回のセッションで最大の回収 (16.6GB)
- `brew cleanup -s --prune=all` の正しい使い方を知らないユーザーでも安全に実行できる

## BDD 受け入れシナリオ

```gherkin
Scenario: Homebrew のダウンロードキャッシュを削除する
  Given ~/Library/Caches/Homebrew/downloads にキャッシュファイルが存在する
  When `sasurahime clean brew` を実行する
  Then `brew cleanup -s --prune=all` が実行される
  And 回収サイズが表示される

Scenario: --dry-run でキャッシュ量を確認する
  Given ~/Library/Caches/Homebrew が存在する
  When `sasurahime clean brew --dry-run` を実行する
  Then 削除予定サイズが表示される（brew cleanup --dry-run の出力をパース）
  And 実際には削除されない

Scenario: Homebrew がインストールされていない
  Given brew コマンドが PATH に存在しない
  When `sasurahime clean brew` を実行する
  Then "Homebrew not found, skipping" と表示して正常終了する
```

## 受け入れ基準
- [ ] `brew cleanup -s --prune=all` を内部実行する
- [ ] `--dry-run` 時は `brew cleanup -s --prune=all --dry-run` を実行して出力を表示する
- [ ] 回収バイト数を stdout からパースして表示する
- [ ] brew 未インストールでもパニックしない

## t_wada スタイル テスト戦略
```
E2Eテスト:
- brew コマンドをモックして呼び出し引数を検証
- dry-run フラグが brew へ正しく渡されることを確認

統合テスト:
- parse_brew_freed_bytes("This operation has freed approximately 16.6GB") => 17_825_792_000

単体テスト:
- parse_size_str("16.6GB") => 17_825_792_000
- parse_size_str("194.3MB") => 203_843_788
```

## 実装アプローチ
- **Outside-In**: brew コマンドの呼び出し検証テストから開始
- brew の出力フォーマット `approximately X.XGB` を正規表現でパース

## 見積もり
2 ストーリーポイント

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] --dry-run が正しく機能する
- [ ] `cargo clippy` 警告ゼロ
