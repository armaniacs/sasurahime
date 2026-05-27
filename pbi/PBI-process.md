# PBI-process.md — shiotsuchi-search における PBI への取り組みパターン

作成日: 2026-05-25
対象: このプロジェクトを担当するすべての開発者（ジュニア含む）

---

## 1. PBI ファイルの命名規則

```
pbi/YYYY-MM-DD-NN-<type>-<slug>.md
```

| 部分 | 説明 | 例 |
|------|------|-----|
| `YYYY-MM-DD` | PBI 作成日 | `2026-05-25` |
| `NN` | 2桁連番（実施順） | `01`, `02`, ... |
| `type` | `fix`（既存問題の修正）/ `feat`（新機能）/ `backlog`（将来候補） | `fix` |
| `slug` | 内容を示す英語ケバブケース | `multi-vault-native-support` |

ファイル名だけで「何番目に取り組む何の作業か」がわかるようにする。

---

## 2. PBI ファイルの構造

各 PBI ファイルは以下のセクションを持つ。

```markdown
# PBI: <タイトル>

## ユーザーストーリー
<ロール>として、<機能>がほしい、なぜなら<ビジネス価値>だから

## ビジネス価値

## BDD 受け入れシナリオ
(Gherkin 形式)

## 受け入れ基準
- [ ] 検証可能な条件

## テスト戦略（t_wada スタイル）

## 実装アプローチ

## 見積もり（ストーリーポイント）

## 技術的考慮事項

## 実装者向け注記（ジュニア開発者必読）
### 現状コードの確認
### 実装手順
### 落とし穴

## Definition of Done
```

### 「実装者向け注記」は必須

ジュニア開発者が単独で着手できるよう、以下を必ず含める：

1. **現状コードの確認コマンド**（`grep` コマンドをそのまま実行できる形で）
2. **既実装かどうかの明示**（「既に実装済み」なら調査タスクとして書き直す）
3. **実装手順**（コードスニペット付きで具体的に）
4. **落とし穴**（ハマりやすい箇所を具体的に）

---

## 3. PBI のライフサイクル

```
バックログ → 展開決定 → 設計 → 実装(TDD) → レビュー(任意) → 完了 → アーカイブ
```

### バックログ段階

- `backlog-` プレフィックスで `pbi/` に配置する
- 実装の詳細は薄くて良い（ビジネス価値と受け入れ基準が明確であれば十分）

### 展開決定時

- 連番を確定し、ファイル名を `fix-` または `feat-` プレフィックスに変更する
- **設計ドキュメントを `.plan/YYYY-MM-DD-<slug>-design.md` として作成する**
  - DB スキーマ変更、API インターフェース、データフローを明記する
  - 設計仕様を PBI の「実装者向け注記」にも反映する

### 実装（TDD）

- **必ず `cargo test -p shiotsuchi-core` をグリーンにしてから次に進む**
- Red → Green → Refactor を各レイヤーで繰り返す
- 実装前に既存コードを必ず読む（特に「既実装済み」の可能性に注意）

### レビュー（任意）

- `/checking-team` または `/code-review` で実施
- 大きな変更（新コマンド追加、DBスキーマ変更、MCP インターフェース変更）では実施を推奨
- レビュー結果は `.plan/YYYY-MM-DD-review-<slug>.md` に保存する

### 完了・アーカイブ

- PBI ファイルを `.plan/archived/` に `git mv` で移動する
- `.plan/00-INDEX.md` の「アーカイブ一覧」に1行追記する
- 関連する設計ドキュメントも同様にアーカイブする

```bash
# アーカイブコマンド例
git mv pbi/2026-05-25-01-fix-mtime-size-two-stage-scan.md .plan/archived/
```

---

## 4. 既存コードとの照合（着手前の必須確認）

### なぜ必要か

過去のパターンを分析すると、「既に実装済み」ということもある

**PBI への着手前に必ず現状コードを確認すること。**

### 着手前の確認（テンプレート）

- 機能名に関連するキーワードでコードを探す
- 既存テストの確認
- 既存のコマンド一覧確認

---

## 5. TDD の進め方（このプロジェクト固有のルール）

### テスト実行コマンド

```bash
cargo test -p shiotsuchi-core   # コア単体テスト
cargo test -p shiotsuchi-cli    # CLI 単体テスト
cargo test                       # ワークスペース全体
cargo bench -p shiotsuchi-core  # ベンチマーク
```

### テストの配置場所

| レイヤー | 場所 |
|---------|------|
| コア単体テスト | `core/src/*.rs` の `#[cfg(test)] mod tests` |
| CLI 単体テスト | `cli/src/**/*.rs` の `#[cfg(test)] mod tests` |
| E2E テスト | `e2e/` クレート |

### Rust 固有の制約

- **FTS5 仮想テーブルには `ALTER TABLE ADD COLUMN` が使えない**
  スキーマ変更はマイグレーション関数（`core/src/db.rs` の `migrate_schema` 系）に追加する
- **Rayon 並列処理がある**
  スレッドセーフでない操作を並列クロージャに渡さないこと
- **`ort` は build time に ONNX バイナリをダウンロードする**
  CI ではキャッシュが効いているか確認する

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
- DB スキーマ変更は必ずマイグレーション関数を書く（既存 DB が壊れる）
- MCP インターフェース変更は後方互換を保つ（フィールド追加は OK、型変更は NG）
- Epic 規模の PBI（PBI-21 OCR、PBI-27 Obsidian プラグイン等）は着手前にシニアと設計を相談する

### やらないこと

- FTS5 仮想テーブルへの `ALTER TABLE ADD COLUMN`（エラーになる）
- `git add .` / `git add -A`（機密ファイルや大きなバイナリを含むリスクがある）
- バイト単位の日本語文字列操作（UTF-8 境界を壊す。`str` のメソッドを使う）
- 本番コードに `unwrap()` を残す
- embedder なしで `SearchMode::Vec` を呼ぶ（`Option<&Embedder>` を必ず確認する）

---

## 8. コマンドリファレンス

| コマンド | 用途 |
|---------|------|
| `shiotsuchi chart` | インデックス作成・更新 |
| `shiotsuchi dive <query>` | 検索（`search` はエイリアス） |
| `shiotsuchi scan` | ファイルシステム監視 |
| `shiotsuchi tide` | 統計情報表示 |
| `shiotsuchi doctor` | 環境ヘルスチェック |
| `shiotsuchi clean` | バックアップ + 再インデックス |
| `shiotsuchi config-migrate` | 設定ファイルフォーマット移行 |

---

## 9. 重要ファイルマップ（コードを読む出発点）

| ファイル | 役割 |
|---------|------|
| `core/src/db.rs` | SQLite スキーマ・クエリ・マイグレーション |
| `core/src/indexer.rs` | ファイルウォーキング・インデックスロジック |
| `core/src/search.rs` | FTS5/Vec/Hybrid 検索、スニペット抽出 |
| `core/src/tokenizer.rs` | Vaporetto トークナイザー |
| `core/src/embedder.rs` | ONNX 埋め込みモデル |
| `core/src/chunker.rs` | Markdown チャンク分割 |
| `core/src/models.rs` | データ構造定義（Chunk, ChunkSearchResult, VaultStats 等） |
| `core/src/config.rs` | 設定スキーマ（ShiotsuchiConfig, VaultEntry 等） |
| `cli/src/main.rs` | CLI エントリポイント・コマンド定義 |
| `cli/src/commands/dive.rs` | 検索コマンド実装 |
| `mcp/src/handler.rs` | MCP ツールハンドラー |
