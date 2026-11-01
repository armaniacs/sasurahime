出典: TTY 対話ウォークスルーgap の実測（2026-11-01、main @ ddc6395）+ /pbi-backlog-prioritizer の 2 候補のうち順位 2。PBI-102（選択 port のテスト注入 seam）と**代替関係**にあり、本 PBI は PBI-102 着地後に再判定する（将来候補）。

# PBI-103: 疑似 TTY ハーネスで対話ループ全体を実際の入力系列で歩く E2E（将来候補）

## ユーザーストーリー
sasurahime の保守担当者として、疑似 TTY（portable-pty / `script -q`）で対話ループ全体（TUI 起動 → メニュー描画 → キー入力 → confirm → clean → freed 表示 → summary）を実際のキー入力系列で歩く E2E がほしい、なぜなら dialoguer 自体の描画とキー操作を含む体験の回帰を検出したいから

## 優先度
- 順位: 2 / 全 2 候補
- RICEスコア: 1.0（Reach=2 / Impact=1.5 / Confidence=50% / Effort=1.5 人週）
- 根拠: PBI-102（スコア ≒ 8.5）と同じ価値帯（対話フローの E2E 化）だが、(1) flaky リスクが構造的（dialoguer 0.12 の keypress は raw mode + terminal size 検出 + ANSI 出力に依存し、疑似 TTY では winsize 設定・expect 文字列待ち・pty 実装の環境差で CI flaky が出やすい）、(2) 依存追加または OS 依存（portable-pty crate / BSD `script` の差分）、(3) **PBI-102 と代替関係**にあり、PBI-102 着地時の残存価値は dialoguer 自体の描画とキー操作（第三者 crate の契約、sasurahime 自体のコードではない）のみに縮小。Confidence 50% はこの flaky リスクと価値縮小の不確かさを反映。着手は PBI-102 着地後の再判定（価値が残っていれば縮小スコープで、残らなければ close）

## 背景/問題

PBI-102 が着地すると、選択 port を通る対話経路（select → 削除 → freed）はモック注入で E2E 化される。ただしその E2E は **dialoguer 自体を通らない**: メニューの実際の描画（ANSI 制御、矢印キーでの移動、スペースでの選択、Enter での確定）は TtyPrompt の dialoguer 呼び出しの中で起き、テストからは port に差し替えられている。dialoguer 0.12 の描画・キー操作が sasurahime の期待と外れた場合（例: dialoguer のバージョンアップで描画が変わる、winsize 検出の挙動が変わる）、PBI-102 の E2E は検出できない。

実コードの実測（main @ ddc6395）:

1. dialoguer 0.12 は TtyPrompt（prompt.rs:136）から呼ばれ、raw mode + terminal size 検出を要求する。疑似 TTY 無しではどのテストもこの経路を通せない
2. 疑似 TTY ハーネスは src/ にも tests/ にも存在しない（grep で確認済み）。`script -q` / pexpect 相当の利用も皆無
3. 疑似 TTY なしで対話ループを歩く唯一の手段が PBI-102 のモック注入（dialoguer を通らない）

## 根本原因

- 攻撃者/ハッカー視点: 対話ループの端末体験（描画とキー操作）がテスト可能な契約として存在しない。dialoguer のバージョンアップや端末環境の差分が、TUI 利用者にだけ見える退行として静かに着地する
- 半年後の保守担当者視点: 疑似 TTY ハーネスは構築コスト（winsize 設定、expect 待ち、flaky 対策）が高く、検出する退行の多くが dialoguer 自体の挙動（sasurahime のコード外）。ハーネスの保守コストと検出価値の天秤が doc に無い
- エンドユーザー視点: TUI の実際の操作感（矢印キー、スペース選択）の退行は、疑似 TTY が無い限りリリース後にしか発見できない

## 発見の詳細（実コードの確認、main @ ddc6395）

- dialoguer 0.12（Cargo.lock 実測）: TtyPrompt::multi_select（prompt.rs:136）から `MultiSelect::new().items(...).interact()` 相当を呼ぶ。raw mode と winsize を要求
- 検討対象のハーネス候補: (1) portable-pty crate（Rust 依存追加、winsize 制御可能）、(2) `script -q`（macOS BSD 差分あり、外部プロセス依存）、(3) pexpect 相当（Python 依存で Rust リポジトリに不向き）
- flaky の構造: expect 文字列待ち（タイムアウト）、ANSI 出力の非決定性、CI ランナーの pty 実装差。macOS 専用リポジトリ（AGENTS.md）のため Linux/Windows 差分は対象外だが、CI ランナー間の差は残る
- PBI-102 との関係: 代替。PBI-102 着地後は「sasurahime 自体のコードの退行」は PBI-102 の E2E が検出し、本 PBI の検出範囲は「dialoguer と端末環境の契約」に縮小する

## BDD 受け入れシナリオ

```gherkin
Scenario: 疑似 TTY で対話ループ全体が歩ける
  Given 疑似 TTY に実際のキー入力系列（スペース / Enter / y）を流した
  When  sasurahime を TUI 起動する（引数なし）
  Then  メニュー描画 → 選択 → confirm → clean → freed 表示 → summary まで進む
  And   freed が summary に出る

Scenario: 端末サイズの差分で描画が崩れない
  Given 疑似 TTY の winsize を小さく設定した（例: 80x24 未満）
  When  対話ループが進む
  Then  dialoguer の描画が期待どおりになり、キー入力の解釈が変わらない
```

## 受け入れ基準
- [ ] 疑似 TTY ハーネス（portable-pty または `script -q`）が tests/ に導入される（依存追加の根拠が記録されている）
- [ ] 対話ループ全体を実際のキー入力系列で歩く E2E が 1 本以上
- [ ] flaky 対策（タイムアウト、リトライ、expect 待ち）が tests/AGENTS.md に文書化される
- [ ] **PBI-102 の着地結果を確認してから着手する**（残存価値の再判定: dialoguer 自体の描画を守る価値が残るか。残らなければ本 PBI は close する）

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- 疑似 TTY + 実キー入力系列による対話ループ全体のウォークスルー

### 統合テスト
- 対象外（PBI-102 が担当）

### 単体テスト
- 対象外（winsize / raw mode は疑似 TTY でしか歩けない）

## 実装アプローチ
- **Outside-In**: 疑似 TTY ハーネス（最小: 1 キー入力系列で 1 ループ）→ E2E 1 本 → flaky 対策 → 拡張（winsize 差分など）
- スパイク分離: ハーネスの実現可能性（flaky レート）を最初の 1 本で計測してから拡張する

## 見積もり
5 ストーリーポイント（要チームでの見積もり。flaky 対策次第で 8 を超える）

## 技術的考慮事項
- 依存関係: portable-pty crate（追加）または `script`（OS 依存）。追加の根拠をコミットメッセージに記録する
- テスタビリティ: expect 文字列待ちのタイムアウトはテストごとに明示する（暗黙の無制限待ちを禁止）
- 非機能要件: macOS 専用リポジトリ（arm64 + x86_64）。Linux/Windows の pty 差分は対象外

## 実装者向け注記

### 現状コードの確認
（着手前に必ず実行すること）
```bash
grep -rn "portable_pty\|script -q\|pexpect" tests/ src/
grep -n "dialoguer" Cargo.toml Cargo.lock
```
疑似 TTY は src/ にも tests/ にも存在しない（本 PBI が初回導入）。PBI-102 の着地結果（対話経路の E2E がモック注入で存在するか）を先に確認すること。

### 実装手順
1. PBI-102 の着地結果を確認（対話経路の E2E がモック注入で存在するか、検出できない退行は何か）
2. スパイク: portable-pty で最小ハーネス（1 キー入力系列で 1 ループ）を構築し、flaky レートを計測（ローカル 10 回連続実行）
3. flaky レートが閾値を超える場合は `script -q` 経路を試す（macOS BSD 差分に注意）
4. E2E 1 本（対話ループ全体）を green にする
5. flaky 対策を tests/AGENTS.md に文書化
6. 残存価値の判定: dialoguer 自体の描画を守る価値が CI で実在するかを 1 スプリント観察してから拡張を決める

### 落とし穴
- dialoguer 0.12 の keypress は winsize 検出に依存 ― 疑似 TTY で winsize を設定しないと描画が崩れ、キー入力の解釈が変わる
- expect 文字列待ちの無制限待ちは CI をハングさせる ― タイムアウト必須
- macOS の `script -q` は BSD 版（GNU とオプションが違う）。macOS 専用リポジトリのため GNU 差分は不要だが、CI ランナーの macOS バージョン差分は残る
- PBI-102 が着地済みの場合、本 PBI の検出範囲は dialoguer 契約のみ ― 価値が小さければ着手せず close することが妥当な判断

## Definition of Done
- [ ] 疑似 TTY ハーネスが tests/ に導入され、依存追加の根拠が記録されている
- [ ] 対話ループ全体を歩く E2E が 1 本以上で flaky 対策が文書化されている
- [ ] PBI-102 との重複がない（残存価値の再判定が記録されている）
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（tests/AGENTS.md のハーネス規約）

## 再判定（2026-11-01、close）

PBI-102 の着地結果（`04a8733` feat + `b065e6ad` follow-up + `b231fd0` docs、focused review Approve）を確認した結果、本 PBI は **close**（着手せず）とする。

- PBI-102 の seam で「対話で選ぶ→削除される→freed が出る」流れが E2E で歩けるようになった（成長 variant の E2E が freed-at-delete-time 契約を対話筋そのもので pin、handback 記録断言で seam 破壊も検出）。**sasurahime 自体のコードの退行は PBI-102 の E2E が検出する**
- 残存価値は「dialoguer 自体の描画とキー操作（ANSI、矢印キー、winsize 検出）を守る価値」のみ ― 第三者 crate の契約で、sasurahime のコードは戻り値の選択結果だけに依存し描画に依存しない。dialoguer は Cargo.lock で pin され、バージョンアップは意図的な作業でその都度手動検証に戻せる
- 5 ストーリーポイント + 構造的 flaky リスク（raw mode + winsize + pty 実装の環境差）+ 依存追加（portable-pty / `script`）をその残存価値に支払う釣り合いが取れない（本 PBI の優先度節 Confidence 50% の根拠どおり）
- 将来 dialoguer のバージョンアップで描画退行が現実に着地した場合、この PBI を再オープンする判断点（Cargo.lock の dialoguer pin を変える作業の受入基準として）
