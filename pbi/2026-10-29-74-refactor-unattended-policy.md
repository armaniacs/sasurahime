# PBI: UnattendedPolicy による無人実行選択の唯一の所有者

## ユーザーストーリー
sasurahime の保守担当者として、「どの target が無人実行されるか」の決定が名前のある 1 つの module に集中することがほしい、なぜなら 3 つの entry point で意図的に異なるポリシーが名前も型もなく手書きされ、誤統合が安全上の事故につながるから。

## 優先度
- 順位: 6 / 8
- RICE スコア: 1.2（Reach=2 / Impact=1.5 / Confidence=80% / Effort=2.0 人週）
- 根拠: 安全最重要の決定（Risky は auto では絶対に走らない）が 1 箇所で検証可能になる。PBI 73（レポート統合）が同じ main.rs arm を先に整理してから着手。

## 発見の詳細（深掘り結果）
- 3 つのフィルタは意図的に異なる（コメントと実装は一致）: auto は Risky/Manual を include_manual でも除外（auto_policy.rs:195-227 で pin）、all は include_manual があれば Risky も実行（main.rs:1354-1356 のコメント）、yes は auto_clean_allowed のみ。
- 差は 6 軸: gate の有無、include_manual の意味、scan 駆動 vs registry 駆動（yes だけ detect → Pruneable フィルタ）、フィルタ源、permanent 時 confirm、通知の文言と出力先。
- 誤統合の抑止は二重構え: (i) Yes だけが results を運ぶ（Auto/All に scan 前提の実装を書けない）(ii) matrix unit test で差分を pin。
- Rendering は select に含めない: 文言と出力先は E2E pin 済みなので call site に SkipReason を render するだけ。
- gate_skipped_names は UnattendedPolicy に入れない（TTY ランタイム事実は別軸、PBI 75 と非結合）。
- clean caches はフィルタなしで Moderate 7 種を無条件実行 = UnattendedPolicy の scope 境界に明記。

## BDD受け入れシナリオ
Scenario: Risky は auto で決して走らない
  Given [policy] include_manual = true の config
  When  `auto` を実行する
  Then  Risky / Manual target は skipped に分類される（NeverUnattended / NeedsOptIn）

Scenario: all は include_manual で Risky を解放する
  Given `clean all --include-manual`
  When  実行する
  Then  Risky target も run に入る（現行意図どおり）

Scenario: yes は scan 駆動で Unknown を除外する
  Given `--yes` と detect できない CLI cleaner
  When  実行する
  Then  Unknown 状態の target は run に入らない

## 受け入れ基準
- [ ] UnattendedPolicy { Auto, All, Yes { results } } + select(cleaners, opts) -> UnattendedSelection が選択の唯一の所有者
- [ ] SkipReason { NeverUnattended(Risk), NeedsOptIn(Risk), NeedsExplicitSelection(Risk) } で skip 理由を型で運ぶ
- [ ] auto の policy.present gate は arm に残留（entry gate は selection と別）
- [ ] matrix unit test（3 variant x 4 Risk x include_manual x Pruneable/Unknown）を追加
- [ ] 文言と出力先が不変（"never runs unattended" / "set [policy] include_manual = true" / "needs --include-manual" / "that need an explicit selection" が現行位置で生存、E2E 無修正で green）
- [ ] gate_skipped_names は run_auto に残留（PBI 75 と非結合）
- [ ] include_manual の 2 意味（All = 全 risk 解放 / Auto = Moderate 解放のみ）を module doc で明記
- [ ] clean caches が UnattendedPolicy の scope 外であることを doc で明記
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
- 単体: matrix unit test（新規、名前を与えたことで初めて unit-testable になる）
- E2E: 既存 auto_policy.rs / clean_all.rs / interactive.rs が無修正で green

## 見積もり
2.0 人週

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（module doc + CHANGELOG）
