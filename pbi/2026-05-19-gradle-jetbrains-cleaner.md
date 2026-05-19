# PBI: Gradle & JetBrains IDE Cache Cleaner

## ユーザーストーリー
JVM/Android開発者として、Gradle のビルドキャッシュと JetBrains IDE（IntelliJ, GoLand, PyCharm等）のキャッシュを掃除したい、なぜなら gradle の caches ディレクトリはプロジェクトごとに数百MB、IDEキャッシュはバージョンアップごとに肥大化するから。

## ビジネス価値
Android/JVM/Go/Python 開発者（JetBrains IDE 利用者）に広く効果がある。

## BDD受け入れシナリオ

```gherkin
Scenario: Gradle キャッシュを削除する
  Given ~/.gradle/caches/ が存在する
  When  sasurahime clean gradle を実行する
  Then  ~/.gradle/caches/ 内の古いバージョンディレクトリが削除される
  And   最新のバージョンのみ保持される

Scenario: JetBrains IDE キャッシュを削除する
  Given ~/Library/Caches/JetBrains/ に複数バージョンのキャッシュが存在する
  When  sasurahime clean jetbrains を実行する
  Then  最新バージョン以外のキャッシュが削除される
```

## テスト戦略

### E2Eテスト
- ダミーの gradle caches / JetBrains ディレクトリを作成してテスト
- 古いバージョンのみ削除され、新しいバージョンが保持されることを確認

### 単体テスト
- Gradle バージョン比較（semver パース）
- JetBrains バージョン抽出（ディレクトリ名から数値バージョン抽出）

## 実装アプローチ
- Gradle: `~/.gradle/caches/` 内のディレクトリを読み、古いバージョンの jar/cache を削除（`browser` と同じ「最新以外を削除」戦略）
- JetBrains: `~/Library/Caches/JetBrains/` 内の `IdeNameXXXX.X` 形式のディレクトリから最新を保持、それ以外を削除
- どちらも `browsers` の `find_old_versions` と同様のパターン
