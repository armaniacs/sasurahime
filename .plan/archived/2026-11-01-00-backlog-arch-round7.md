# バックログ索引: 持ち越し候補ラウンド 7（2026-11-01）

出典: ラウンド 6 の持ち越し候補 3 件（Option A 段 3 / TUI 見積もり tier 2 / KNOWN_ENTRIES 条件付き中間解）のなぜなぜ分析（3 立場 × 32-33 段 × 3 グループ = 連鎖合計 290 段超、全文は .plan/whywhy-round7-*.md 3 ファイル、gitignored）

## RICE 索引

| PBI | タイトル | R | I | C | E（人週） | スコア | 判定 |
|---|---|---|---|---|---|---|---|
| 101 | TUI の "Will free approximately" を path 単位の共有に寄せる（tier 2） | 2 | 1.5 | 80% | 0.42 | 5.7 | PBI 化（tier 1 着地後も cross-sibling 過小差し引き / Homebrew 誤差し引き / 未 net 二重計上の両方向実害が実在。hint 所有 23 path のうち 16 ≒ 70% が 2 sibling counted 集合と重複） |

direct commit rider（PBI 化不要、rationale worktree で実装済み、`e0b2588` で着地済み）:
- src/batch.rs 所有者表の行番号引用を round 6 の行ずれ後に実測値へ再整列（`main.rs:893-915` → `main.rs:894-986`、`main.rs:934-940` → `main.rs:937-953`）
- Option A 段 3 発火条件文にラウンド 7 再トリガー T1-T4 を 1 文追記

## 分析の裁定（PBI 化しない項目とその理由）

- **Option A 段 3（CO1）**: deferred 維持、PBI 化しない。round 6 の 6 PBI（95-100）はいずれも batch.rs の発火条件（行家族の追加 / gate の移動 / gate 条件の変更）に非該当（PBI 95 は graze したが dispatch gate 不変 + 既存契約の完成 + 所有者表更新ルール遵守の 3 点で非該当）。発火条件は PBI-90（round 4）と PBI 95（round 6）の 2 度正しく機能した。再トリガー T1-T4 を src/batch.rs の条件文に記録済み（`e0b2588`）。発火時（T2/T3）は A5 形態 enum 先行 → 5 筋置換 → run_clean_target → run_batch(1 要素) 置換 → 所有者表全面書き直しの順で、その場で PBI を書く（PBI 先置きは「PBI があるから発火した」誤読を生む）
- **KNOWN_ENTRIES 条件付き中間解（CO2）**: PBI 化なし、条件付き休止維持。中間解の段階（display_name 一意性 pin）は PBI 99 で着地済みで中間解自体が完成。残る選択肢（表統合・条件付き join 廃止）は round 6 chain が本格化時の判断として明示的に見送り済みで再裁定しない。需要の発火条件（GitHub Issue で hint 追加要望 3 件以上）は材料化していない（GitHub Issue の実数はリポジトリ読み取りのみの分析からは観測不可、最終確認は次に表を触る者が行う）

## TUI tier 2 の PBI 形状（PBI 101 に記載済みの要点）

- sibling 側 counted 述語 2 つを新設（library_caches::counts_owned、app_support::counts_owned、静的・pure。**正本は sibling 側** ― hint 側のみの判定は Homebrew を誤共有する）
- 新 trait メソッド `Cleaner::sub_owned_paths`（既定 vec![]、hints のみ実装。sub_targets 戻り値拡張は CLI に波及するため避ける）
- SelectionEntry 拡張（sub 行の所有 path 注記）+ build_selection_items 運搬。string re-match は PBI 59 逆行のため採らない
- compute_selected_total を per-sibling net に一般化（tier 1 pin 2 本の意味論は不変、fixture 拡張 + 期待値更新を伴う）
- 採らない裁定: 完全 path 和集合（ScanResult path 運搬）、brew × hints Homebrew / library-logs × hints Logs の未 net（射程外、将来 PBI）、threshold_override（CLI 専用、TUI は None 固定、残り窗口）

## 次ラウンドの持ち越し候補

1. Option A 段 3 + A5 形態 enum（T2/T3 発火時、その場で PBI）
2. KNOWN_ENTRIES 表統合（前置き「3 照合の意味論統一」の着手を先に、CO2 トリガー後）
3. brew × hints Homebrew / library-logs × hints Logs の未 net（sibling 集合拡張の将来 PBI、PBI 101 の射程外として分離）
4. PBI 101（TUI tier 2）の実装
