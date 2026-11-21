# sasurahime ― プロダクトバックログ

macOS 開発環境の不要ファイルを安全に削除する Rust 製 CLI ツール。

## 完了した PBI

Sprint 1〜5 の全 7 PBI（22 SP）と PBI 01〜12（25 SP）は `v0.2.0` としてリリース済みです。
システムデータ肥大化対策の 12 PBI（45 SP）も `main` へのマージとリリースが完了しています。
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
- **51 クリーンターゲット**（`sasurahime targets` で一覧。システムデータ対策の完了後の実測値）
- **879 tests**（bin unit 634 + 統合/E2E 245、34 テストファイル）。実Trash 操作を伴う 5 件は環境依存で TCC 拒否になりうる

## 全体レビュー・拡張バックログ（2026-10-22 / 2026-10-23、全 15 PBI 完了）

全体レビュー（2026-10-22）と拡張（2026-10-23）由来の 15 PBI はすべて実装と検証が完了し、
`.plan/archived/` にアーカイブしました。実装はブランチ `1023a` で行い、`main` にマージ済みです。
実装コミットは `.plan/00-INDEX.md` のアーカイブ一覧に記載しています。

## システムデータ肥大化対策（2026-10-20 設計、全 12 PBI 完了）

設計書は `docs/superpowers/specs/2026-10-20-*.md`。実装順は固定で、前提 PBI が完了してから次に着手しました。
12 本すべて完了し、`.plan/archived/` に移動済みです。作業は複数のブランチに分かれており、
いずれも `main` にマージ済みです。

| 順 | PBI | タイトル | SP | 前提 | 設計書 | 状態 |
|:--:|-----|---------|:--:|------|--------|---|
| 1 | 2026-10-20-01-fix-readme-data-access-facts | README Data access を実際の挙動に合わせる | 1 | なし | 01-roadmap の 7 章 (P0) | 完了 |
| 2 | 2026-10-20-02-feat-scan-coverage | scan に標準ターゲット全件を表示、--yes は従来の15件に限る | 5 | PBI-01 | 04-docker の S1 から S3 | 完了 |
| 3 | 2026-10-20-03-feat-docker-disk-image-notice | Docker と Colima の仮想ディスクの実サイズ表示と縮小案内 | 5 | PBI-02 | 04-docker の D1 から T5 | 完了 |
| 4 | 2026-10-20-04-feat-orphan-containers | Containers 残骸の検出とゴミ箱送り | 8 | PBI-02 (固定順で PBI-03 の後) | 02-orphan-containers | 完了 |
| 5 | 2026-10-20-05-feat-library-caches | ~/Library/Caches の既知アプリを選んで削除 | 5 | PBI-04 | 03-library-caches | 完了 |
| 6 | 2026-10-20-06-feat-app-support-cache-cleanup | Application Support の許可リスト方式キャッシュ削除 | 8 | PBI-05 | 05-app-support の Task 1 から 7 | 完了 |
| 7 | 2026-10-20-07-feat-app-support-leftover-report | Application Support の残骸表示と Finder 起動 | 5 | PBI-04, PBI-06 | 05-app-support の Task 8 から 13 | 完了 |

完了ごとの `sasurahime targets` の行数: PBI-01 から 03 は 48、PBI-04 で 49、PBI-05 で 50、PBI-06 で 51 (PBI-07 は変わらず 51)。

### バックログ（すべて完了）

| PBI | タイトル | SP | 前提 | 状態 |
|-----|---------|:--:|------|---|
| 2026-10-20-08-backlog-explorer-open-helper | Finder 起動 (open) を CommandRunner 経由の共通ヘルパーにまとめる | 1 | PBI-07 の後 | 完了 |
| 2026-10-20-09-backlog-warn-ineffective-per-cleaner-config | 効かない per-cleaner 設定を起動時にまとめて警告する | 3 | PBI-02 の後 | 完了 |
| 2026-10-20-10-backlog-ios-backup-yes-prompt | --yes で ios-backup / apfs-snapshot の確認プロンプトが出る問題を解消する | 1 | PBI-02 の後 | 完了 |
| 2026-10-20-11-backlog-clean-result-trash-message | clean 結果の「moved to Trash」表示を CleanResult.uses_trash に基づかせる | 1 | なし | 完了 |
| 2026-10-20-12-backlog-apfs-snapshot-followups | apfs-snapshot の軽微な積み残しを整理する | 2 | なし | 完了 |

## GitHub Pages 改善（2026-11-16、全 4 PBI 完了）

docs 配下のアクセシビリティ・レスポンシブ・言語切替・SEO を段階的に改善した 4 PBI（S1-S4）。
PR #8 で `main` にマージし、`v0.4.7` としてリリース済みです。詳細は `.plan/archived/` を参照。

| PBI | タイトル | 概要 |
|:---:|---------|------|
| S1 | アクセシビリティ土台 | `<main>` ランドマーク、スキップリンク、caption/scope、aria 属性、:focus-visible |
| S2 | テーブルとレスポンシブ改善 | 横スクロールテーブル + sticky ヘッダー、`color-scheme: dark`、container queries 移行 |
| S3 | 言語切替 JS 再設計と /ja/ 下層整備 | data-lang + addEventListener 化、/ja/ 配下に主要ドキュメント 3 件を新設 |
| S4 | meta と SEO 整備 | Open Graph、canonical、hreflang (x-default 含む)、ページ固有 description、ユニーク title |

本番デプロイの実機検証 PBI（`2026-11-17-01`）も完了し、`.plan/archived/` にアーカイブ済みです。
発見事項（`superpowers/` 配下の本番配信、ローカル jekyll build の復旧）は修正 PBI の起票対象として検証記録に残っています。

## 大局的コード改善 round 9（2026-11-20、全 4 PBI 完了）

holistic-code-improvement スキルによる大局レビュー（DRY/SoC/拡張性/堅牢性）+ なぜなぜ分析
（3 立場 × 30 段超 × 3 グループ）で RICE 採点した 4 PBI。実行順は依存優先で
Batch 1（01∥04 並列）→ 02 → 03。最終ゲート: lib 903 passed（既知 TCC 依存 1 件のみ）、
統合テスト 43 ターゲット全 ok、clippy 0、fmt clean、build ok。
詳細は `.plan/archived/` と台帳 `2026-11-20-00-backlog-holistic-round9.md` を参照。

| PBI | タイトル | 概要 |
|:---:|---------|------|
| 01 | scan-cache flush 時の期限切れ破棄 | record() insert のみで ghost エントリが単調増加する無上限成長を解消（history の MAX_HISTORY_ENTRIES 規約へ揃えた） |
| 04 | expand_tilde 統一 | explorer の 1 行版を config 正装版に寄せ、MANAGED_PATTERNS 全要素 "~/" 接頭辞を source pin で固定 |
| 02 | main() フロー抽出 | 634 行の main() を 9 個の run_<flow> へ抽出。flush を main 末尾 1 箇所に、External arm の dry-run 解決を 1 箇所に |
| 03 | 無人バッチ骨格共有化 | select → batch → 報告の骨格と SkipReason 描画を batch.rs に 1 所有者化（PBI 74 の rendering 呼び出し側決定は維持） |

追完（同日、ユーザー指示の個別 PBI）: `2026-11-20-05` — 直接依存の chrono を廃止し jiff へ置換
（history タイムスタンプの書式は byte 等価。chrono は trash の推移依存として lock に残存）。

台帳送り（再検討トリガー付き）: Cleaner trait の capability split（default method 11 超で再評価）、
tests/ fixture 重複の全数監査、`SCAN_CACHE.set` テストリセット脆弱点、--yes skip 汎用行への語彙追加。

## 大局的コード改善 round 10（2026-11-21、全 3 PBI 完了）

継続ラウンド（差分スコープ）。round 9 の台帳送りトリガーを再評価し 3 PBI を実装。
最終ゲート: workspace 1289 passed 0 failed、clippy 0、fmt clean、build ok。

| PBI | タイトル | 概要 |
|:---:|---------|------|
| 01 | BatchOptions 部分適用 | run_selected_batch と無人 3 経路の呼び出し面を named 化 + bounds debug_assert。run_batch 本体は round 7 契約（引数リスト変更 = stage 3 統合同時）により据え置き |
| 03 | Cleaner trait 裁定 | 12 メソッドのオーバーライド率実測に基づき split 不採用・4 消費者群分類枠を trait doc に記録（investigate） |
| 02 | --yes skip 語彙付与 | --yes の skip summary に needs an explicit selection を付与。header の重複理由は削除し理由の所有者を per-target 行に一本化 |

台帳送り（再検討トリガー付き）: run_batch 引数リストの全面移行（stage 3 統合 PR と同時）、
exit-path 列挙 pin（フロー 15 超で）、RFC3339 形状 helper 統合（同型 pin 3 箇所で）、
tests/ fixture 監査（explore 系タスク時）、SCAN_CACHE.set 脆弱点（scan_cache テスト基盤を触る機会）。

## autonomous-task-closer（2026-11-21、PBI 04・05 の自律実装）

pbi-backlog-prioritizer で起票した 2 PBI を worktree 隔離の並列クローザーで実装。
最終ゲート: workspace **1303 passed 0 failed**、clippy 0、fmt clean、build ok。

| PBI | タイトル | 概要 |
|:---:|---------|------|
| 04 | scan-cache テストリセットの決定化 | lock なし swap 設計（get_or_init → Mutex 内 swap）。境界トリガー（グローバルを触るテスト 2 本目で lock 再判断）を doc に記録。`--lib scan_cache` 連続 10 回 green |
| 05 | stage 3 統合 | report_single_target で表示 5 家族を 1 所有者化（BatchOptions 全面移行 + 小関数群移設）。round 7 契約文言を「統合機構 plus stage 3」に修正、#6 は意図的な非対称、stage-3 trap source pin 新設 |

## 未着手のバックログ

なし。2026-10-22 / 2026-10-23 の各一覧の PBI はすべて実装済みで
`.plan/archived/` にアーカイブしました。各一覧ファイルは索引として残っています。

| 一覧 | 内容 | 状態 |
|------|------|------|
| `2026-10-22-00-backlog.md` | 全体レビューで挙がった修正・改善 7 件 | ✅ 全件実装済み |
| `2026-10-23-00-backlog.md` | scan キャッシュ、グローバルフィルタ、ポリシーエンジン、プラグイン 8 件 | ✅ 全件実装済み |
| `2026-10-23-10-backlog-research-adoption.md` | FDA 診断、プロジェクト生成物、vm 表示、スナップショット間引き 4 件(リスク分類は 2026-10-22-04 に統合) | ✅ 実装済み・不採用項目は根拠付きで見送り |

## 設計方針（不変）

- **安全性優先**: `--dry-run` を全クリーナーに実装
- **Cleaner trait**: 全クリーナーが共通インターフェイスを実装
- **CommandRunner trait**: 外部コマンドをモック可能にしてテスト容易性を確保
- **Outside-In TDD**: E2E テストから開始し内側へ
- **macOS 専用**: Apple Silicon (arm64) + Intel (x86_64)
