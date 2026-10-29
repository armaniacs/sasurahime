# PBI: レビュー Quick Win の minor bundle（テスト衛生 + ドキュメント + 死んだメッセージの配線）

## ユーザーストーリー
リポジトリの貢献者として、ドキュメントが実際の仕組みを指し、テストのボイラープレートが最小で、死んだコードがない状態がほしい。なぜなら、存在しないシンボルを探す時間とロック忘れによるフレークは保守コストだから。

## 優先度
- 順位: 54 / 全 8 候補
- RICE スコア: 12.0（Reach=3 / Impact=1 / Confidence=100% / Effort=0.25 人週）
- 根拠: 4 件の Quick Win（低難易度 x 実価値）を 1 個に束ねた minor bundle。リポジトリの前例（PBI 47 minor bundle）に倣う。個々は小さいが、TEST_LOCK の 1 行変更は約 41 テストの前文を削除しロック忘れバグのクラスを消す

## ビジネス価値
- docs/HOWTO-ADD-target.md の stale 節（存在しない AUTO_CLEAN_TARGETS を参照、実際は risk() -> auto_clean_allowed() -> 自動 ManualOnly）がなくなり、貢献者が実在しないシンボルを探さない
- CleanTarget::Caches 臂が 7 行を手動複製している（main.rs:1136-1163）ことを membership テストが固定し、表と `clean caches` の静かな乖離を防ぐ
- colima の after_clean_message が clean 経路で死んでいる（generic.rs:516-536 のみで参照、Some(detect_dir) 分岐 :545-723 は未チェック）問題が解消される
- TrashModeGuard が TEST_LOCK を内部取得し、約 41 テストの 3 行前文が消える

## BDD 受け入れシナリオ
Scenario: 新規キャッシュターゲットが clean caches に含まれる
  Given 表に新しいキャッシュ系ターゲットを 1 行追加した
  When  membership テストを実行する
  Then  clean caches の臂が表の行と一致していない場合にテストが失敗する

Scenario: colima の clean 後メッセージが出る
  Given colima がインストール済みで ~/.colima が存在する
  When  clean colima が完了する
  Then  ディスクイメージの縮小に関する after_clean_message が表示される（または削除済みで出ない）

Scenario: TrashModeGuard のテストがロックを自前で取らない
  Given 新しい trash-mode テストを書く
  When  TrashModeGuard::set を使う
  Then  TEST_LOCK の 3 行前文を書かずに済み、並列テストでフレークしない

## 受け入れ基準
- [x] docs/HOWTO-ADD-target.md:110 と :226 の stale 節が risk() -> auto_clean_allowed() の実際の仕組みを説明する
  - 実装: risk() と auto_clean_allowed() の対応を説明する節に書き直し
- [x] CleanTarget::Caches 臂の membership テストが表の 7 行を反復して検証する
  - 検証: tests/dry_run.rs の membership テストが表の 7 行を反復
- [x] colima の after_clean_message が clean 経路で表示される、または意図的に削除されている
  - 実装: after_clean_message を detect_dir ありの成功経路に接線（WHY コメント付き、失敗時は表示しない）
- [x] TrashModeGuard::set が TEST_LOCK を内部取得し、約 41 テストの前文が削除されている
  - 実装: test_helpers.rs の TrashModeGuard が TEST_LOCK を内部取得。41 preambles 削除
- [x] 全テストが緑
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）

## テスト戦略
- 統合: Caches 臂の membership テスト（表の行を反復）
- 単体: colima の after_clean_message 表示（dry-run でない場合）
- リファクタ: TrashModeGuard 変更後の全テストが緑（既存テストの前文削除）

## 実装アプローチ
- TrashModeGuard::set（test_helpers.rs:121-137）が TEST_LOCK を VerboseGuard（test_helpers.rs:175-206）と同様に内部取得する
- after_clean_message は Some(detect_dir) 分岐（generic.rs:545-723）でも参照する。ただし表示条件を検討する（detect_dir が縮小した場合のみ、など）。表示しない判断なら削除して generic.rs のコメントに根拠を残す
- membership テストは SUPPORTED_TARGETS と Caches 臂の一致を反復検証

## 見積もり
2

## 技術的考慮事項
- after_clean_message の配線と削除のどちらにするかは、表示が clean の出力を濁さないかで判断する（colima は Freed: 表示と重なる可能性）。判断を generic.rs に Why コメントで残す
- TrashModeGuard の内部取得は Drop 順序に影響しないことを確認する（Guard がスコープで出るときロックも解放される）

## 実装者向け注記
現状確認:
```
grep -rn "AUTO_CLEAN_TARGETS" docs/ src/          # docs のみにヒットするのが現状
grep -n "after_clean_message" src/cleaners/generic.rs
grep -rn "TEST_LOCK.lock()" tests/ | wc -l        # 約 72 件
grep -n "TrashModeGuard" src/test_helpers.rs tests/*.rs | head -8
sed -n '1136,1163p' src/main.rs                   # Caches 臂の 7 行手動複製
```
手順: (1) TrashModeGuard 変更 + 前文削除、(2) HOWTO 修正、(3) membership テスト、(4) after_clean_message の判断と配線または削除。落とし穴: 前文削除は機械的だが、TrashModeGuard を使わず TEST_LOCK を直接取るテスト（約 31 件）は残る。そちらはそのまま。

## Definition of Done
- [x] 全テストが緑（前文削除後も）
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）
- [x] membership テストが緑
  - 検証: tests/dry_run.rs の membership テスト緑
- [x] HOWTO の stale 節が消えている
  - 検証: docs/HOWTO-ADD-target.md の risk()/auto_clean_allowed() 節を書き直し
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
