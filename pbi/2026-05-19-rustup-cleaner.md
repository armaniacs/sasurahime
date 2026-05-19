# PBI: Rustup Toolchain Cleaner

## ユーザーストーリー
Rust開発者として、古い Rust ツールチェーンのアンインストールを自動化したい、なぜなら rustup は多数の past toolchains を残し続け、ディスク容量を圧迫するから。

## ビジネス価値
Rust ユーザーのうち nightly を頻繁に更新する層（特に競技プログラマーや CLI 開発者）に有効。

## BDD受け入れシナリオ

```gherkin
Scenario: アクティブでないツールチェーンを一覧表示する
  Given rustup がインストールされている
  When  sasurahime clean rustup --dry-run を実行する
  Then  未使用のツールチェーンが表示される
  And   現在アクティブなツールチェーンは表示されない

Scenario: 未使用ツールチェーンを削除する
  Given 複数の古い nightly ツールチェーンがインストールされている
  When  sasurahime clean rustup を実行する
  Then  現在アクティブでないツールチェーンが削除される
```

## テスト戦略

### E2Eテスト
- ダミーの `rustup` スクリプトを作成（rustup toolchain list の出力を模倣）
- 未使用ツールチェーンのみ削除されることを確認

### 単体テスト
- `rustup toolchain list` 出力のパース
- アクティブなツールチェーンの識別

## 実装アプローチ
- mise と同様の戦略: `rustup toolchain list` で現在インストールされている toolchain を取得
- `mise ls --current` に相当する「現在アクティブな toolchain」を「default + 現在のディレクトリの override」から判定
- 未使用 toolchain に対して `rustup toolchain remove <name>` を実行
- 注意: `rustup` は mise より複雑な toolchain 命名規則を持つ（`stable-aarch64-apple-darwin` 等）
