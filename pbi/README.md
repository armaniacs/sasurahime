# sasurahime — プロダクトバックログ

macOS 開発環境の不要ファイルを安全に削除する Rust 製 CLI ツール。

## バックログ一覧

| PBI | タイトル | SP | Sprint | 備考 |
|-----|---------|----|----|------|
| [PBI-001](PBI-001-scan-report.md) | スキャン & サマリーレポート | 3 | 1 | |
| [PBI-002](PBI-002-uv-cache.md) | uv キャッシュクリーナー | 2 | 1 | 期待回収 ~18GB ※ |
| [PBI-003](PBI-003-homebrew-cache.md) | Homebrew キャッシュクリーナー | 2 | 1 | 期待回収 ~16GB ※ |
| [PBI-004](PBI-004-mise-runtimes.md) | mise 旧ランタイム検出・削除 | 4 | 2 | `mise ls --current` 方式 |
| [PBI-005](PBI-005-playwright-puppeteer.md) | Playwright/Puppeteer 旧ブラウザ削除 | 3 | 2 | 期待回収 ~2.4GB ※ |
| [PBI-011](PBI-011-config-file.md) | 設定ファイルサポート | 3 | 2 | `~/.config/sasurahime/config.toml` |
| [PBI-006](PBI-006-generic-caches.md) | 汎用キャッシュ (bun/go/pip/node-gyp) | 3 | 3 | CLI 対称化済み |
| [PBI-007](PBI-007-log-cleanup.md) | ログファイル自動整理 | 2 | 3 | kilo/opencode/claude-code |
| [PBI-009](PBI-009-npm-yarn-pnpm.md) | npm / yarn / pnpm キャッシュ | 3 | 3 | |
| [PBI-010](PBI-010-xcode-derived-data.md) | Xcode DerivedData | 2 | 3 | |
| [PBI-008](PBI-008-interactive-mode.md) | インタラクティブ TUI モード | 3 | 4 | |

**合計**: 30 SP

> ※ 期待回収サイズは実装者 araki の環境での実測値。あくまでも一例であり、使用ツールや運用期間によって大きく異なります。

## スプリント計画

### Sprint 1 — MVP
PBI-001 + 002 + 003: `sasurahime scan` と最大効果の 2 クリーナーで動くものを作る。

### Sprint 2 — 安全性と設定基盤
PBI-004 + 005 + 011: mise の旧ランタイム削除（安全策が核心）と設定ファイル基盤を整える。

### Sprint 3 — 全クリーナー揃える
PBI-006 + 007 + 009 + 010: 残りのキャッシュ、ログ汎化、Xcode。`Cleaner` trait の設計が安定していることを確認してから進める。

### Sprint 4 — TUI
PBI-008: 全クリーナーを統合した対話型モード。

## 設計方針

- **安全性優先**: `--dry-run` を全クリーナーに実装。誤削除は絶対に避ける
- **Cleaner trait**: 全クリーナーが共通インターフェイスを実装し PBI-008 に備える
- **CommandRunner trait**: 外部コマンド（mise, brew 等）をモック可能にしてテスト容易性を確保
- **Outside-In TDD**: E2E テスト（tmpdir + モックコマンド）から開始し内側へ
- **macOS 専用**: Apple Silicon (arm64) + Intel (x86_64)、Sonoma 以降を想定
