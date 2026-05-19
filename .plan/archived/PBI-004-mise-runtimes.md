# PBI-004: mise 旧ランタイム検出・削除

> **2026-05-17 更新**: アクティブバージョンの取得を `mise ls --current` コマンド経由に変更。TOML 自前パース・`.mise.toml` 探索を廃止。

## ユーザーストーリー
**mise を使う開発者**として、どのプロジェクトからも参照されていない旧バージョンの Node.js / Python 等を検出して削除したい、なぜなら古いランタイムが数GB 単位で放置されがちだから。

## ビジネス価値
- 今回のセッションでは Node (5バージョン) + Python 3.11.8 で約 4GB 回収（araki 環境の一例）
- mise 管理ランタイムのバージョン管理を安全に行える

## BDD 受け入れシナリオ

```gherkin
Scenario: 未参照の旧 Node バージョンを一覧表示する
  Given ~/.local/share/mise/installs/node に複数バージョンが存在する
  And `mise ls --current` が node 24.15.0 のみをアクティブとして返す
  When `sasurahime scan` を実行する
  Then 参照されていない node バージョンとサイズが "unused" として表示される

Scenario: 未参照の旧ランタイムを削除する
  Given `mise ls --current` が node 24.15.0 のみを返す
  And ~/.local/share/mise/installs/node/20.11.0 が存在する
  When `sasurahime clean mise` を実行する
  Then 未参照バージョンの一覧と合計サイズが確認プロンプトと共に表示される
  And ユーザーが "y" を入力すると削除が実行される
  And 削除後の回収量が表示される

Scenario: macOS immutable フラグがあるディレクトリを削除する
  Given node_modules 配下に uchg フラグが付いたファイルが存在する
  When `sasurahime clean mise` を実行する
  Then `chflags -R nouchg` を実行してからディレクトリを削除する
  And エラーなく完了する

Scenario: プロジェクト固有設定で参照されているバージョンはスキップされる
  Given `mise ls --current` が node 20.19.5 をアクティブとして返す（プロジェクト設定由来）
  When `sasurahime clean mise` を実行する
  Then node 20.19.5 は "in use" として表示され削除されない

Scenario: mise がインストールされていない
  Given mise コマンドが PATH に存在しない
  When `sasurahime clean mise` を実行する
  Then "mise not found, skipping" と表示して正常終了する
```

## 受け入れ基準
- [ ] `mise ls --current` を実行してアクティブなバージョン一覧を取得する
- [ ] `~/.local/share/mise/installs/<tool>/` のバージョンと照合し、未参照を検出する
- [ ] 未参照バージョンを一覧表示し、確認後に削除する
- [ ] macOS の immutable フラグ (uchg) を `chflags -R nouchg` で解除してから削除する
- [ ] `--dry-run` で削除せず一覧のみ表示する
- [ ] mise 未インストールでもパニックしない

## t_wada スタイル テスト戦略
```
E2Eテスト:
- mise コマンドをモックして `mise ls --current` が node 24.15.0 を返すよう設定
- tmpdir に node/20.11.0 と node/24.15.0 を作成し clean mise を実行
- 20.11.0 が削除され 24.15.0 が残ることを確認

統合テスト:
- MiseCleaner::active_versions(runner) が `mise ls --current` の stdout を正しくパースすること
- uchg フラグが付いたディレクトリを chflags 経由で削除できること

単体テスト:
- parse_mise_ls_output(stdout) => {node: ["24.15.0"], python: ["3.12.11"]}
- version_is_active("24.15.0", &active_set) => true
- version_is_active("20.11.0", &active_set) => false
```

## 実装アプローチ
- **Outside-In**: `mise ls --current` をモックした E2E テストから開始
- `CommandRunner` trait 経由で `mise` を呼び出す（テスト時はモック注入）
- `mise ls --current` の出力パースは空白区切りの第1・2列（tool, version）を取得

## 見積もり
4 ストーリーポイント（旧 5SP から削減。TOML パース・config 探索が不要になったため）

## 技術的考慮事項
- 依存: `mise` コマンド（なければスキップ）
- 安全性: mise 自身の判断に委ねるため、`.tool-versions`・環境変数由来のバージョンも自動的にカバーされる

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] モック経由でアクティブバージョン取得をテストしている
- [ ] uchg フラグ対応が確認されている
- [ ] `cargo clippy` 警告ゼロ
