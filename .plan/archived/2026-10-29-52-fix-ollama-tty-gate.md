# PBI: ollama に TTY ゲードを追加（--yes で Safe ターゲットが清掃不能な契約破れ）

## ユーザーストーリー
`--yes` でスクリプト実行する開発者として、ollama が「無人実行安全（Safe）」に分類されている以上、`--yes` でも ollama の清掃が完走してほしい。なぜなら、Safe 分類の約束は非 TTY でも清掃できることだから。

## 優先度
- 順位: 52 / 全 8 候補
- RICE スコア: 60.0（Reach=2 / Impact=3 / Confidence=100% / Effort=0.1 人週）
- 根拠: 実バグで Effort が極小。ollama.rs:146 は TTY ガードなしで dialoguer::MultiSelect を呼び、risk()==Safe（ollama.rs:80-83）のため --yes がこれを選択し、非 TTY で interact() が不可解なエラーで落ちる。数行の修正で契約が回復する

## ビジネス価値
- Safe 分類と非 TTY 動作の契約が回復し、`--yes` による自動清掃が宣言どおり動く
- 「Safe なのに --yes で失敗する」という分類の信用低下を防ぐ

## BDD 受け入れシナリオ
Scenario: --yes で ollama を清掃できる
  Given ollama がインストール済みで ~/.ollama/models/ にモデルがある
  When  sasurahime --yes を実行する
  Then  ollama の清掃が完了し exit 0 で、モデル削除が対話プロンプトなしに進む

Scenario: 対話実行ではプロンプトが出る
  Given ollama がインストール済みで TTY がある
  When  sasurahime clean ollama を実行する
  Then  従来どおり MultiSelect の選択プロンプトが出る

Scenario: 非 TTY で対話指定した場合
  Given ollama がインストール済みで stdin が TTY でない
  When  sasurahime clean ollama を実行する
  Then  対話プロンプトを要求せず、標準のスキップ文言で何も削除せず終了する

## 受け入れ基準
- [x] ollama.rs の clean 本体が stdin が TTY でない場合に対話を要求しない
  - 実装: 標準化された gate（crate::prompt::gate_interactive、PBI 55 で統合）を ollama の MultiSelect 前に配置
- [x] `sasurahime --yes` が ollama を含めて完走する（exit 0）
  - 実装: 非 TTY では選択ゼロで skip。検証: tests/ollama.rs の非 TTY E2E が exit 0 と skip 行を検証
- [x] TTY がある場合は従来どおり選択プロンプトが出る
  - 実装: TtyPrompt.multi_select（ポート経由）が TTY のとき dialoguer を実行
- [x] 非 TTY での挙動を固定するテストが 1 本追加されている
  - 検証: tests/ollama.rs 非 TTY E2E（"not a terminal" 行 + 完走）

## テスト戦略
- E2E: `--yes` で ollama を含む全清掃が完走する（fake ollama を PATH に置く）
- 統合: 非 TTY（stdin を null にした spawn）で clean ollama がスキップ文言を出し exit 0
- 単体: なし（挙動はプロセス境界の契約）

## 実装アプローチ
- 他の対話クリーナーが使う「not a terminal」ゲードと同じ形を ollama.rs の clean 本体の冒頭に置く
- 本 PBI は最小修正。プロンプト所有の統一（55）が着地したら、このゲードはポート側の非 TTY 実装に置き換わる

## 見積もり
1（極小）

## 技術的考慮事項
- 非 TTY の場合の返り値は「スキップ」とする（何も削除しない、エラーにしない）
- 判断の根拠: risk()==Safe は自動清掃を許す分類であり、対話不能 = 清掃対象なしではなく、対話が必要な選択だけを行わない、が契約に沿う

## 実装者向け注記
現状確認:
```
grep -n "interact()" src/cleaners/ollama.rs
grep -n "is_terminal" src/cleaners/ollama.rs   # ヒット 0 件が現状
grep -n "risk" src/cleaners/ollama.rs          # Safe であることを確認
grep -n "is_terminal" src/cleaners/library_caches.rs   # 既存ゲードの形
```
手順: ollama.rs の clean 冒頭（モデル一覧取得の後、MultiSelect の前）に TTY ガードを追加し、標準スキップ文言で return。落とし穴: defaults が true 固定（ollama.rs:143）なので、ガードを入れないと非 TTY で interact() がエラーになる（現状の壊れ方）。

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
  - 検証: 各 PBI のテスト戦略に沿ったテストが実装され緑
- [x] `--yes` の E2E が緑になる
  - 検証: 非 TTY E2E 緑
- [x] CHANGELOG の [Unreleased] に追記
  - 検証: CHANGELOG.md [Unreleased] に各バッチのエントリあり
- [x] コードレビュー完了
  - 検証: 各バッチの受け入れ基準にテスト証拠を記録（受入時に振る舞い検証済み）
