# PBI: sasurahime history エイリアスの追加

## ユーザーストーリー
sasurahimeユーザーとして、`sasurahime history` で履歴と統計を表示したい。なぜなら、Mole の `mo history` や他の CLI ツールと同じ感覚で「いつ・何を・どれだけ消したか」を振り返りたいから。

## ビジネス価値
- ユーザーが直感的なコマンド名で履歴にアクセスできる
- `sasurahime stats` という名前に違和感があるユーザーの導線を確保する
- ブログ記事で宣言した機能を実現し、ユーザー期待に応える

## BDD受け入れシナリオ

```gherkin
Scenario: history エイリアスで統計を表示する
  Given history.json に過去の clean 記録が 1 件以上ある
  When  ユーザーが `sasurahime history` を実行する
  Then  `sasurahime stats` と同じ統計情報と最近の clean 一覧が表示される
  And   合計解放バイト数と実行回数が表示される

Scenario: 履歴が空の場合
  Given history.json が存在しない、または空である
  When  ユーザーが `sasurahime history` を実行する
  Then  "No history yet. Run 'sasurahime clean' to get started." と表示される
```

## 受け入れ基準
- [ ] `sasurahime history` を実行すると `sasurahime stats` と同じ出力になる
- [ ] `sasurahime history --last 5` などのオプションも `sasurahime stats --last 5` と同じ動作をする
- [ ] 既存の `sasurahime stats` の動作に変更がない
- [ ] `sasurahime --help` で `history` エイリアスが確認できる

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- テンポラリ HOME ディレクトリを用意し、`history.json` に履歴を書き込む
- `sasurahime history` を実行し、期待される統計メッセージが stdout に含まれることを検証
- `sasurahime history --last 1` で最新 1 件のみ表示されることを検証

### 統合テスト
- 不要。エイリアス追加のみであり、既存の履歴表示ロジックを流用する

### 単体テスト
- clap のサブコマンドパースで `history` が `Stats` として解釈されることを検証
- `Stats` の各オプションが `history` 経由でも受け渡されることを検証

## 実装アプローチ
- **Outside-In**: E2E テストから開始し、失敗を確認してから実装
- **Red-Green-Refactor**: 最小変更でテストをグリーンにする
- **リファクタリング**: 重複があれば整理する

## 見積もり
1 ストーリーポイント

## 技術的考慮事項
- clap の `#[command(alias = "history")]` または `#[command(visible_alias = "history")]` を使用
- `Commands::Stats` の処理は変更せず、エイリアスとして解決されるようにする
- macOS のみ対象。他プラットフォームは対象外

## 実装者向け注記

### 現状コードの確認
```bash
grep -rn "Commands::Stats" src/
grep -rn "Show deletion history" src/
```

`src/main.rs` に `Commands::Stats` が定義されており、履歴の読み込み・表示処理は `src/history.rs` に実装済み。

### 実装手順
1. `src/main.rs` の `Commands::Stats` に `#[command(visible_alias = "history")]` を追加
2. E2E テストを追加し、`sasurahime history` と `sasurahime stats` の出力が一致することを確認
3. `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check` を実行

### 実装例
```rust
#[derive(Subcommand)]
enum Commands {
    // ...
    /// Show deletion history and statistics
    #[command(visible_alias = "history")]
    Stats {
        /// Show only the last N entries
        #[arg(long, default_value = "10")]
        last: usize,
    },
}
```

### 落とし穴
- `alias` だけだとヘルプに表示されない。`visible_alias` を使うことを推奨
- 既存の `stats` コマンドの挙動を壊さないよう注意

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] `cargo clippy -- -D warnings` がパスする
- [ ] `cargo fmt --check` がパスする
- [ ] コードレビュー完了
- [ ] ドキュメント（README の Usage セクション）に `history` が追記されている
