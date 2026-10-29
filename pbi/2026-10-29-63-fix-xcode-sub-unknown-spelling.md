# PBI: `xcode --sub` の不明な綴りをエラーにする

## ユーザーストーリー
CLI で `sasurahime clean xcode --sub <name>` を実行するユーザーとして、綴りを間違えたときに DerivedData だけが掃除されるのではなくエラーで知らせてほしい。なぜなら、現在は不明な綴りが `filter_map` で黙って捨てられ、サブカテゴリ指定が空になり DerivedData のみが掃除される（アーカイブが残る）うえ、ヘルプは「(default: all)」と虚偽を表示するから。

## 優先度
- 順位: 63 / 全 9 候補
- RICE スコア: 8.0（Reach=1 / Impact=1 / Confidence=80% / Effort=0.1 人週）
- 根拠: `sasurahime clean xcode --sub bogus` は main.rs:1212-1221 の `filter_map` で黙って捨てられ、`with_subcategories` が呼ばれず、XcodeCleaner::clean が既定 `vec![DerivedData]`（src/cleaners/xcode.rs:118-121）で動く。ユーザーは「アーカイブも掃除した」と思うが実際は DerivedData のみ。CLI ヘルプ（main.rs:241）は「(default: all)」と表示するが、`--sub` なしの既定は DerivedData のみ

## ビジネス価値
- 綴りミスで削除範囲が黙って狭くなるバグのクラスが消える
- ヘルプの虚偽表示が消える

## BDD 受け入れシナリオ
Scenario: 不明な綴りはエラーになる
  Given `XcodeSubcategory::parse` が "bogus" を受け付けない
  When  `sasurahime clean xcode --sub bogus` を実行する
  Then  エラーで終了し、期待される綴り（derived-data / archives）がメッセージに示される

Scenario: 有効な綴りは従来どおり動く
  Given `XcodeSubcategory::parse` が "archives" を受ける
  When  `sasurahime clean xcode --sub archives` を実行する
  Then  アーカイブのみが掃除される

Scenario: ヘルプが実際の既定を説明する
  Given CLI ヘルプのサブカテゴリ節
  When  `--help` を表示する
  Then  「(default: all)」の代わりに実際の既定（DerivedData）が示される

## 受け入れ基準
- [ ] `--sub` に不明な綴りを渡すとエラーになり、有効な綴り一覧がメッセージに示される
- [ ] 有効な綴り（derived-data / deriveddata / archives / DerivedData / Archives）は従来どおり動く
- [ ] CLI ヘルプの「(default: all)」が実際の既定を説明する
- [ ] 不明綴りのテストが 1 本追加されている
- [ ] 全テストが緑

## テスト戦略
- E2E: tests/xcode.rs に `--sub bogus` のエラー主張を追加
- 単体: XcodeSubcategory::parse の有効綴り一覧のテスト

## 実装アプローチ
- 実装順: (1) main.rs の `filter_map` を `map` + 不明綴りでエラーに変える（`xcode_subcategory` が None を返したら ok_or_else）、(2) ヘルプの文言修正、(3) E2E
- 判断の根拠: 黙って DerivedData にフォールバックする選択肢は採らない。削除範囲が黙って狭まるのはユーザーを欺く

## 見積もり
0.1 人週

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] CHANGELOG の [Unreleased] に追記
- [ ] コードレビュー完了
