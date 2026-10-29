# Backlog: v0.4.3 レビューからの改善候補一覧（2026-10-29）

## 出典
v0.4.3 の 4 観点コードレビュー（correctness/security、performance、maintainability/tests、CHANGELOG 主張の照合）の指摘のうち、即時修正しなかった項目を PBI 化した。
即時修正済み: 共有ループ契約の固定（92c40ae）、brew 二重走査の遅延化（92c40ae）、cargo/brew の失敗経路テスト（ba759b8）、CHANGELOG 誇張の軟化（ee084ea）。

## RICE 採点と最終順位

| 順位(NN) | 候補 | type | Reach | Impact | Conf | Effort(人週) | スコア | 出典 |
|---|---|---|---|---|---|---|---|---|
| 60 | ollama `--yes` の skip 通知 | fix | 2 | 1 | 80% | 0.1 | 16.0 | review 指摘 3 |
| 61 | plugin JSON 抽出の文字列対応 | fix | 1 | 3 | 100% | 0.25 | 12.0 | review 指摘 1 |
| 62 | 死んだ削除 API 整理（delete_path 削除 + is_skippable_error 統合） | fix | 1 | 1.5 | 100% | 0.1 | 15.0 | review 指摘 5+6 統合 |
| 63 | `xcode --sub` 不明綴りをエラーに | fix | 1 | 1 | 80% | 0.1 | 8.0 | review 指摘 7 |
| 64 | プロンプト suspend 契約の統一 | fix | 2 | 2 | 80% | 1.0 | 3.2 | review 指摘 4 |
| 65 | uv/hint/apfs の非ゼロ終了テスト | fix | 1 | 1 | 80% | 0.25 | 3.2 | review 指摘 8 |
| 66 | plugin 申告値の cap | fix | 1 | 1 | 80% | 0.25 | 3.2 | review 指摘 2 |
| 67 | io エラー kind コメント修正 | fix | 1 | 0.25 | 100% | 0.05 | 5.0 | review 指摘 10 |
| 68 | app_support の progress 総計 pin | fix | 1 | 0.25 | 80% | 0.1 | 2.0 | review 指摘 9 |

## 依存による順序調整
- 依存なし。全 9 候補が独立（触る情報源・出力ファイルが重ならない）
- バッチ計画: A（60+61+62+63 並列、非重複）→ B（64 単独、1.0 人週の最大物）→ C（65+66+67+68 並列）

## 同点の決め手
- 64/65/66（3.2 同点）: リスク軽減効果の順 (再描画バグ再現（64）> 回帰保護（65）> 信頼境界（66）)
- 67（5.0）が 64-66（3.2）より高いがファイル位置が散在するため NN の割当は採点順に従わず提示順を維持（ユーザー確認済み）

## 判断が必要な項目
- 60（ollama `--yes`）: skip 挙動の維持 + 通知追加が本 PBI の方向。ollama を実際に無人クリーニングするかは別判断（PBI 52 修正の意図的挙動）

---

# Backlog: アーキテクチャ深掘り（2026-10-29 第二陣）

## 出典
/codebase-design 語彙による探索ウォークで特定した 6 候補（削除帳簿 ledger、PromptPort gating、unattended ポリシー、TrashMode センチネル、CleanResult 構成、apfs mode）を 6 体の並列サブエージェントで深掘り（隠れた仮定・未発見リスク・未検討の意思決定）し、深掘り結果に基づき scope を修正して PBI 化した。
主な深掘り発見: colima の非 TTY 無確認全削除（安全床の不整合）、ios_backup が skipped を DROP、clean_with_opts のデフォルト実装が opts を無視（トレイト統合が必須）、`--all --thin` の黙って無視。候補 6 は Speculative から格上げ、候補 3 は PBI-A/B/C に分割。

## RICE 採点と最終順位

| 順位(NN) | 候補 | type | Reach | Impact | Conf | Effort(人週) | スコア | 出典 |
|---|---|---|---|---|---|---|---|---|
| 69 | colima 非 TTY 安全床 + --yes --permanent 終了コード | fix | 3 | 3 | 100% | 0.75 | 12.0 | 候補 3 深掘りの PBI-C |
| 70 | 削除帳簿 ledger 一般化（迂回 9 ファイル回収） | refactor | 3 | 2.5 | 80% | 3.0 | 2.0 | 候補 1 |
| 71 | TrashMode concrete 化 + Cleaner::clean トレイト統合 | refactor | 3 | 1.5 | 80% | 1.75 | 2.06 | 候補 4 |
| 72 | CleanResult dead field 削除 + constructor 収束 | refactor | 3 | 1 | 80% | 0.75 | 3.2 | 候補 5 |
| 73 | バッチ skip レポートの既存関数採用統合 | refactor | 2 | 1 | 80% | 0.75 | 2.13 | 候補 3 深掘りの PBI-A |
| 74 | UnattendedPolicy による無人実行選択の所有者 | refactor | 2 | 1.5 | 80% | 2.0 | 1.2 | 候補 3 深掘りの PBI-B |
| 75 | PromptPort が非 TTY を自身の状態として解決 | refactor | 3 | 1.5 | 80% | 1.5 | 2.4 | 候補 2 |
| 76 | apfs-snapshot mode 型明示化 + --all/--thin エラー化 | fix | 1 | 1 | 80% | 0.6 | 1.33 | 候補 6 |

## 依存による順序調整
- 70 → 71 → 72: ledger（70）が迂回 9 ファイルの共通 call site を先に消し、TrashMode（71）と CleanResult（72）の churn を最小化。72 の 21 箇所 data-carrying literal は 70 に吸収させる
- 73 → 74: 両方とも main.rs の同一 arm を触る。74 は 73 の整理後の arm 上に組む
- 69 → 75: generic.rs:557-558（colima）は PromptPort の seam 上にあるため安全側の意味決定（69）を先に。75 は 69 後の colima 意味を pin
- スコア上位の 75（2.4）/ 72（3.2）を後ろに倒したのはファイル重複による churn 回避

## バッチ計画（実装順）
- A（並列）: 69（generic.rs + interactive.rs）+ 76（apfs_snapshot.rs + main.rs dispatch arm）
- B（単独）: 70（trash.rs + 迂回 9 ファイル + ios_backup）最大物
- C（単独）: 71（TrashMode、~130 サイト最大面）
- D（並列）: 72（cleaner.rs + empty 変換）+ 73（main.rs 3 arm のみ）は非重複
- E（単独）: 74（UnattendedPolicy、main.rs arm + 新 module）
- F（単独）: 75（prompt.rs + 10 cleaner の gate）

## 判断が必要な項目
- なし（深掘りで colima の安全床、トレイト統合の必須性、apfs の scope 修正まで確定済み）
