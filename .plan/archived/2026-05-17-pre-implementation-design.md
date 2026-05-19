# 実装前準備 設計ドキュメント

**作成日**: 2026-05-17  
**ステータス**: 承認済み

## 概要

コードを一行も書く前に完了させるべき作業の定義。  
完了後に `writing-plans` で Sprint 1 の実装計画を作成する。

---

## 1. git 整理

未コミットの `.gitignore` 変更をコミットする。

```bash
git add .gitignore
git commit -m "chore: update .gitignore for Rust project"
```

---

## 2. Rust プロジェクト初期化

### cargo new

```bash
cargo new sasurahime --name sasurahime
```

生成される構造:
```
sasurahime/
├── Cargo.toml
└── src/
    └── main.rs
```

### Cargo.toml 依存クレート

| クレート | バージョン | 用途 |
|---------|-----------|------|
| `clap` | 4, features=["derive"] | CLI サブコマンド定義 |
| `anyhow` | 1 | エラーハンドリング（全クリーナー共通） |
| `walkdir` | 2 | ディレクトリ再帰スキャン |
| `indicatif` | 0.17 | スキャン中のプログレス表示 |
| `dialoguer` | 0.11 | インタラクティブ TUI（PBI-008）|
| `comfy-table` | 7 | scan 結果の表形式出力 |
| `dirs` | 5 | ホームディレクトリ取得（`~` 展開）|
| `toml` | 0.8 | 設定ファイルパース（PBI-011）|
| `serde` | 1, features=["derive"] | TOML デシリアライズ |

dev-dependencies:

| クレート | バージョン | 用途 |
|---------|-----------|------|
| `tempfile` | 3 | E2E・統合テスト用 tmpdir |
| `assert_cmd` | 2 | CLI プロセス起動 E2E テスト |

---

## 3. CONTRIBUTING.md

以下の内容を記載する:

- **ブランチ命名**: `feat/PBI-XXX-description` / `fix/description` / `chore/description`
- **コミットメッセージ**: `feat:` / `fix:` / `chore:` / `test:` / `docs:` プレフィックス
- **PR**: 1 PBI = 1 PR を基本とする。PBI 番号を PR タイトルに含める
- **Issue**: 機能提案は Issue で先に議論する
- **テスト**: PR には対応するテストを含める（`cargo test` が green であること）
- **Lint**: `cargo clippy -- -D warnings` と `cargo fmt` が通ること

---

## 4. GitHub Actions CI

`.github/workflows/ci.yml` を作成する。

**トリガー**: `push`（全ブランチ）および `pull_request`

**ランナー**: `macos-latest`（macOS 専用ツールのため Linux runner は使わない）

**ジョブ**:
1. `cargo fmt --check`
2. `cargo clippy -- -D warnings`
3. `cargo test`

キャッシュ: `actions/cache` で `~/.cargo` と `target/` をキャッシュする。

---

## 5. Sprint 1 実装計画

上記 1〜4 完了後、`writing-plans` スキルで Sprint 1（PBI-001 + 002 + 003）の詳細タスクに分解する。

### Sprint 1 スコープ

| PBI | 内容 |
|-----|------|
| PBI-001 | スキャン & サマリーレポート |
| PBI-002 | uv キャッシュクリーナー |
| PBI-003 | Homebrew キャッシュクリーナー |

実装順: `Cleaner` trait 定義 → PBI-001（スキャン基盤）→ PBI-002（uv）→ PBI-003（brew）

---

## 完了条件

- [ ] `.gitignore` がコミット済み
- [ ] `Cargo.toml` が存在し依存クレートが定義されている
- [ ] `CONTRIBUTING.md` が存在する
- [ ] `.github/workflows/ci.yml` が存在し push で green になる
- [ ] GitHub リモートリポジトリが作成され push 済み
- [ ] Sprint 1 の実装計画が `docs/superpowers/plans/` に存在する
