# sasurahime — プロダクトバックログ

macOS 開発環境の不要ファイルを安全に削除する Rust 製 CLI ツール。

## 実装済み

Sprint 1〜5 の全 PBI は `.plan/archived/` にアーカイブ済み。現在 32 のクリーンターゲットを提供する（`sasurahime targets` で一覧）。

## バックログ一覧

| PBI | タイトル | SP | 優先度 | 実測サイズ | 備考 |
|-----|---------|:--:|:------:|:---------:|------|
| [Colima](2026-05-20-colima-cleaner.md) | Colima VM ディスクキャッシュ | 1 | 🔴 高 | **9.3 GB** | `colima prune --all` CLI委譲、実装計画済み |
| [追加候補一覧](2026-05-19-additional-cleaners-backlog.md) | ollama/simulator/maven/terraform/flutter 他 | — | 🟡 中 | 〜70GB | 環境に応じて実装判断 |
| [ドキュメント整備](2026-05-19-docs-deps-housekeeping.md) | README更新、cargo update | — | 🟢 低 | — | 依存関係整理 + ドキュメント同期 |

**合計**: 1 SP（確定）+ 将来検討項目

## 優先度ランキング（実環境でのディスク影響ベース）

| 順位 | クリーナー | 想定回収サイズ | 判断基準 |
|:---:|----------|:------------:|---------|
| 1 | Colima | 9.3 GB | このマシンで実測 |
| 2 | ollama | 1〜70 GB/モデル | 未インストールだが影響大 |
| 3 | simulator | 1〜10 GB | xcrun インストール済み、データ未確認 |
| 4 | VSCode 拡張キャッシュ | 1.1 GB | 新規候補（バックログ外） |
| 5〜9 | maven/terraform/flutter 等 | 〜数GB | 未インストール |

## 設計方針（不変）

- **安全性優先**: `--dry-run` を全クリーナーに実装
- **Cleaner trait**: 全クリーナーが共通インターフェイスを実装
- **CommandRunner trait**: 外部コマンドをモック可能にしてテスト容易性を確保
- **Outside-In TDD**: E2E テストから開始し内側へ
- **macOS 専用**: Apple Silicon (arm64) + Intel (x86_64)
