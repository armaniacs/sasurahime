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
- **49 クリーンターゲット**（`sasurahime targets` で一覧）
- **346 tests, 0 failures**（全 unit, 25 テストファイル）
- **バイナリサイズ: 872KB**（LTO + panic=abort + strip 最適化済み）

## 設計方針（不変）

- **安全性優先**: `--dry-run` を全クリーナーに実装
- **Cleaner trait**: 全クリーナーが共通インターフェイスを実装
- **CommandRunner trait**: 外部コマンドをモック可能にしてテスト容易性を確保
- **Outside-In TDD**: E2E テストから開始し内側へ
- **macOS 専用**: Apple Silicon (arm64) + Intel (x86_64)
