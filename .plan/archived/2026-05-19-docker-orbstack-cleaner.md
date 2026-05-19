# PBI: Docker & Orbstack Cache Cleaner

## ユーザーストーリー
コンテナを日常的に使う開発者として、Docker と Orbstack の不要なイメージ・ビルドキャッシュを掃除したい、なぜなら docker system prune で数十GB単位で容量が回収できるから。

## ビジネス価値
最も効果的な削減ターゲットの一つ。docker system prune は一度の実行で10GB以上回収できる。

## BDD受け入れシナリオ

```gherkin
Scenario: Docker システムキャッシュを削除する
  Given docker コマンドが PATH に存在する
  When  sasurahime clean docker を実行する
  Then  docker system prune -af が実行される
  And   docker builder prune -af が実行される

Scenario: Docker がインストールされていない場合はスキップ
  Given docker コマンドが PATH に存在しない
  When  sasurahime clean docker を実行する
  Then  "docker: not found" と表示される
  And   終了コード 0 で終了する

Scenario: Orbstack キャッシュを削除する
  Given orb コマンドが PATH に存在する
  When  sasurahime clean orbstack を実行する
  Then  orb prune が実行される
```

## テスト戦略

### E2Eテスト
- ダミーの `docker` / `orb` スクリプトを作成して実行されることを確認（既存パターン）
- docker 未インストール時のスキップ動作

### 単体テスト
- なし（外部CLI委譲のみ）

## 実装アプローチ
- 外部CLI委譲（`brew`, `bun` 等と同じパターン）
- Docker: `docker system prune -af` + `docker builder prune -af` の順に実行
- Orbstack: `orb prune`
- 検出: `which docker` / `which orb`
- detect ではツール存在のみを確認（削除可能サイズは不明）
