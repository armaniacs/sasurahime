# バックログ一覧: 2026-10-23 他製品調査の取り込み(新規4件+既存1件に統合)

調査文書「macOSシステムデータ肥大化の構造と消去メカニズム」(CleanMyMac / DaisyDisk / OnyX / mac-cleanup-py / mac-cleanup-go / Mole)を既存実装と突き合わせ、未実装の5点のうち4点を新規PBI化し、リスク分類は既存の `2026-10-22-04-feat-auto-policy` に統合した。

## 既存実装で対応済みの項目(PBI化しない)

| 調査の指摘 | 既存の対応 |
|-----------|-----------|
| 設定ファイル破損時の中断(Mole) | `config.rs` がパース失敗でエラー終了 |
| デフォルトのゴミ箱移動(mac-cleanup-go) | `src/trash.rs` と Trash モード |
| 操作の監査ログ(Mole) | 構造化ログ(アーカイブ済 PBI-05) |
| ドライラン(mac-cleanup-py) | `--dry-run` |
| Xcode DerivedData / simulator / Docker / Homebrew | `xcode.rs`、`generic.rs`、`brew.rs` |
| iOSバックアップ・APFSスナップショット個別削除 | `ios_backup.rs`、`apfs_snapshot.rs` |
| Library/Caches・Application Support | `library_caches.rs`、`app_support.rs` |

## 新規RICE採点表(降順=着手順)

| 順位 | 候補 | Reach | Impact | Conf | Effort | RICE | ファイル |
|-----:|------|------:|-------:|:----:|:---:|-----:|----------|
| 1 | FDA事前診断(doctor) | 400 | 2 | 80% | 1.5 | 427 | `pbi/2026-10-23-05-feat-fda-preflight.md` |
| 2 | プロジェクト生成物パージ | 600 | 3 | 70% | 4.0 | 315 | `pbi/2026-10-23-06-feat-project-artifacts.md` |
| 3 | スワップ/スリープ表示 | 300 | 1 | 80% | 1.0 | 240 | `pbi/2026-10-23-07-feat-vm-swap-report.md` |
| 4 | パージ可能領域とスナップショット間引き | 400 | 2 | 60% | 2.5 | 192 | `pbi/2026-10-23-09-feat-purgeable-thinning.md` |

リスク4段階分類(Safe / Moderate / Risky / Manual)は `pbi/2026-10-22-04-feat-auto-policy.md` に統合した(RICE 360、03の後)。2は `2026-10-23-01` の永続キャッシュ後にキャッシュキーへ載せる。

## 不採用とした項目

| 調査の提案 | 不採用の理由 |
|-----------|-------------|
| `dd` / `hdiutil` でディスクを満杯にしてパージを強制 | SSDへの大量書き込みと、満杯中の他プロセス障害のリスクが利益を上回る。OSの間引き(`tmutil thinlocalsnapshots`)で代替 |
| 言語パック(.lproj)の削除 | アプリのコード署名が壊れ、起動不能や更新失敗を招く |
| `~/Library/Caches` と `/private/var/log` の無差別削除、DNSフラッシュ | 既存の許可リスト方式と矛盾し、再構築コストが高い(調査5章の Cache Churn)。範囲外 |
| Spotlight / LaunchServices / dyld の再構築 | 効果が容量削減ではなく修復で、目的外 |

## 疑問の解決メモ
- Q: パージ可能領域は直接解放できるか → A: OSの遅延回収のため直接は不可 → 解: 表示と間引きで原因切り分けに留める(09)
- Q: 生成物の誤削除をどう防ぐか → A: マニフェスト必須+直近更新除外+`--yes` 対象外 → 解: 受け入れ基準に固定(06)

## 次の一手
`2026-10-23-05` から着手。`cargo test` グリーン確認後に Red→Green→Refactor。
