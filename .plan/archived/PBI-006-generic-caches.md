# PBI-006: 汎用キャッシュクリーナー (bun / go / pip / node-gyp)

> **2026-05-17 更新**: CLI を対称化。各ツールが独立した `sasurahime clean <name>` サブコマンドを持つ。`clean caches` はグループエイリアスとして残す。

## ユーザーストーリー
**複数の言語・ツールを使う開発者**として、bun・Go・pip・node-gyp などのキャッシュをまとめて削除したい、なぜなら個別のコマンドを覚えるのが面倒で、合計するとそれなりの量になるから。

## ビジネス価値
- 今回のセッションで bun 5.5GB + go 262MB + pip 166MB + node-gyp 242MB ≈ 6.2GB 回収
- ツールごとのコマンドを sasurahime に集約できる

## BDD 受け入れシナリオ

```gherkin
Scenario: インストール済みツールのキャッシュをまとめてクリアする
  Given bun / go / pip がインストールされている
  When `sasurahime clean caches` を実行する
  Then 各ツールのキャッシュサイズが表示される
  And 確認後に各ツールのクリアコマンドが実行される
  And 合計回収サイズが表示される

Scenario: 特定ツールのみクリアする
  When `sasurahime clean caches --only bun,go` を実行する
  Then bun と go のキャッシュのみクリアされる
  And pip / node-gyp はスキップされる

Scenario: インストールされていないツールはスキップされる
  Given pip がインストールされていない
  When `sasurahime clean caches` を実行する
  Then pip は "not found, skipped" と表示される
  And 他のツールは処理される
```

## 受け入れ基準
- [ ] `sasurahime clean bun` / `clean go` / `clean pip` / `clean node-gyp` が個別に動作する
- [ ] `sasurahime clean caches` が上記全ツールを順に実行するグループエイリアスとして動作する
- [ ] `bun pm cache rm` を実行して bun キャッシュを削除する
- [ ] `go clean -cache` を実行して Go ビルドキャッシュを削除する
- [ ] `pip cache purge` を実行して pip キャッシュを削除する
- [ ] `~/.cache/node-gyp` / `~/Library/Caches/node-gyp` を直接削除する
- [ ] 未インストールツールはスキップ（エラーにしない）
- [ ] `--dry-run` でコマンド一覧を表示するのみ

## t_wada スタイル テスト戦略
```
E2Eテスト:
- 各コマンドをモックして呼び出し順と引数を検証
- --only フラグで指定外ツールが呼ばれないことを確認

統合テスト:
- ToolCache::is_available("bun") がコマンド存在チェックを正しく行うことを確認

単体テスト:
- parse_only_flag("bun,go") => [Tool::Bun, Tool::Go]
- Tool の Display が "bun" / "go" / "pip" / "node-gyp" であること
```

## 実装アプローチ
- `Tool` enum で各クリーナーを統一インターフェイス化
- 各 Tool は `detect_size() -> u64` と `clean() -> Result<u64>` を実装

## 見積もり
3 ストーリーポイント

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] --only フラグが正しく機能する
- [ ] `cargo clippy` 警告ゼロ
