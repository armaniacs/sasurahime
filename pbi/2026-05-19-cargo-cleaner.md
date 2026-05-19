# PBI: Cargo Cache Cleaner

## ユーザーストーリー
Rust開発者として、cargo のビルドキャッシュとレジストリキャッシュを掃除したい、なぜなら `~/.cargo/registry/cache/` とプロジェクト内の `target/debug/` が数十GBに膨れ上がることがあるから。

## ビジネス価値
Rustユーザーの大半が対象。target ディレクトリはプロジェクトごとに数百MB〜数GB消費する。レジストリキャッシュも定期的に掃除しないと肥大化する。

## BDD受け入れシナリオ

```gherkin
Scenario: cargo レジストリキャッシュを削除する
  Given ~/.cargo/registry/cache/ にキャッシュファイルが存在する
  When  sasurahime clean cargo を実行する
  Then  レジストリキャッシュが削除される
  And   解放サイズが報告される

Scenario: target ディレクトリをスキャンして削除候補を表示する
  Given ホームディレクトリ以下に target/debug/ ディレクトリが存在する
  When  sasurahime scan を実行する
  Then  cargo の削除候補として target ディレクトリの合計サイズが表示される
```

## テスト戦略

### E2Eテスト
- 空の cargo registry キャッシュで実行してもエラーにならない
- ダミーの target/debug/ ディレクトリを作成して scan が認識する

### 単体テスト
- 既存の `dir_size` 関数で target ディレクトリのサイズ計算が可能
- レジストリキャッシュのパス解決

## 実装アプローチ
- `GenericCleaner` パターンを踏襲（外部CLI削除 + ディレクトリ削除の組み合わせ）
- レジストリキャッシュ: `rm -rf ~/.cargo/registry/cache/*`（rsync 不要）
- target ディレクトリ: `find ~ -maxdepth 4 -name target -type d` でスキャン（動作確認のため上限深さ4）
