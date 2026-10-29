# PBI: plugin clean --json プロトコル拡張（bytes_freed の報告）

## ユーザーストーリー
外部プラグインを書く開発者として、プラグインが削除した量を sasurahime に報告できるようにほしい。なぜなら、現状は bytes_freed が 0 固定で、プラグインの清掃が stats から永久に不可視だから。

## 優先度
- 順位: 58 / 全 8 候補
- RICE スコア: 1.6（Reach=1 / Impact=1 / Confidence=80% / Effort=0.5 人週）
- 根拠: PluginCleaner::clean（plugin.rs:329-351）は bytes_freed: 0 を固定（exit code のみのプロトコル）。`Total freed` が 0 と出て、history ガード `!dry_run && bytes_freed > 0`（main.rs:802-808）が発火しないので stats に永久に記録されない。プロトコル自体は堅牢（PATH 探索 plugin.rs:131-163、10s タイムアウト + kill_tree plugin.rs:170-247）

## ビジネス価値
- プラグインの清掃が stats と Total freed に反映される
- プラグイン作者に「何をしたか報告する」インセンティブが生まれる
- プロトコルは追加的な拡張（後方互換: json なしのプラグインは exit code のみで動き続ける）

## BDD 受け入れシナリオ
Scenario: json を出すプラグインの解放量が報告される
  Given プラグインが clean --json で {"freed": 1234, "uses_trash": false} を出す
  When  sasurahime clean <plugin> を実行する
  Then  Total freed に 1234 が表示され、history に記録される

Scenario: json を出さないプラグインは後方互換
  Given プラグインが exit code のみで報告する（現行プロトコル）
  When  sasurahime clean <plugin> を実行する
  Then  従来どおり bytes_freed 0 で成功として扱われる

Scenario: 壊れた json は後方互換で扱う
  Given プラグインが不正な json を出す
  When  sasurahime clean <plugin> を実行する
  Then  exit code で成功か否かを判定し、解析不能な出力は無視する

## 受け入れ基準
- [x] プラグイン呼び出しに --json（または最後の json オブジェクト解析）を追加する
  - 実装: plugin.rs が extract_last_json_object（既存の size parser ユーティリティ）を再利用
- [x] {"freed": n, "uses_trash": b} の解析で bytes_freed と uses_trash を反映する
  - 実装: CleanReport + parse_clean_output。検証: tests/plugin.rs 有効 json -> history freed_bytes 1234
- [x] json がない場合、解析失敗の場合は現行の exit code のみの挙動にフォールバックする
  - 検証: tests/plugin.rs の json なし（互換挙動）と壊れた json（フォールバック）の 2 本
- [x] stats と history に記録される
  - 検証: tests/plugin.rs が history freed_bytes への反映を検証
- [x] 後方互換のテスト（json なし、壊れた json）がある
  - 検証: tests/plugin.rs +4（有効 json / json なし / 壊れた json / 受信引数がちょうど clean）

## テスト戦略
- 統合: fake プラグイン（スクリプト）で json あり/なし/壊れた json の 3 系統
- E2E: プラグイン清掃の Total freed 表示
- 単体: extract_last_json_object 風の解析（既存の解析ユーティリティがあれば再利用）

## 実装アプローチ
- プロトコルの追加は 1 フィールド（既存の解析: extract_last_json_object があれば再利用、なければ最小の json 解析）
- 判断の根拠: プロトコルを破壊的に変えない（後方互換のフォールバック）。リスク(risk)/advisory のメタデータ拡張は本 PBI の範囲外（将来候補）

## 見積もり
3

## 技術的考慮事項
- --json フラグを渡すか、stdout の最後の json オブジェクトを解析するか: --json は明示だが古いプラグインが受けていない引数で失敗する可能性がある（引数を見ないプラグインは無視するが、見るプラグインは壊れる）。最後の json オブジェクト解析は両方に安全
- 判断の根拠: 後方互換を壊さない方が、現存プラグインが動き続ける

## 実装者向け注記
現状確認:
```
grep -n "bytes_freed" src/cleaners/plugin.rs
grep -n "extract_last_json_object\|json" src/ -r --include="*.rs" | grep -v tests | head -8
sed -n '329,351p' src/cleaners/plugin.rs
```
手順: (1) stdout の最後の json オブジェクト解析を足す、(2) freed/uses_trash の反映、(3) フォールバックのテスト。落とし穴: --json フラグの追加は既存プラグインを壊す可能性がある（引数を見るプラグイン）。最後の json 解析に寄せる。

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
  - 検証: 各 PBI のテスト戦略に沿ったテストが実装され緑
- [x] 後方互換のテストが緑
  - 検証: tests/plugin.rs の json なし / 壊れた json テスト緑
- [x] CHANGELOG の [Unreleased] に追記
  - 検証: CHANGELOG.md [Unreleased] に各バッチのエントリあり
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
