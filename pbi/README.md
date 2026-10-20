# sasurahime — プロダクトバックログ

macOS 開発環境の不要ファイルを安全に削除する Rust 製 CLI ツール。

## 全 PBI 完了 ✅

Sprint 1〜5 の全 7 PBI（22 SP）、および PBI 01〜12（25 SP）が完了し、`v0.2.0` としてリリース済み。
各 PBI の詳細は `.plan/archived/` にアーカイブされています。

| PBI | タイトル | SP | 概要 |
|:---:|---------|:--:|------|
| A | 並列スキャン最適化 | 3 | `rayon` による並列スキャン、未インストールツールの早期スキップ |
| B | 堅牢なエラーハンドリング | 3 | 権限エラー・ファイルロックをスキップ、サマリー表示、終了コード分岐 |
| C | ゴミ箱移動の警告UI | 1 | Trash モード時の明示的な警告表示、大容量ファイルの事前警告 |
| D | Xcode サブカテゴリ選択 | 5 | DerivedData/Archives の部分削除（CLI + TUI）|
| E | config.toml 統合設定 | 5 | exclude, --config, [[custom]], per-cleaner フィルタ |
| F | --yes フラグ | 2 | 非インタラクティブ一括削除（cron/CI 対応）|
| G | sasurahime stats | 3 | 削除履歴の自動記録 + 統計表示 |
| 01 | is_skippable_error 精度向上 | 1 | fix | 文字列マッチングの誤検出防止 |
| 02 | Gradle/JetBrains Trash 対応 | 1 | Trash モード一貫性 |
| 03 | E2E test パラメータ化 | 2 | rstest 導入、VerboseGuard 移行 |
| 04 | detect/clean キャッシュ | 3 | OnceLock で二重 walk 排除 |
| 05 | 構造化ログ | 5 | env_logger + 監査証跡 |
| 06 | Cleaner トレイト契約統一 | 3 | clean_with_opts 追加 |
| 07 | main.rs 登録システム整理 | 5 | cmd_name! 削除、exit_code 重複解消 |
| 08 | プライバシー文書 | 1 | README Privacy セクション |
| 09 | デッドコードクリーンアップ | 1 | #[allow(dead_code)] 一掃 |
| 10 | gem cleaner | 1 | `gem cleanup` ラッパー |
| 11 | bundle cleaner | 1 | `bundle clean` ラッパー |
| 12 | dotnet cleaner | 1 | `dotnet nuget locals all --clear` ラッパー |

**現在のスペック:**
- **48 クリーンターゲット**（`sasurahime targets` で一覧。2026-10-20 時点の実測値）
- **346 tests, 0 failures**（全 unit, 25 テストファイル）
- **バイナリサイズ: 872KB**（LTO + panic=abort + strip 最適化済み）

## システムデータ肥大化対策（2026-10-20 設計）

設計書は `docs/superpowers/specs/2026-10-20-*.md`。実装順は固定で、並行して進めない。上から順に着手し、前提 PBI が完了していない PBI には着手しない。

| 順 | PBI | タイトル | SP | 前提 | 設計書 |
|:--:|-----|---------|:--:|------|--------|
| 1 | 2026-10-20-01-fix-readme-data-access-facts | README Data access を実際の挙動に合わせる | 1 | なし | 01-roadmap の 7 章 (P0) |
| 2 | 2026-10-20-02-feat-scan-coverage | scan に標準ターゲット全件を表示、--yes は従来の15件に限る | 5 | PBI-01 | 04-docker の S1 から S3 |
| 3 | 2026-10-20-03-feat-docker-disk-image-notice | Docker と Colima の仮想ディスクの実サイズ表示と縮小案内 | 5 | PBI-02 | 04-docker の D1 から T5 |
| 4 | 2026-10-20-04-feat-orphan-containers | Containers 残骸の検出とゴミ箱送り | 8 | PBI-02 (固定順で PBI-03 の後) | 02-orphan-containers |
| 5 | 2026-10-20-05-feat-library-caches | ~/Library/Caches の既知アプリを選んで削除 | 5 | PBI-04 | 03-library-caches |
| 6 | 2026-10-20-06-feat-app-support-cache-cleanup | Application Support の許可リスト方式キャッシュ削除 | 8 | PBI-05 | 05-app-support の Task 1 から 7 |
| 7 | 2026-10-20-07-feat-app-support-leftover-report | Application Support の残骸表示と Finder 起動 | 5 | PBI-04, PBI-06 | 05-app-support の Task 8 から 13 |

完了ごとの `sasurahime targets` の行数: PBI-01 から 03 は 48、PBI-04 で 49、PBI-05 で 50、PBI-06 で 51 (PBI-07 は変わらず 51)。

### バックログ（上の PBI の範囲外。着手順は未定）

| PBI | タイトル | SP | 前提 |
|-----|---------|:--:|------|
| 2026-10-20-08-backlog-explorer-open-helper | Finder 起動 (open) を CommandRunner 経由の共通ヘルパーにまとめる | 1 | PBI-07 の後 |
| 2026-10-20-09-backlog-warn-ineffective-per-cleaner-config | 効かない per-cleaner 設定を起動時にまとめて警告する | 3 | PBI-02 の後 |
| 2026-10-20-10-backlog-ios-backup-yes-prompt | --yes で ios-backup / apfs-snapshot の確認プロンプトが出る問題を解消する | 1 | PBI-02 の後 |
| 2026-10-20-11-backlog-clean-result-trash-message | clean 結果の「moved to Trash」表示を CleanResult.uses_trash に基づかせる | 1 | なし |
| 2026-10-20-12-backlog-apfs-snapshot-followups | apfs-snapshot の軽微な積み残しを整理する | 2 | なし |

## 設計方針（不変）

- **安全性優先**: `--dry-run` を全クリーナーに実装
- **Cleaner trait**: 全クリーナーが共通インターフェイスを実装
- **CommandRunner trait**: 外部コマンドをモック可能にしてテスト容易性を確保
- **Outside-In TDD**: E2E テストから開始し内側へ
- **macOS 専用**: Apple Silicon (arm64) + Intel (x86_64)
