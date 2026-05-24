# PBI-A: 並列スキャン最適化

## ユーザーストーリー

macOS 開発者として、`sasurahime scan` が素早く完了してほしい。なぜなら、40以上のクリーナーを順番に実行すると起動が遅くなり、日常的に使うツールとして許容しにくいから。

## ビジネス価値

- スキャン完了までの体感時間を短縮（目標: 現状比 50% 以上削減）
- 未インストールツールのI/Oオーバーヘッドを排除
- クリーナーが増えても性能劣化しないスケーラブルな設計に移行

## BDD受け入れシナリオ

```gherkin
Scenario: 未インストールツールは早期スキップされる
  Given uv がインストールされていない環境
  When sasurahime scan を実行する
  Then UvCleaner はバイナリ検索のみで即座にスキップされる
  And ファイルシステムへのアクセスは発生しない

Scenario: 複数クリーナーが並列実行される
  Given 5つ以上のクリーナーが利用可能な環境
  When sasurahime scan を実行する
  Then 全クリーナーが並列に実行される
  And 合計時間が最も遅いクリーナーの時間に近い（直列合計より短い）

Scenario: 並列実行でも結果は決定的に表示される
  Given 複数クリーナーが並列実行される
  When 結果テーブルを表示する
  Then クリーナーの順序は常に一定（名前順など）で表示される
  And レースコンディションによる表示崩れがない
```

## 受け入れ基準

- [ ] `rayon` の `par_iter` を使いスキャンを並列化している
- [ ] バイナリ不在（`which` で確認）の場合は detect() を呼ばずスキップし `NotFound` を返す
- [ ] 結果テーブルの表示順は決定的（クリーナー登録順）
- [ ] 既存の全テストが引き続きパスする
- [ ] `cargo clippy -- -D warnings` がクリーン

## t_wada スタイル テスト戦略

```
E2Eテスト:
- tempdir を HOME に設定し sasurahime scan を実行、完了時間が閾値以内であることを確認
- 未インストールツール環境でスキャンしても NotFound が返ることを確認

統合テスト:
- 各 Cleaner の detect() が CommandRunner モック経由で呼ばれているか確認
- NotFound 判定のロジック（バイナリ検索）を CommandRunner モックでテスト

単体テスト:
- is_binary_available(name) の純関数テスト
- 並列結果のソート・集約ロジックのテスト
```

## 実装アプローチ

- **Outside-In**: E2E テストで「scan が X 秒以内」を先に書く
- **Red-Green-Refactor**:
  1. Red: 逐次実行のまま時間閾値テストが落ちることを確認
  2. Green: `rayon::par_iter` で並列化
  3. Refactor: バイナリ検索の共通化（`Scanner::is_available` など）
- **リファクタリング**: `Scanner` 構造体に並列実行ロジックを集約

## 技術的考慮事項

- 依存関係: `rayon` を `Cargo.toml` に追加（既存の `tokio` は不要）
- テスタビリティ: `CommandRunner` trait の `which` 相当を追加
- スレッド安全: 各 Cleaner は `Send + Sync` を実装する必要あり
- 出力の順序: `par_iter` 後に `sort_by_key` で名前順に整列

## 見積もり

**3 SP**

## Definition of Done

- [ ] 受け入れシナリオが全て通る
- [ ] `cargo test` 全パス
- [ ] `cargo clippy -- -D warnings` クリーン
- [ ] `cargo fmt --check` クリーン
- [ ] コードレビュー完了
