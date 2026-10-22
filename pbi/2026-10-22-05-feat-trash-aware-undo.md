# PBI: Trashを考慮した解放表示とundo導線

## ユーザーストーリー
macOS開発者として、消した後に本当に空いたのか・元に戻せるのかが分かってほしい、なぜなら `Freed: 0 B (moved to Trash)` と `df` が減らないギャップで不信になるから

## ビジネス価値
削除後の納得感が上がり、「44GB消したのに減ってない」系の混乱・ issue が減る。ゴミ箱起因のディスク圧迫事故も防げる。

## 優先度
- 順位: 05 / 7
- RICEスコア: 672（Reach=800 / Impact=3 / Confidence=70% / Effort=2.5人週）
- 根拠: Reach最大級。01の `Unavailable` 分離後に論理/物理の2軸表示を入れるため01の後。

## BDD受け入れシナリオ
Scenario: 論理と物理が分かれて表示される
  Given Trashモードで1GB削除できる状態がある
  When `clean uv` を実行する
  Then `Logical freed: 1.0 GB / Physical freed: 0 B (in Trash)` のように2軸で表示される

Scenario: 復元導線が示される
  Given 削除直後の状態がある
  When `stats` または削除結果を見る
  Then Finderでの復元手順または `open ~/.Trash` 導線が表示される

## 受け入れ基準
- [ ] `uses_trash` に基づく2軸表示（論理/物理）が全clean経路で統一される
- [ ] `clean trash` がサイズとFinderで空にする手順を示す（安全な空化はしない）
- [ ] `history.json` に trash/物理の区別が残り `stats` で確認できる
- [ ] `--permanent` 時の監査証跡が `--verbose` なしでも最低限残る

## テスト戦略
- E2E: Trashモード/`--permanent` の両方で表示文言のゴールデンテスト（本物Trash依存は `--permanent` + モックに限定）
- 統合: `freed_message` の分岐テストを論理/物理に拡張
- 単体: Byte整形・分岐条件の境界値テスト

## 見積もり
5 SP（要チームでの見積もり）

## 実装アプローチ
1. `main.rs:539` の `freed_message` を論理/物理に拡張
2. `CleanResult` に `uses_trash` 済み情報を活かし history に種別保存
3. `stats` 表示に復元導線を追加

## 技術的考慮事項
- `trash` クレートの失敗時は `is_skippable_error` と連動させ二重表示にしない
- 大容量Trash移動前の事前警告（空き容量不足）を検討だが本PBIでは表示に留める

## 実装者向け注記
- 現状確認: `grep -rn "freed_message\|uses_trash\|moved to Trash" src/ | head -n 20`
- 着手前に `cargo test` でグリーン確認。E2Eは本物Trashを使わず `--permanent` 固定の流儀を守る。

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（README Safety/Trash 節を更新）
