# PBI-48: HOME 解決の 1 本化（4 サイト 2 セマンティクス + stale コメント修正）

## ユーザーストーリー

保守担当者として、HOME 解決の env → dirs チェーンが 1 か所にまとまっていてほしい。なぜなら 4 サイトに 2 セマンティクスが重複し、generic.rs:851-855 のコメントが「main::home は空 HOME をフィルタしない」と主張するが、PBI 47 の修正で main.rs:482 はフィルタするようになり、コメントが既に嘘をついている（重複が 1 回噛み付いた実績）から。

## 発見元

アーキテクチャ deepening 後日候補（pbi/2026-10-23-27 索引、2026-10-24 再評価）。RICE: Reach 40 / Impact 1 / Conf 80% / Effort 0.5 = 64（残存候補の最上位）。

## BDD受け入れシナリオ

```gherkin
Scenario: 統一リゾルバは env → dirs の順で解決し空 HOME を拒む
  Given HOME 解決が 1 つの共有関数に統一されている
  When  HOME 環境変数が空文字列の状態で解決する
  Then  env は不採用となり dirs::home_dir にフォールバックする
  And   is_safe_delete_target 経路では dirs 失敗時に fail-closed になる

Scenario: dirs 失敗時のセマンティクスは呼び出し側が選べる
  Given 共有リゾルバが Option を返す
  When  dirs::home_dir が None を返す
  Then  main::home は expect で停止し、config/history は既定値に落ちる
  And   各呼び出し側の既存 pin テストが green を維持する
```

## 受け入れ基準

- [x] HOME 解決の env → dirs チェーンが 1 つの共有関数に統一される（main.rs home() / generic.rs is_safe_delete_target 内 / config.rs / history.rs の 4 サイトが通る）
- [x] generic.rs:851-855 の stale コメントが削除または訂正される（「main::home は空 HOME をフィルタしない」は PBI 47 の修正以降、偽）
- [x] main::home の expect セマンティクス（pin: main.rs:1436-1459）と is_safe_delete_target の fail-closed 空フィルタ（pin: generic.rs:1495-1503, 1760-1925）が維持される
- [x] lib 失敗 ⊆ 既知 TCC セット / bin / 統合 / clippy / fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 統合: 既存 pin テスト（main.rs home_filters_an_empty_home_var / generic.rs is_safe_delete_target 系）の維持
- 単体: 共有リゾルバの空 HOME・dirs 失敗ケース

## 実装者向け注記

### 現状コードの確認（2026-10-24 調査済み）
- src/main.rs:476-486 home()（env 空フィルタ → dirs → expect、pin main.rs:1436-1459）
- src/cleaners/generic.rs:856-860 同チェーン inline（stale コメント 851-855）
- src/config.rs:176 dirs::home_dir().unwrap_or_default()
- src/history.rs:239 dirs::home_dir().unwrap_or_default()（main.rs:1408 は home を渡すが write 経路は独立に再解決）

### 実装手順
1. 共有リゾルバ（env 空フィルタ → dirs、Option 返し）を 1 か所に置く
2. 4 サイトを呼び出し側セマンティクス（expect / fail-closed / default）のまま通す
3. stale コメントを訂正

### 落とし穴
- is_safe_delete_target の空フィルタは fail-closed、config/history の unwrap_or_default は空文字列を返す ― この差は呼び出し側に残してよい（リゾルバは取得だけを共有）
- main.rs / generic.rs は PBI 38-47 の変更直後のホットファイル。既存 pin テストを壊さないこと
- history.rs:631 は dirs::home_dir().is_some() を assert する ― 影響なしを確認

## 検証記録 (2026-10-24)

- 実装コミット: 226c2c3 (wt-home、1023a にマージ済み)
- 内容: resolved_home_dir (src/config.rs:152、Option<PathBuf>) に統一。4 呼び出しサイト (main.rs home() expect / generic.rs is_safe_delete_target fail-closed / config.rs unwrap_or_default / history.rs unwrap_or_default) が通過。generic.rs の stale コメント (「main::home は空 HOME をフィルタしない」) を訂正
- 備考 (正直な逸脱): EnvGuard::unset を src/test_helpers.rs に追加 (PBI 許可範囲)。dirs 完全失敗 (passwd 不在) はプロセス内で強制不能のため、HOME-unset フォールバックでカバーし Option 返しを呼び出し側の契約として pin
- 検証: RED 3 件 (E0425 compile) → GREEN。1023a HEAD d750c9c で lib 723/4 (失敗 ⊆ 既知 TCC セット) / bin 37/37 / 統合 40 ターゲット 0 failed / clippy 0 warnings / fmt clean
