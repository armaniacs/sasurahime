# PBI: safety.rs 抽出と generic.rs 分割（削除シームの逆依存解消）

## ユーザーストーリー
リポジトリの保守担当者として、削除の安全ポリシーがプラットフォーム層に住み、generic.rs を 2695 行読まなくてもシームを推論できる状態がほしい。なぜなら、キャッシュクリーナーの編集が削除の安全を黙って変え得る現状は、安全の推論コストが高すぎるから。

## 優先度
- 順位: 57 / 全 8 候補
- RICE スコア: 3.2（Reach=3 / Impact=2 / Confidence=80% / Effort=1.5 人週）
- 根拠: generic.rs（2695 行）に 6 責務があり、削除シーム（trash.rs:172）がクリーナー層の is_safe_delete_target を呼び戻す逆転がある（trash.rs:172、main.rs:1436、custom.rs:55、project_artifacts.rs:176）。resolve_env_dir（generic.rs:999-1029）はコンストラクタ内でプロセス環境変数を読む隠れた副作用
- 依存: 55（generic.rs を大きく触る）の後。56（delete_entries は trash.rs に置く）とも整合

## ビジネス価値
- 削除の安全ポリシーが 1 モジュールに集まり、シームの推論が generic.rs を読まずに済む
- キャッシュクリーナー編集と削除安全の変更が切り離される
- 逆依存（プラットフォーム層がクリーナー層を呼ぶ）が解消される

## BDD 受け入れシナリオ
Scenario: 削除シームがクリーナー層を呼ばない
  Given safety.rs に is_safe_delete_target が抽出されている
  When  trash.rs の safe_delete を読む
  Then  crate::cleaners::generic への依存がなく、safety.rs への依存になる

Scenario: キャッシュクリーナーの編集が安全に影響しない
  Given generic.rs の CliCleaner のコンストラクタを編集する
  When  safety.rs のテストを実行する
  Then  安全ポリシーのテストは generic.rs の変更に影響されず緑のまま

## 受け入れ基準
- [x] `src/safety.rs` に is_safe_delete_target と resolve_env_dir が抽出されている（trash.rs の下の層）
  - 実装: src/safety.rs（両関数 + 移設した 33 本の境界テスト）
- [x] trash.rs:172、main.rs:1436、custom.rs:55、project_artifacts.rs:176 が safety.rs を参照する
  - 実装: trash.rs / main.rs / custom.rs / project_artifacts.rs が crate::safety::is_safe_delete_target を参照。逆依存（プラットフォーム層がクリーナー層を呼ぶ）を解消
- [x] CliFallbackCleaner / clean_cli_or_fallback が src/cli_fallback.rs に抽出されている（huggingface.rs:10、pre_commit.rs:11 の参照先更新）
  - 実装: src/cli_fallback.rs + 2 テスト移設。huggingface.rs / pre_commit.rs の参照先を crate::cli_fallback に更新
- [x] CliCleaner と PathCleaner が各モジュールに分割されている、または分割せず責務の境界をコメントで明示する判断を記録している
  - 判断: 分割しない。対応テスト（constructor correspondence）が 2 型の境界を固定しており、分割は対応を離すだけ。generic.rs 冒頭に判断を記録
- [x] resolve_env_dir の env 読みが副作用として明示されている
  - 実装: doc コメントに "# Side effect" 節を追加（呼び出し時の env 読み、注入への変更は挙動変更として本 PBI 外）
- [x] 全テストが緑（挙動維持）
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）

## テスト戦略
- リファクタ: 既存テストが緑のまま（外部挙動の維持）
- 単体: safety.rs の is_safe_delete_target の境界（~、Trash、相対パス、シンボリックリンク）テストが新モジュールで緑
- 統合: huggingface/pre_commit の CliFallbackCleaner 参照先の更新を検証

## 実装アプローチ
- 抽出順: (1) safety.rs（is_safe_delete_target + resolve_env_dir）、(2) cli_fallback.rs、(3) CliCleaner/PathCleaner の分割の判断
- 判断の根拠: is_safe_delete_target と resolve_env_dir は「同じ理由で同時に変わる」（削除の安全）ため 1 モジュールに集める。CliCleaner と PathCleaner は責務が異なるため分割候補だが、correspondence テスト（generic.rs:1579-1599）が境界を固定しているので、分割しない判断も記録として妥当

## 見積もり
8

## 技術的考慮事項
- 32 個の名前付きコンストラクタの表化（uv/brew の define_cleaners! パターン）は本 PBI の範囲外（または 58 の後に別 PBI）
- resolve_env_dir の env 読みは、副作用の明示（doc コメント + 呼び出し側での注入検討）まで。注入への変更は挙動変更になるため本 PBI では行わない

## 実装者向け注記
現状確認:
```
grep -n "^impl \|^pub fn \|^fn \|^struct " src/cleaners/generic.rs
grep -rn "is_safe_delete_target" src/ --include="*.rs" | grep -v tests
grep -rn "resolve_env_dir" src/ --include="*.rs" | grep -v tests
grep -rn "CliFallbackCleaner\|clean_cli_or_fallback" src/ --include="*.rs" | grep -v tests
```
手順: 上記抽出順のとおり。git mv ではなく新ファイルへの移動 + 参照先更新。落とし穴: (1) trash.rs が generic.rs を参照する逆依存は savor.rs 抽出後にのみ解消する、(2) 55 のプロンプトポートが generic.rs を大きく触るため先に着地させる、(3) huggingface.rs:10 と pre_commit.rs:11 の import 更新を忘れない。

## Definition of Done
- [x] safety.rs と cli_fallback.rs が抽出されている
  - 検証: 両モジュールが lib.rs に登録され参照更新済み
- [x] 削除シームの逆依存が解消されている
  - 検証: trash.rs が crate::safety を参照（crate::cleaners::generic への参照なし）
- [x] 全テストが緑
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib（既知 TCC 4 件のみ）（外部挙動の維持）
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
