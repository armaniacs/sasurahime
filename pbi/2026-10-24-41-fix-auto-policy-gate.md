# PBI-41: auto の no-policy ガードを有効化（[policy] セクション実在時のみ許可）

## ユーザーストーリー

既存の config.toml（[policy] セクションなし）を持つユーザーとして、`sasurahime auto` が自分の意図しない無人削除を実行しないようにしてほしい。なぜなら、doc は「セクション不在なら auto は拒否する」と約束しているのに、現状はどんな既存ファイルでも年齢フィルタなしの無人削除が走るから。

## 発見元

1023a 全体レビュー Major #7（config.rs:157-162, main.rs:864）。

## BDD受け入れシナリオ

```gherkin
Scenario: [policy] セクションなしの既存 config で auto は拒否される
  Given config.toml に trash_mode = false だけが書かれている
  When  ユーザーが sasurahime auto を実行する
  Then  コマンドは [policy] セクションがないことを理由に拒否する
  And   削除は一切行われない

Scenario: [policy] セクションありの config で auto は実行される
  Given config.toml に [policy] older_than_days = 30 が書かれている
  When  ユーザーが sasurahime auto を実行する
  Then  ポリシーに従った無人削除が実行される
```

## 受け入れ基準

- [x] parse_config_file が `policy.present` を「設定ファイルに [policy] セクションが実在するときのみ true」にする（serde デフォルト由来の true をやめる。serde_json の Value レベルでセクション実在を判定する）
- [x] config.rs:245-249 のテストに「ファイル実在・セクションなし → present == false」のケースを追加
- [x] main.rs:864-868 のガードは現状維持（present を見るだけ）で、挙動が doc と一致することをテストで pin
- [x] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 統合: parse_config_file の3ケース（不在ファイル / 実在・セクションなし / 実在・セクションあり）
- 単体: serde デフォルトと present 判定の分離

## 実装者向け注記

RawConfig.policy は `#[serde(default)]` のため、セクションなしでもデシリアライズに成功する。present の判定は serde_json::Value として ["policy"] を見るか、RawConfig に Option<PolicySection> を導入する。parse 側だけ変え、main.rs 側のガードは触らない。

## 検証記録 (2026-10-24)

- 実装コミット: 89d37ba (wt-auto、1023a にマージ済み)
- 内容: `RawConfig.policy` を `Option<PolicySection>` に変更し、`policy.present = raw.policy.is_some()`。serde デフォルト由来の true を廃止。config.rs に「ファイル実在・セクションなし → present == false」「セクション実在 → present == true」のテストを追加。
- 備考: main.rs のガードは現状維持 (present を見るだけ)。tests/auto_policy.rs で挙動を pin。
- 検証: 1023a HEAD 64c44a1 で lib 失敗 ⊆ 既知 TCC セット / bin 30/30 / 統合 38 ターゲット green / clippy 0 warnings / fmt clean。
