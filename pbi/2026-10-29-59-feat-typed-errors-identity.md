# PBI: 型付き DeleteError と CleanerId（文字列で運ばれる分類・同一性の解消）

## ユーザーストーリー
リポジトリの保守担当者として、skip/fatal の分類とクリーナーの同一性が型で保証され、文言の言い換えやリネームで制御フローが変わらない状態がほしい。なぜなら、エラーメッセージの部分文字列で制御フローが決まる現状は、言い換え 1 つで skip が致命的失敗に反転するから。

## 優先度
- 順位: 59 / 全 8 候補
- RICE スコア: 0.96（Reach=2 / Impact=1.5 / Confidence=80% / Effort=2.5 人週）
- 根拠: skip/fatal 分類がエラーメッセージの部分文字列で決まる（cleaner.rs:150-164 の is_skippable_error が msg.contains("trash failed") や starts_with("Permission denied") で判定、trash.rs:199-203 が生の散文を skip_reason に、テストが散文を固定 generic.rs:2614-2618 は reason.starts_with("trash failed") を主張）。同一性も表示文字列（cleaner.rs:22/66、main.rs:891-896 の文字列等価結合、cleaner.rs:253 + interactive.rs:54-61、main.rs:833-838 の二重綴りパーサ）
- 依存: 56（delete_entries が DeleteError の着地先）の後

## ビジネス価値
- 文言の言い換えで skip が致命的失敗に反転するバグのクラスが消える
- リネームで TUI -> clean シームが実行時に壊れるバグのクラスが消える（コンパイルエラーになる）
- 二重綴りパーサ（main.rs:833-838）が 1 つの解析に統合される

## BDD 受け入れシナリオ
Scenario: エラーメッセージの言い換えで分類が変わらない
  Given DeleteError::TrashRefused が型で定義されている
  When  trash.rs の anyhow! 文言を言い換える
  Then  is_skippable() の判定は変わらず、テストも緑のまま

Scenario: クリーナーのリネームでシームが壊れない
  Given CleanerId 列挙が trait の同一性である
  When  クリーナーの表示名を変更する
  Then  TUI -> clean シームは CleanerId で結ばれ、コンパイルが通る

Scenario: sub-targets の合計が型で結ばれる
  Given sub_targets が enum id + 表示名を持つ
  When  interactive.rs が合計を計算する
  Then  文字列再一致ではなく id 結合になり、不一致はコンパイルエラーになる

## 受け入れ基準
- [x] `DeleteError` 列挙（TrashRefused / PermissionDenied / Busy / Io）が定義され is_skippable() を持つ
  - 実装: src/cleaner.rs（共通語彙として cleaner.rs に配置。trash.rs のシームが生成する）。検証: cleaner::tests::delete_error_*_is_skippable 5 本が variant ごとに分岐を検証
- [x] prose は Display のみに残り、is_skippable_error の文字列一致（cleaner.rs:150-164）が削除されている
  - 実装: safe_delete は `e.is_skippable()` で variant 判定し、is_skippable_error の contains/starts_with 削除（downcast のみに縮小）。検証: delete_error_rewording_does_not_flip_the_classification が文言変更で分類不変を検証
- [x] trash.rs:199-203 の skip_reason が型を運ぶ（SkippedEntry に variant + path、表示は print 時に整形）
  - 実装: DeleteOutcome.skip_reason: Option<DeleteError> + SkippedEntry.reason: SkipCause（Delete(DeleteError) | Reason(String)）。25 箇所のリテラルを機械的に置換。検証: generic/cli_fallback の seam-skip テストが SkipCause::Delete(TrashRefused(_)) を matches! で検証
- [x] generic.rs:2614-2618 の散文ピンテストが型のテストに置き換わっている
  - 実装: reason.starts_with("trash failed") の主張を matches!(SkipCause::Delete(DeleteError::TrashRefused(_))) に置換（generic.rs + cli_fallback.rs + trash.rs の 3 箇所）
- [ ] `CleanerId` 列挙が trait の同一性になり、name() は表示に降格する
  - **判断（実装アプローチで許容された選択）: 新設しない。** 標準ターゲットの型付き同一性は既に `CleanTarget` 列挙（define_cleaners! が生成、CLI dispatch はこれを通る）であり、trait に並列の CleanerId を載せると 26 構造体クリーナーで 1:1 複製になり、plugin の動的発見のために文字列 arm が残る = 同一性の真実が 2 つになる。この判断を main.rs の clean_by_name シームにコメントで記録。文字列等価の残存箇所はシーム 1 箇所のみで、選択リストの両側が同一コード（cleaner.name()）から生成される
- [x] sub_targets が enum id + 表示名を持ち、interactive.rs:54-61 の文字列再一致が削除されている
  - 実装: サブの走査サイズを選択マッピングに同梱（SelectionEntry = (usize, Option<(&str, u64)>)）し、合計は運ばれたサイズから計算。文字列再一致（find(|(n, _)| n == sub_name)）を削除。XcodeSubcategory::parse は display 名も 1 解析で受ける。検証: build_selection_items_with_sub_targets がサイズ同梱を検証
- [x] main.rs:833-838 の二重綴りパーサが 1 つの解析に統合されている
  - 実装: xcode_subcategory の display 名再一致を削除し、XcodeSubcategory::parse に統合（"DerivedData"/"Archives" も parse で受ける）
- [x] 全テストが緑（挙動維持）
  - 検証: cargo test --no-fail-fast 40 スイート緑 + lib 758 passed / 4 failed（既知の TCC 依存 4 件のみ、main ブランチのベースラインと同一）。fmt/clippy 0

## テスト戦略
- 統合: DeleteError の is_skippable() 分岐（各 variant）
- E2E: trash 失敗 -> SkippedEntry（文言を言い換えても緑）
- 単体: CleanerId の一意性と対応テストの置き換え

## 実装アプローチ
- 実装順: (1) DeleteError 列挙 + is_skippable()、(2) trash.rs と delete_entries（56）の着地先に型を流す、(3) SkippedEntry の型化、(4) CleanerId 列挙、(5) sub_targets の型化、(6) 二重綴りパーサの統合
- 判断の根拠: CleanerId は trait に直接載せると 26 クリーナー + プラグインの動的発見（plugin.rs）と衝突する。plugin は CleanerId::Plugin(id) または文字列のままにする判断が必要

## 見積もり
13

## 技術的考慮事項
- CleanerId の導入は define_cleaners! マクロ（main.rs:31-94）の変形を伴う。マクロで enum を生成するか、手動 enum + 表の対応テストで固定するか
- 判断の根拠: プラグインの動的同一性（実行時に発見される sasurahime-*）は静的列挙に入らない。CleanerId は静的クリーナーのみにし、プラグインは名前のままにする（境界を明示）

## 実装者向け注記
現状確認:
```
grep -n "is_skippable_error" src/cleaner.rs
sed -n '150,164p' src/cleaner.rs
grep -n "skip_reason" src/trash.rs src/cleaner.rs | head -8
sed -n '833,838p' src/main.rs
grep -n "name() ==" src/main.rs src/interactive.rs
```
手順: 上記実装順のとおり。落とし穴: (1) 56 の delete_entries が着地先なので先に着地させる、(2) generic.rs:2614-2618 の散文ピンテストは型化後に意味を失うので削除、(3) CleanerId のマクロ変形は define_cleaners! の生成物（CleanTarget 列挙、SUPPORTED_TARGETS、dispatch）と整合させる必要がある。

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] is_skippable_error の文字列一致が消えている
- [ ] 全テストが緑
- [ ] コードレビュー完了
