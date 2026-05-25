# PBI-E: config.toml 統合設定ファイル

## ユーザーストーリー

macOS 開発者として、設定ファイルでカスタムパスの追加・クリーナーの除外・フィルタ条件を一元管理したい。なぜなら、コマンドラインフラグだけでは設定を毎回指定しなければならず、cron や自動化での再利用が難しいから。

## ビジネス価値

- 一度設定すれば毎回フラグを書かなくて済む
- ホワイトリストにより「消してはいけないキャッシュ」の保護を永続化できる
- カスタムパスで対応していないツールも sasurahime で管理できる

## 実装ステータス

**全機能: ✅ 完了**（一部スコープ制限あり）

- `exclude` フィールド — ✅ クリーナーをスキャン/TUI から除外
- `--config <path>` フラグ — ✅ 設定ファイルパスを上書き
- `[[custom]]` カスタムターゲット — ✅ 任意ディレクトリをスキャン対象に追加
- `[cleaner.<name>]` フィルタ — ✅ older_than_days / larger_than_mb のパースと適用

### スコープノート

Per-cleaner フィルタ（`older_than_days`, `larger_than_mb`）は **DeleteDirs 方式のクリーナー（act, colima, downloads 等）および LogCleaner** でのみ動作します。コマンドベースのクリーナー（uv, brew, bun 等）にフィルタが設定された場合は、実行時にワーニングが表示され、フィルタは無視されます。これは外部コマンドが内部のファイル選択を制御するため、sasurahime 側で介入できない制約によるものです。

## BDD 受け入れシナリオ

```gherkin
Scenario: ホワイトリストで指定したクリーナーが除外される
  Given ~/.config/sasurahime/config.toml に exclude = ["huggingface"] が設定されている
  When sasurahime scan を実行する
  Then huggingface クリーナーはスキャン対象から除外される
  And TUI でも huggingface はリストに表示されない
  # 実装メモ: all_cleaners() で exclude フィルタを retain() で適用。
  # sasurahime clean huggingface は直接呼べる（exclude は scan/TUI のみ）

Scenario: カスタムパスがスキャン対象に追加される
  Given config.toml に [[custom]] name = "my-cache" path = "~/work/.cache" が設定されている
  When sasurahime scan を実行する
  Then "my-cache" がスキャン結果テーブルに表示される
  And そのサイズが正しく計算される
  # 実装メモ: CustomPathCleaner が Cleaner trait を実装。
  # サブコンテンツのみ削除（ルートディレクトリは維持）
  # macOS uchg フラグを自動処理

Scenario: per-cleaner フィルタで古いキャッシュのみ対象にする
  Given config.toml の [cleaner.uv] に older_than_days = 30 が設定されている
  When sasurahime clean uv を実行する
  Then 30日以内に使用された uv キャッシュは削除されない
  And 30日より古いキャッシュのみ削除される
  # 制限: uv はコマンドベースクリーナーのためフィルタは適用されず、
  # ワーニングが表示される。DeleteDirs 方式の cleaner（act 等）でのみ有効。
  # LogCleaner は effective_logs_keep_days() を通じて反映。

Scenario: larger_than_mb フィルタで大きいキャッシュのみ対象にする
  Given config.toml の [cleaner.brew] に larger_than_mb = 500 が設定されている
  When sasurahime clean brew を実行する
  Then 500MB 未満のキャッシュは削除されない
  # 制限: brew はコマンドベースのためフィルタは適用されずワーニング表示。
  # DeleteDirs 方式の cleaner でのみ有効。

Scenario: config.toml が存在しない場合はデフォルト動作する
  Given ~/.config/sasurahime/config.toml が存在しない
  When sasurahime scan を実行する
  Then エラーなく通常のスキャンが実行される

Scenario: --config フラグで設定ファイルパスを上書きできる
  Given /tmp/custom-config.toml に設定が書かれている
  When sasurahime scan --config /tmp/custom-config.toml を実行する
  Then そのファイルの設定が適用される
  # 実装メモ: ファイルが存在しない場合はワーニング表示 + デフォルト動作
```

## 受け入れ基準

- [x] `~/.config/sasurahime/config.toml` を自動ロードする
- [x] `exclude = ["cleaner-name", ...]` でクリーナーをスキャン・TUI から除外できる
- [x] `[[custom]]` セクションで任意ディレクトリをスキャン対象に追加できる
- [x] `[cleaner.<name>]` セクションで `older_than_days` / `larger_than_mb` を設定できる
- [x] config.toml が存在しない場合はデフォルト動作（エラーなし）
- [x] `--config <path>` フラグで設定ファイルパスを上書きできる
- [x] TOML パースエラー時はわかりやすいエラーメッセージを表示して終了コード 1
- [ ] docs/HOWTO-USE.md に新しい設定項目のドキュメントを追記（exclude, custom, per-cleaner）

## t_wada スタイル テスト戦略

```
E2Eテスト（実装済み: tests/config.rs に 11 テスト）:
- exclude で指定したクリーナーが scan 出力から除外されることを検証
- 除外されたクリーナーでも direct clean は動作することを検証
- --config フラグでカスタムパスの設定が適用されることを検証
- カスタムパスのスキャン結果に my-workspace が表示されることを検証
- 存在しないカスタムパスが "not found" になることを検証
- older_than_days フィルタで新規ファイルが非表示になることを検証
- larger_than_mb フィルタで小ファイルが非表示になることを検証
- logs ターゲットの older_than_days フィルタが適用されることを検証

統合テスト（実装済み: src/config.rs + src/cleaners/generic.rs）:
- Config::load(path) のパース（default, exclude, per_cleaner, custom 等）
- Config::load_from_path(path) の明示的パス読み込み
- 各フィルタ（older_than_days / larger_than_mb）の適用ロジック
- GenericCleaner::DeleteDirs のフィルタリング動作
- exclude リストの適用
- Config::effective_logs_keep_days() の優先順位

単体テスト（実装済み）:
- Config のデフォルト値テスト（ファイルなし時）
- TOML パースエラーのエラーメッセージフォーマットテスト
- is_older_than(path, days): mtime 比較の純関数テスト
- meets_age_filter(metadata, older_than_days): メタデータフィルタテスト
- meets_size_filter(size, larger_than_mb): サイズフィルタテスト
- CustomPathCleaner の全パス（detect NotFound/Pruneable/Clean, dry_run, 実削除）
- expand_tilde のパス展開テスト
```

## 実装アプローチ

- **Outside-In**: `exclude` の E2E テストから開始し、1タスクずつ実装
- **Subagent-Driven Development**: 3タスクに分割し、各タスクで実装→Specレビュー→品質レビューのサイクルを実行
- **Red-Green-Refactor**: 各タスクで TDD サイクル

### Task 1: exclude + --config（1/3）
- `exclude: Vec<String>` を Config に追加
- `--config <path>` CLI フラグ追加
- `Config::load_from_path()` 実装
- `all_cleaners()` で exclude フィルタリング

### Task 2: [[custom]] カスタムターゲット（2/3）
- `CustomTarget` struct 追加
- `CustomPathCleaner`（src/cleaners/custom.rs）実装
- all_cleaners() に custom cleaner 追加

### Task 3: [cleaner.<name>] フィルタ（3/3）
- `PerCleanerConfig` struct（older_than_days, larger_than_mb）
- `HashMap<String, PerCleanerConfig>` で管理
- `is_older_than`, `meets_age_filter`, `meets_size_filter` ユーティリティ
- `GenericCleaner` に `with_config()`, `with_older_than()`, `with_larger_than()` builder
- DeleteDirs の detect/clean でフィルタ適用
- LogCleaner に older_than_days 連携

## Config 構造体

```toml
# ~/.config/sasurahime/config.toml

# スキャン/TUI から除外するクリーナー
exclude = ["huggingface", "ollama"]

# カスタムキャッシュターゲット
[[custom]]
name = "my-cache"
path = "~/work/.cache"

# per-cleaner フィルタ（DeleteDirs cleaners のみ有効）
[cleaner.act]
older_than_days = 30

[cleaner.colima]
larger_than_mb = 500

# logs cleaner の保持日数
[cleaner.logs]
older_than_days = 30  # keep_days として反映
```

## 変更履歴（PBI 更新）

| 日付 | 変更内容 |
|------|---------|
| 2026-05-25 | PBI-E 全機能実装完了。exclude, --config, [[custom]], [cleaner.<name>] フィルタ |

## 技術的考慮事項

- **依存関係**: `toml` クレート + `serde`（`serde_derive` feature）— 既存
- **パス展開**: `Config::expand_tilde()` — 既存の `~` 展開処理を custom target でも使用
- **`[cleaner.<name>]` の TOML パース**: `#[serde(rename = "cleaner")]` で `HashMap<String, PerCleanerConfig>` にマッピング
- **`Box::leak` パターン**: CustomPathCleaner の name フィールドは `Box::leak()` で `&'static str` に変換（プログラム起動ごとに1回生成されるため安全）
- **DeleteDirs のフィルタ**: detect() では `filter_map` で `dir_size()` を1回だけ計算し tuple で保持。clean() でも同様にフィルタリング
- **chflags nouchg**: CustomPathCleaner は削除前に `chflags -R nouchg` を実行（macOS immutable flag 対策）
- **コマンドベース cleaner のフィルタ制限**: `with_config()` が `CleanMethod::Command`/`CommandWithDetectDir` に対して警告を出力
- **Comprehensive test coverage**: 418 tests total（PBI-E 分: 270 unit/integration + 11 E2E）

## 見積もり

**5 SP**（Task 1: 2 SP, Task 2: 1.5 SP, Task 3: 1.5 SP）

## Definition of Done

- [x] exclude の E2E シナリオが通る
- [x] [[custom]] の E2E シナリオが通る
- [x] per-cleaner フィルタの統合テストが通る
- [x] --config フラグの E2E シナリオが通る
- [x] 既存テストとの互換性維持
- [x] `cargo test` 全パス（418 tests, 0 failures）
- [x] `cargo clippy -- -D warnings` クリーン
- [x] `cargo fmt --check` クリーン
- [x] コードレビュー完了（3 タスクそれぞれ spec + quality review × 2回）
- [ ] `docs/HOWTO-USE.md` に `exclude`, `[[custom]]`, `[cleaner.<name>]` の使い方を追記
