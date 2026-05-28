# sasurahime — プロダクトバックログ

macOS 開発環境の不要ファイルを安全に削除する Rust 製 CLI ツール。

## 全 PBI 完了 ✅

Sprint 1〜5 の全 7 PBI（合計 22 SP）が完了し、`v0.1.27` としてリリース済み。
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

**現在のスペック:**
- **49 クリーンターゲット**（`sasurahime targets` で一覧）
- **507 tests, 0 failures**（345 unit + 162 integration/E2E、25 テストファイル）
- **バイナリサイズ: 872KB**（LTO + panic=abort + strip 最適化済み）

---

## バックログ

Checking Team Review (2026-05-28, 総合スコア 90/100) の未修正 Medium 指摘から作成した 9 PBI。

| PBI | タイトル | SP | 種別 | 概要 | 状態 |
|:---:|---------|:--:|:----:|------|:----:|
| 01 | is_skippable_error 精度向上 | 1 | fix | 文字列マッチングの誤検出防止 | ✅ |
| 02 | Gradle/JetBrains Trash 対応 | 1 | fix | `fs::remove_dir_all` → `trash::delete_path` | ✅ |
| 03 | E2E test パラメータ化 | 2 | fix | 17件の "tool not found" テスト重複 + VerboseGuard 移行 | ✅ |
| 08 | プライバシー文書 | 1 | fix | README Privacy セクション + iOS 警告文 | ✅ |
| 09 | デッドコードクリーンアップ | 1 | fix | `#[allow(dead_code)]` の整理 | ✅ |
| 10 | gem cleaner | 1 | feat | `gem cleanup` ラッパー | ✅ |
| 11 | bundle cleaner | 1 | feat | `bundle clean` ラッパー | ✅ |
| 12 | dotnet cleaner | 1 | feat | `dotnet nuget locals all --clear` ラッパー | ✅ |
| **04** | detect/clean キャッシュ | 3 | fix | Cargo/Mise の二重 HOME walk 最適化 | ⏳ |
| **05** | 構造化ログ | 5 | fix | env_logger + 監査証跡 + stderr loss | ⏳ |
| **06** | Cleaner トレイト契約統一 | 3 | fix | LibraryLogsCleaner clean_all の統合 | ⏳ |
| **07** | main.rs 登録システム整理 | 5 | fix | マクロ三重管理 + exit_code 重複 | ⏳ |

**完了: 8 PBI（合計 9 SP）／ 残り: 4 PBI（合計 16 SP）**

## 設計方針（不変）

- **安全性優先**: `--dry-run` を全クリーナーに実装
- **Cleaner trait**: 全クリーナーが共通インターフェイスを実装
- **CommandRunner trait**: 外部コマンドをモック可能にしてテスト容易性を確保
- **Outside-In TDD**: E2E テストから開始し内側へ
- **macOS 専用**: Apple Silicon (arm64) + Intel (x86_64)
