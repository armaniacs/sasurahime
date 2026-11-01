# バックログ: TTY 対話ウォークスルー（2026-11-01、/pbi-backlog-prioritizer）

出典: TTY 対話ウォークスルーgap の実測（main @ ddc6395、39/41 integration ファイルが実バイナリ spawn の Outside-In E2E を持ち、対話ループ本体だけが歩いていない）+ なぜなぜ分析 round 8（.plan/whywhy-round8-freed-drift.md）の「select 経路の pin は不可、ヘルパー直呼びが唯一」記録。

## 順位表

| 順位 | 候補 | RICEスコア | 根拠 | 依存関係 |
|---|---|---|---|---|
| 1 | [PBI-102 feat 選択 port のテスト注入 seam](2026-11-01-102-feat-select-port-test-seam.md) | ≒ 8.5（Reach 2 × Impact 1.5 × Confidence 85% / Effort 0.3 人週） | `PromptPort` trait・2 アダプタ・7 呼び出し元が全て実在（PBI 75）で欠けているのは注入 seam のみ。対話→削除→freed の E2E を依存追加なし・flaky なしで届ける。round 8 の C2（select 経路の freed drift）がまさにこの筋の実例 | **PBI-103 と代替**。本 PBI 着地で PBI-103 の残存価値は dialoguer 自体の描画に縮小 |
| 2 | [PBI-103 backlog 疑似 TTY ハーネス](2026-11-01-103-backlog-pseudo-tty-harness.md) | 1.0（Reach 2 × Impact 1.5 × Confidence 50% / Effort 1.5 人週） | dialoguer 自体の描画とキー操作まで歩けるが、flaky リスクが構造的（raw mode + winsize + pty 実装の環境差）で依存追加を伴う。**PBI-102 着地後に再判定**（残存価値があれば縮小スコープで、なければ close） | PBI-102 に依存（着地後の再判定が着手条件） |

## 判定の要点

- 2 候補は同じ価値帯（対話フローの E2E 化）の**代替案**で、並行着手はしない。順位 1（PBI-102）を先に着地させ、その結果で順位 2 の価値を再判定する
- PBI-102 は既実装確認（フェーズ 0）で「port の新設」から「注入 seam の新設」に修正済み（`PromptPort` は PBI 75 で実在、`for_current_run` の ambient 解決だけがテストから差し替え不能）
- PBI-103 はスパイク分離（flaky レートの計測）を着手条件に含める
