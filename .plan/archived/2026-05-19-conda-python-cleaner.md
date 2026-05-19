# PBI: Conda & Python Cache Cleaner

## ユーザーストーリー
データサイエンス／Python開発者として、Conda と Poetry、pipx のキャッシュを掃除したい、なぜなら conda clean で数GBの不要パッケージキャッシュが削除でき、poetry/pipx のキャッシュも蓄積するから。

## ビジネス価値
Anaconda/Miniconda ユーザーはキャッシュが数十GBに達することがある。Poetry 利用者も増加中。

## BDD受け入れシナリオ

```gherkin
Scenario: Conda キャッシュを削除する
  Given conda コマンドが PATH に存在する
  When  sasurahime clean conda を実行する
  Then  conda clean --all -y が実行される

Scenario: Poetry キャッシュを削除する
  Given poetry コマンドが PATH に存在する
  When  sasurahime clean poetry を実行する
  Then  poetry cache clear --all が実行される

Scenario: pipx が未インストールならスキップ
  Given pipx コマンドが PATH に存在しない
  When  sasurahime clean pipx を実行する
  Then  "pipx: not found" と表示される
```

## テスト戦略

### E2Eテスト
- ダミー `conda` / `poetry` / `pipx` スクリプトで実行確認

### 単体テスト
- 各ツールの PATH 検出

## 実装アプローチ
- Conda: `conda clean --all -y`
- Poetry: `poetry cache clear --all`
- pipx: `pipx list` の出力をパースして未使用パッケージを検出→削除（高度, 初版は `pipx uninstall` 個別実行にできる）
- 初版: conda のみ実装、poetry/pipx は次版で追加も可（ハッピーパス優先分割）
