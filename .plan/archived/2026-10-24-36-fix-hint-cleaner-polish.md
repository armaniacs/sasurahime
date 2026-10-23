# PBI: hint cleaner の仕上げ（skip 後の [OK] と osascript エスケープ）

## ユーザーストーリー
hint auto-clean を使うユーザーとして、スキップされた削除が [OK] と表示されず、AppleScript 補間が安全であってほしい、なぜなら hint/cleaner.rs:79-86 と 105-112 はスキップ後も無条件で `  [OK]` を出力し（旧実装はエラーで中断）、hint/cleaner.rs:21 は `quit app "{quit_app}"` に `"` をエスケープせず補間する（現行値はすべてコンパイル時定数で安全、latent）から。

## 優先度
- 順位: 9 / 10
- RICEスコア: 8（Reach=10 / Impact=0.5 / Confidence=80% / Effort=0.5人日）
- 根拠: latent injection はリスク軽減効果でテスト衛生（37）との同点タイブレーク勝ち。

## BDD受け入れシナリオ
Scenario: スキップされた削除は [OK] を出さない
  Given 削除がスキップされるエントリ
  When  hint auto-clean を実行する
  Then  そのエントリは警告付きで報告され、[OK] は出ない

Scenario: osascript 補間の安全性
  Given `"` を含む quit_app 値（将来の追加を想定）
  When  quit コマンドが構築される
  Then  補間はエスケープされ、任意の AppleScript が実行されない

## 受け入れ基準
- [x] hint/cleaner.rs:79-86 と 105-112 で、skip 分岐の後に [OK] が出ない（[OK] は削除成功時のみ）
- [x] hint/cleaner.rs:21 の osascript 補間がエスケープされる（または値の不変条件が型/ランタイムで強制される）
- [x] 既存の hint 系テストが green を維持する

## テスト戦略
- 単体: skip 分岐の [OK] 抑制テスト、osascript エスケープテスト
- 統合: 既存の hint auto-clean テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 81a3bbe。
- [OK] は成功時のみ: 両削除サイト（auto_clean_hint / clean_hint_dirs）が skip 理由を Vec<String> に収集し、trailing 無条件 [OK] を completion_line(&[String]) ヘルパーに置換（空 → "  [OK]"、skip あり → "  [SKIPPED] {reasons}"、ファイルの bracket-tag 規約に整合）。per-path log::warn 維持。Err は ? で伝播（fatal は "  Error: {e}"）。
- osascript 安全化: quit_app_script(quit_app) ヘルパーが backslash → double-quote の順にエスケープしてから `quit app "..."` に補間（strip ではなく escape を選択 — 将来の quote 含み値でも意味的に正しいアプリ名を保持）。WHY コメント付き（コンパイル時 KNOWN_ENTRIES テーブル、型は `"`/`\` を禁止しない、escape によりリテラルは早期に閉じない）。シグネチャとテーブルは未変更。
- Red 証拠（現行挙動の純粋抽出後、fix 前）: RecordingRunner で `quit app "Foo"Bar"`（AppleScript リテラルが早期に閉じる）、抽出ヘルパーで `quit app "" & do shell script "pwned"`（インジェクション payload がリテラル外）、skip 理由ありで [OK]。4 failed / 22 passed at RED → 全 green。
- テスト観測方式: クレートに stderr キャプチャ規約がなく real-trash 経路は TCC 環境依存のため、print 判断と script 構築をプライベートヘルパーに抽出して直接単体テスト + mod.rs の wiring テスト（実 quit_app_and_wait 経路の記録された osascript 引数を assert）。6 新規テスト。
- 検証: lib 656 passed / 4 failed（既知 TCC のみ、baseline 650 + 6）、統合 36 ターゲット green、clippy 0、fmt クリーン。pinned テスト auto_clean_deletes_paths_after_quit は green 維持（成功パスは [OK] を出力）。
