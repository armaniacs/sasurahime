# sasurahime バックログ設計ドキュメント

**作成日**: 2026-05-17  
**ステータス**: 承認済み

## 背景

このドキュメントは `/brainstorming` セッションで確定した PBI の変更・追加をまとめたものです。PBI-001〜008 の初期定義に対してレビューを行い、以下の修正と追加を決定しました。

---

## 確定した変更

### PBI-004: mise 旧ランタイム検出・削除

**変更前**: `~/.config/mise/config.toml` と `.mise.toml` を自前でパースして使用バージョンを判定。

**変更後**: `mise ls --current` コマンド経由で使用バージョンを取得する。

**理由**:
- mise は `.mise.toml`・`.tool-versions`（asdf 互換）・環境変数など複数のソースを参照する。これを自前実装すると mise の仕様変更に追従し続ける必要がある
- `mise ls --current` に委ねることで、mise 自身の判断を尊重し、`.tool-versions` 対応も自動的に解決する
- デメリット: mise コマンドが必要（mise 未インストール環境ではスキップ）

```
mise ls --current
# 出力例:
# node  24.15.0  ~/.config/mise/config.toml  24
# python  3.12.11  ~/.config/mise/config.toml  3.12.11
```

アクティブバージョンのセットを取得後、`~/.local/share/mise/installs/<tool>/` 以下のバージョンと照合し、未参照のものを削除候補とする。

---

### PBI-006: 汎用キャッシュクリーナー — CLI 対称化

**変更前**: `sasurahime clean caches --only bun,go` という形でのみ個別指定可能。

**変更後**: 各ツールが独立したサブコマンドを持つ。`clean caches` はグループエイリアス。

```bash
sasurahime clean bun        # bun のみ
sasurahime clean go         # go のみ
sasurahime clean pip        # pip のみ
sasurahime clean node-gyp   # node-gyp のみ
sasurahime clean caches     # 上記を全て実行（エイリアス）
```

**理由**: 他の全クリーナー（uv, brew, mise, browsers）が `sasurahime clean <name>` 形式なのに、bun/go/pip だけ異なる形式にする根拠がない。CLI の一貫性を優先する。

---

### PBI-007: ログファイル自動整理 — kilo 専用から汎化

**変更前**: kilo のログディレクトリのみをハードコード。

**変更後**: 複数ツールのログディレクトリを対象とし、設定ファイル（PBI-011）で拡張可能にする。

**初期ターゲット（ハードコード）**:

| ツール | パス | 除外パターン |
|--------|------|------------|
| kilo | `~/.local/share/kilo/log/` | `dev.log` |
| opencode | `~/.local/share/opencode/logs/` | — |
| claude-code | `~/.local/share/claude/logs/` | — |

`LogTarget { name, path, pattern, exclude }` 構造体で定義し、PBI-011 の設定ファイルからユーザーが追加できるよう設計する。

---

## 追加 PBI

### PBI-009: npm / yarn / pnpm キャッシュクリーナー

**独立 PBI とする理由**: Node.js エコシステムのパッケージマネージャーは複数が共存しやすく（npm + pnpm 混在など）、検出ロジックと存在チェックが bun/go/pip より複雑。

**対象**:

| ツール | コマンド | キャッシュパス |
|--------|---------|--------------|
| npm | `npm cache clean --force` | `~/.npm` |
| yarn | `yarn cache clean` | `~/.yarn/cache` |
| pnpm | `pnpm store prune` | pnpm が管理 |

**CLI**: `sasurahime clean npm` / `sasurahime clean yarn` / `sasurahime clean pnpm`（個別）+ `sasurahime clean caches` のグループに含める。

**SP見積もり**: 3

---

### PBI-010: Xcode DerivedData クリーナー

**対象**: `~/Library/Developer/Xcode/DerivedData/`

削除ロジックはシンプル（直接 rm）だが、サイズが数十GB になりうるため影響大。

**考慮点**:
- DerivedData はビルドキャッシュのみで、プロジェクト設定やソースは含まれない
- Simulator データ（`~/Library/Developer/CoreSimulator`）は対象外（別の判断が必要なため）
- Xcode が起動中の場合の警告表示を検討

**CLI**: `sasurahime clean xcode`

**SP見積もり**: 2

---

### PBI-011: 設定ファイルサポート

**配置**: `~/.config/sasurahime/config.toml`

**Sprint 2 に追加する理由**: ログ保持日数・ログターゲット追加・除外パスなどがハードコードされており、後から設定ファイルに外出しする改修コストを避けるため。

**設定項目（初期スコープ）**:

```toml
[logs]
keep_days = 7           # デフォルト保持日数

[[logs.targets]]
name = "kilo"
path = "~/.local/share/kilo/log"
exclude = ["dev.log"]

[[logs.targets]]
name = "my-tool"        # ユーザー追加
path = "~/.local/share/my-tool/log"
```

**動作**:
- 設定ファイルが存在しない場合はデフォルト値で動作（設定ファイルは必須ではない）
- CLI フラグ（`--keep-days` 等）は設定ファイルの値を上書きする

**SP見積もり**: 3

---

## 更新後のバックログ全体

| PBI | タイトル | SP | Sprint | 変更 |
|-----|---------|----|----|------|
| 001 | スキャン & サマリーレポート | 3 | 1 | — |
| 002 | uv キャッシュクリーナー | 2 | 1 | — |
| 003 | Homebrew キャッシュクリーナー | 2 | 1 | — |
| 004 | mise 旧ランタイム検出・削除 | 5 | 2 | `mise ls --current` 方式に変更 |
| 005 | Playwright/Puppeteer 旧ブラウザ削除 | 3 | 2 | — |
| 011 | 設定ファイルサポート | 3 | 2 | **新規** |
| 006 | 汎用キャッシュ (bun/go/pip/node-gyp) | 3 | 3 | CLI 対称化 |
| 007 | ログファイル自動整理 | 2 | 3 | kilo 専用 → 汎化 |
| 009 | npm / yarn / pnpm キャッシュ | 3 | 3 | **新規** |
| 010 | Xcode DerivedData | 2 | 3 | **新規** |
| 008 | インタラクティブ TUI モード | 3 | 4 | — |

**合計**: 31 SP（旧 23 SP から +8）

---

## アーキテクチャへの影響

すべてのクリーナーが `Cleaner` trait を実装するという方針は変わらない。今回の変更で追加されるのは:

1. **`CommandRunner` trait** — `mise ls`, `npm cache clean` 等の外部コマンドをモック可能にする（テスト容易性）
2. **`Config` 構造体** — PBI-011 で導入。各クリーナーが `Config` を受け取り、設定値を参照する
3. **サブコマンドの拡張** — bun/go/pip/node-gyp/npm/yarn/pnpm が個別サブコマンドを持つため、`clap` のサブコマンド定義が増える。グループエイリアス（`clean caches`）は対象クリーナーのリストを持つ

## 未決定事項

- Cargo レジストリキャッシュ（`~/.cargo/registry/cache`）は今回スコープ外。ユーザーからの要望があれば PBI-012 として追加を検討。
- `sasurahime clean caches` のグループ定義（npm/yarn/pnpm を含めるか）は PBI-009 実装時に確定。
