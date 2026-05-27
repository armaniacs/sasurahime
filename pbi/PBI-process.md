# PBI-process.md — sasurahime における PBI への取り組みパターン

対象: このプロジェクトを担当するすべての開発者・AIエージェント

---

## 1. PBI ファイルの命名規則

```
pbi/YYYY-MM-DD-NN-<type>-<slug>.md
```

| 部分 | 説明 | 例 |
|------|------|-----|
| `YYYY-MM-DD` | PBI 作成日 | `2026-05-25` |
| `NN` | 2桁連番（実施順） | `01`, `02`, ... |
| `type` | `fix` / `feat` / `backlog` | `fix` |
| `slug` | 内容を示す英語ケバブケース | `multi-vault-native-support` |

---

## 2. PBI ファイルの構造

新規 PBI は **スキル `pbi:create-bdd` で生成する**。手動で書かない。

生成されるセクション構成:

- ユーザーストーリー
- ビジネス価値
- BDD 受け入れシナリオ（Gherkin 形式）
- 受け入れ基準
- テスト戦略（t_wada スタイル）
- 実装アプローチ
- 見積もり（ストーリーポイント）
- 技術的考慮事項
- 実装者向け注記（現状確認コマンド・実装手順・落とし穴）
- Definition of Done

「実装者向け注記」には `grep` コマンドをそのまま実行できる形で既実装確認手順を含める。

---

## 3. PBI のライフサイクル

```
バックログ → 展開決定 → 実装(TDD) → レビュー(任意) → 完了 → アーカイブ
```

### バックログ段階

- `backlog-` プレフィックスで `pbi/` に配置
- ビジネス価値と受け入れ基準が明確であれば実装詳細は薄くて良い

### 展開決定時

- 連番を確定し、ファイル名を `fix-` または `feat-` プレフィックスに変更
- 必要に応じて `.plan/YYYY-MM-DD-<slug>-design.md` として設計ドキュメントを作成
  - DB スキーマ変更、API インターフェース、データフローを明記

### 実装（TDD）

- **着手前に `cargo test` でグリーンを確認**
- Red → Green → Refactor を各レイヤーで繰り返す
- **着手前に既存コードを必ず読む**（既実装済みの可能性に注意）

### レビュー（任意）

- `/checking-team` または `/code-review` で実施
- 新クリーナー追加・DB スキーマ変更・MCP インターフェース変更では推奨
- レビュー結果は `.plan/YYYY-MM-DD-review-<slug>.md` に保存

### 完了・アーカイブ

```bash
git mv pbi/2026-05-25-01-fix-example.md .plan/archived/
# .plan/00-INDEX.md の「アーカイブ一覧」に1行追記する
```

---

## 4. 既存コードとの照合（着手前の必須確認）

過去のパターンを分析すると「既に実装済み」ということもある。**PBI への着手前に必ず現状コードを確認すること。**

```bash
# 機能名に関連するキーワードでコードを探す
grep -r "keyword" src/

# 既存クリーナー一覧
ls src/cleaners/

# 既存テスト一覧
ls tests/

# 既存コマンド確認
cargo run -- --help
cargo run -- targets
```

---

## 5. TDD の進め方（このプロジェクト固有のルール）

### テスト階層（Outside-In）

1. **E2E テスト**: `tempdir` を HOME に見立ててバイナリまたはトップレベル関数を呼び出す。終了コードと stdout を検証。
2. **Integration テスト**: 偽ルートパスで `Cleaner` を構築し、`detect()` / `clean(dry_run=true)` を呼び出す。
3. **Unit テスト**: `parse_size_str`、`version_matches_spec`、`is_older_than` などの純粋関数。

### モックのルール

- 外部コマンド（uv, brew, bun, go, pip 等）は `CommandRunner` トレイトをインジェクションしてモック
- ファイルシステムは `tempdir` を使う（実ファイルシステムを直接操作しない）

### テストファイルの場所

| 種別 | 場所 |
|------|------|
| E2E / Integration | `tests/*.rs` |
| Unit | `src/` 内の各モジュール末尾 `#[cfg(test)]` |
| テスト共通ヘルパー | `src/test_helpers.rs` |

### 必須チェック（PR 前）

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

---

## 6. ブランチ・コミット戦略

### ブランチ名

```
<type>/<slug>
例: fix/mtime-two-stage-scan
    feat/frontmatter-filter
```

### コミットメッセージ（Conventional Commits）

```
feat(core): add mtime fast-path to incremental scan
fix(cli): handle empty vault error gracefully
test(db): add migration test for file_cache schema
docs(pbi): archive multi-vault-support PBI
```

---

## 7. ベストプラクティスと禁止事項

### やること

- 実装前に `cargo test` でグリーンを確認してから始める
- Epic 規模の PBI は着手前にシニアと設計を相談する
- mise ランタイム削除時は `~/.config/mise/config.toml` と HOME 以下の `.mise.toml`（深さ5まで）をクロスチェックする
- macOS の `uchg` 不変フラグ対策: `chflags -R nouchg <path>` してから `rm -rf`
- 外部ツールが PATH に無い場合は `NotFound` ステータスを返す（エラーにしない）

### やらないこと

- `detect()` または `clean(dry_run=true)` 内でファイルを削除する
- Linux / Windows 向けのコードを追加する（macOS 専用）
- 外部コマンドを `CommandRunner` トレイト経由でなく直接 `Command::new` で呼ぶ（テスト不可になる）

---

## 8. コマンドリファレンス

```bash
cargo build                    # ビルド
cargo test                     # 全テスト実行
cargo test <test_name>         # 単一テスト実行
cargo clippy -- -D warnings    # lint（警告ゼロが必須）
cargo fmt --check              # フォーマット確認
cargo fmt                      # フォーマット適用
cargo run -- scan              # スキャンレポート表示
cargo run -- targets           # 全クリーナー一覧
cargo run -- clean <target>    # 特定クリーナー実行
cargo run -- --yes             # 非インタラクティブ一括クリーン
cargo run -- explore           # ディレクトリ探索モード
cargo run -- stats             # 削除履歴・統計表示
```

---

## 9. 重要ファイルマップ（コードを読む出発点）

| 役割 | ファイル |
|------|---------|
| Cleaner トレイト定義 | `src/cleaner.rs` |
| 全クリーナー登録 | `src/cleaners/mod.rs` |
| 各クリーナー実装 | `src/cleaners/<name>.rs` |
| CLI エントリーポイント | `src/main.rs` |
| コマンドランナートレイト | `src/command.rs` |
| 設定ファイル読み込み | `src/config.rs` |
| インタラクティブ TUI | `src/interactive.rs` |
| テスト共通ヘルパー | `src/test_helpers.rs` |
| E2E・Integration テスト | `tests/*.rs` |
| 設計・アーカイブ | `.plan/archived/` |
