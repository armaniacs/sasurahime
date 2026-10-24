# PBI-50: TUI/explorer 自己再実行の in-process 化（Total freed 過小表示の修正）

## ユーザーストーリー

TUI でサブターゲットを選んで掃除するユーザーとして、Total freed に実際の解放バイト数が表示されてほしい。なぜなら interactive.rs:256-258 の自己再実行が bytes_freed: 0 を捏造して合計を過小表示し、explorer.rs:245+278-290 も自分自身を PATH 経由で再実行していて壊れやすいから。

## 発見元

アーキテクチャ deepening 後日候補（pbi/2026-10-23-27 索引、2026-10-24 再評価）。RICE: Reach 10 / Impact 2（ユーザー可視の欠陥）/ Conf 80% / Effort 1.5 = 約 11。残存候補で唯一のユーザー可視欠陥。

## BDD受け入れシナリオ

```gherkin
Scenario: TUI でサブターゲットを選んで掃除すると合計に実バイトが反映される
  Given サブターゲット掃除の in-process シームが存在する
  When  TUI でサブターゲット（例: xcode --sub DerivedData）を選んで掃除する
  Then  実際の CleanResult が返り Total freed に実バイトが反映される
  And   bytes_freed: 0 の捏造が消える

Scenario: explorer で管理エントリを選ぶと in-process で掃除される
  Given explorer の掃除が自分自身の再実行でなく in-process 呼び出しになった
  When  管理エントリを選んで実行する
  Then  PATH フォールバックに依存せず結果が返る
  And   失敗は exit コードでなく Result で扱われる
```

## 受け入れ基準

- [x] interactive.rs:256-258 の `clean <name> --sub` 自己再実行が in-process 呼び出しに置き換わり、実際の CleanResult を返す（262-268 の捏造除去）
- [x] explorer.rs:245+278-290 の自己再実行が in-process 呼び出しに置き換わる（current_exe / PATH フォールバック依存の除去）
- [x] サブターゲット掃除の in-process シーム（cleaner 名 + sub 名で dispatch）が追加される
- [x] explorer 再実行が含んでいた scan_cache flush（main.rs:1423-1425 相当）の扱いが決定され記録される（複製または意図的な省略）
- [x] set_skip_confirm ブラケット意味論が維持される（interactive.rs:246/308）
- [x] 新シームのテスト追加、全ゲート green

## テスト戦略（t_wadaスタイル）

- 統合: シーム経由のサブターゲット掃除が CleanResult を返すこと
- 単体: シームの名前解決（未知の cleaner / sub 名のエラー扱い）

## 実装者向け注記

### 現状コードの確認（2026-10-24 調査済み）
- src/interactive.rs:256-258 自己再実行（`Command::new(current_exe)` + `clean <name> --sub <sub>`）、262-268 で bytes_freed: 0 を捏造、310 の Total freed が過小表示
- src/explorer.rs:245 current_exe + PATH フォールバック、278-290 で clean を spawn、失敗は exit コード検査
- blocker: サブターゲット掃除に in-process シームがない（CLI `--sub` フラグのみ、CleanTarget::Xcode arm in main.rs）

### 実装手順
1. サブターゲット用の in-process シームを追加（cleaner 名 + sub 名で Cleaner を解決して clean_with_opts を呼ぶ）
2. interactive.rs と explorer.rs をシーム経由に置き換える
3. scan_cache flush の扱いを決めて記録する

### 落とし穴
- バッチ B（PBI-48/49 の main.rs / interactive.rs マージ後に着手。両方 interactive.rs と main.rs に触れるため直列）
- 掃除済み項目の再スキャンが必要な挙動があれば explorer 側で維持すること
- 既存 interactive.rs テスト（364-505）は純粋ヘルパーのみで、新シームのテストは追加でよい

## 検証記録 (2026-10-24)

- 実装コミット: d750c9c (wt-tui、1023a にマージ済み)
- 内容: CleanByName シーム (src/cleaner.rs:147、resolver callback を main.rs から配線、Option A)。interactive.rs clean_sub_target と explorer.rs clean_managed_entry が自己再実行を置換。Total freed に実バイトが反映
- 備考 (正直な逸脱): シームは clean_with_opts でなく bare .clean() (dispatch_clean と同一の CLI パリティ、既定 opts で挙動同一)。run_explore に reporter パラメータ追加 (子の出力形状と suppress フラグ維持のため)。scan_cache flush は意図的に廃止し explorer.rs と clean_by_name doc に記録
- ボーナス修正: 旧再実行は TUI 表示名を --sub に渡し XcodeSubcategory::parse が黙って拒否 (Archives 選択で DerivedData を掃除)。シームは両表記を受理し pin テスト追加
- 検証: RED 6 lib + 10 bin (E0425) → GREEN。1023a HEAD d750c9c で lib 723/4 / bin 37/37 / 統合 40 ターゲット 0 failed / clippy 0 warnings / fmt clean
