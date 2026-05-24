# sasurahime — プロダクトバックログ

macOS 開発環境の不要ファイルを安全に削除する Rust 製 CLI ツール。

## 実装済み

Sprint 1〜5 の全 PBI は `.plan/archived/` にアーカイブ済み。現在 32 のクリーンターゲットを提供する（`sasurahime targets` で一覧）。

## バックログ一覧

### フェーズ1: 堅牢性・UX改善（優先）

| PBI | タイトル | SP | 優先度 | 備考 |
|-----|---------|:--:|:------:|------|
| [PBI-A](2026-05-25-pbi-a-parallel-scan.md) | 並列スキャン最適化 | 3 | 🔴 高 | rayon 並列化・未インストール早期スキップ |
| [PBI-B](2026-05-25-pbi-b-robust-error-handling.md) | 堅牢なエラーハンドリング | 3 | 🔴 高 | 権限エラー・ファイルロックをスキップし失敗サマリー表示 |
| [PBI-C](2026-05-25-pbi-c-trash-warning-ui.md) | ゴミ箱移動の警告UI | 1 | 🟡 中 | 「ゴミ箱を空にするまで容量解放されない」を明示 |
| [PBI-D](2026-05-25-pbi-d-xcode-subcategory-selection.md) | Xcode サブカテゴリ選択 | 5 | 🟡 中 | DerivedData / Archives / Simulators を個別選択 |

### フェーズ2: 新機能

| PBI | タイトル | SP | 優先度 | 備考 |
|-----|---------|:--:|:------:|------|
| [PBI-E](2026-05-25-pbi-e-config-toml.md) | config.toml 統合設定 | 5 | 🔴 高 | カスタムパス・ホワイトリスト・per-cleaner フィルタ |
| [PBI-F](2026-05-25-pbi-f-yes-flag.md) | --yes フラグ | 2 | 🟡 中 | 非インタラクティブ全削除（cron/CI 向け）|
| [PBI-G](2026-05-25-pbi-g-stats-command.md) | sasurahime stats | 3 | 🟢 低 | 削除履歴ログ + 累計削減量表示 |

### 既存バックログ

| PBI | タイトル | SP | 優先度 | 実測サイズ | 備考 |
|-----|---------|:--:|:------:|:---------:|------|
| [Colima](2026-05-20-colima-cleaner.md) | Colima VM ディスクキャッシュ | 1 | 🔴 高 | **9.3 GB** | `colima prune --all` CLI委譲、実装計画済み |
| [追加候補一覧](2026-05-19-additional-cleaners-backlog.md) | ollama/simulator/maven/terraform/flutter 他 | — | 🟡 中 | 〜70GB | 環境に応じて実装判断 |
| [ドキュメント整備](2026-05-19-docs-deps-housekeeping.md) | README更新、cargo update | — | 🟢 低 | — | 依存関係整理 + ドキュメント同期 |

**合計**: 22 SP（フェーズ1+2）+ 既存 1 SP

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
