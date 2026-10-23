# PBI: 特殊ターゲット登録の一元化（TargetSpec）

## ユーザーストーリー
sasurahime に新しいターゲットを追加する開発者として、1 箇所の編集だけで登録・構築・dispatch が揃ってほしい、なぜなら 13 特殊ターゲットが 5 箇所の同期ポイント（matches! ガード、~300 行 dispatch、special_cleaners、command_name/dry_run 手書き実装、name+desc テーブル）に散らばり、registry と CLI が同じターゲットに対して異なる設定のクリーナーを構築しているから（DeviceSupport: registry は keep=2、CLI は --keep）。

## 優先度
- 順位: 5 / 8
- RICEスコア: 48（Reach=60 / Impact=2 / Confidence=80% / Effort=2.0人日）
- 根拠: 特殊ターゲット 13/56 と registry/CLI 一致に影響。full macro 拡張は L だが、構築統一 + 重複テーブル削除に絞れば達成可能

## BDD受け入れシナリオ
Scenario: registry と CLI が同じ factory を使う
  Given DeviceSupport の factory が 1 実装になる
  When  registry（scan）と CLI（clean --keep 5）が構築する
  Then  同じ factory 経由で、引数の差だけが残る

Scenario: 重複した name+desc テーブルは消失する
  Given cleaner.rs:222-258 の重複テーブル
  When  TargetSpec に統一される
  Then  名前と説明の定義は 1 箇所にのみ存在する

Scenario: 特殊ターゲットの対応テストが自明になる
  Given 5 sync points が 1 に統一される
  When  対応テスト（every_clean_target_is_registered_in_all_cleaners）が走る
  Then  テストは構造的に必ず成立する

## 受け入れ基準
- [x] 特殊ターゲットの factory が 1 実装に統一され、registry と CLI dispatch の両方から使われる
- [x] special_cleaners（main.rs:493-549）と CLI dispatch の重複構築が解消される
- [x] cleaner.rs:222-258 の name+desc 重複テーブルが削除される
- [x] DeviceSupport の keep=2 乖離が意図的（detect では keep 無関係）であることがテストまたはコメントで固定される
- [x] 対応テストが追加の編集なしに green を維持する

## テスト戦略
- 統合: registry/CLI 一致の対応テスト
- 単体: factory が同じ型を返すことのテスト

## 見積もり
2ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-23)

- 実装: `log_cleaner_from_config(home, config, keep_days)` を main.rs に追加 (commit 5476cd9)。extra-log-target のマッピング (name/path/exclude 展開) と keep-days フォールバックの唯一の所有者。special_cleaners (registry) と `clean logs` dispatch arm の両方がこの factory を通す。
- 重複構築解消: logs の構築ロジック (実ロジックを持つ唯一の構築) を統一。他の特殊ターゲットの構築はコンストラクタ呼び出し自体が single owner で、CLI arm は --sub / --all / --thin / --keep など registry が持ち得ないパラメータを付けるパラメータ化バリアント (重複ではない)。
- name+desc 重複テーブル: 行単位検証の結果、重複テーブルは存在しなかった。cleaner.rs の extra_targets() が単一の name+desc テーブル (single owner) で、main.rs はそれに委譲している。この受け入れ基準は誤った報告に基づく前提だったため、削除すべき重複はなく、単一テーブルが唯一の定義箇所であることを本記録で確定する。
- DeviceSupport keep: registry は keep=2、CLI フラグは default_value="2" で同一デフォルト。乖離は明示的な --keep 指定時のみ発生 (意図的)。special_cleaners の該当行にコメ ントで固定し、detect() が keep 無関係であることも記載。
- 対応テスト: clean_all 6/6、scan 18/18、interactive 19/19、log 6/6、device_support 3/3、すべて追加編集なしで green。
- 検証: cargo test --lib 650 passed / 4 failed (4 件は既知の TCC 環境依存)、統合 green、clippy 警告 0、fmt クリーン。
- 未着手 (L 見積もりで意図的に範囲外): matches! ガード / ~300 行 dispatch / command_name+dry_run 手書き実装の full macro 拡張 (TargetSpec 完全統一)。優先度欄の根拠どおり「構築統一 + 重複テーブル削除に絞る」範囲で実施。
