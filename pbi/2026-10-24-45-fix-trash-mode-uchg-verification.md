# PBI-45: trash モードの uchg 取り扱いを検証・記録（trash.rs doc 正確化込み）

## ユーザーストーリー

mise 等の uchg 付きキャッシュを清掃するユーザーとして、trash モードでも immutable フラグ付き項目が正しく削除（または明示的にスキップ）されてほしい。なぜなら、seam は trash モードで chflags を実行せず「Finder がフラグを処理する」という未検証の根拠で動いており、main は両モードで chflags していたため挙動リグレッションの疑いがあるから。

## 発見元

1023a 全体レビュー Major #5（trash.rs:114-121）。

## BDD受け入れシナリオ

```gherkin
Scenario: trash モードで uchg 付き項目の挙動が決定論的に文書化されている
  Given trash.rs の doc が trash モードの uchg 取り扱いを主張している
  When  実装者が trash 5.2.6 の Finder バックエンド挙動を検証する
  Then  検証結果（Finder が uchg を処理するか/拒否時にどうなるか）が記録される
  And   doc が検証結果と一致する

Scenario: 拒否時は skippable として既存の分類に従う
  Given Finder が uchg 付き項目の trash を拒否する環境がある
  When  safe_delete が Trash モードで削除を試みる
  Then  エラーは "trash failed" 経由で skippable に分類される（現行分類の維持）
```

## 受け入れ基準

- [x] trash 5.2.6 の vendored macos/mod.rs を読み、Finder バックエンドの uchg 取り扱いを検証（osascript の delete が uchg 付き項目を処理するか）し、検証記録を残す
- [x] 検証結果に基づき trash.rs:114-121 の doc を修正（主張が検証できない場合は「未検証」を明示するか、permanent と同様の chflags 前処理を追加するかを判断して記録）
- [x] trash.rs:95 の「Every deletion in the codebase goes through here」主張を修正（外部ツールは自身で削除する）
- [x] trash.rs:70-72 の delete_path bypass wrapper の doc 更新（stale な「Cleaner-level call sites keep using this」）
- [x] lib/integration/clippy/fmt の全ゲート green

## テスト戦略（t_wadaスタイル）

- 検証ファースト: 実装変更は検証結果次第。vendored trash クレートのソース読み取りが主、書ける範囲で単体テスト

## 実装者向け注記

verify-first の PBI。Finder/TCC のライブテストはサンドボックスで TCC ダイアログを出すため実行しない（vendor ソースの読み取りで判定）。chflags 前処理を追加する場合のみ実装+テストが必要。seam 分類（is_skippable_error）は変更しない。

## 検証記録 (2026-10-24)

- 実装コミット: 75b740f (1023a、doc-only)
- 検証内容: vendored trash 5.2.6 の src/macos/mod.rs を読み、クレート全体で uchg/chflags/immutable の取り扱いが 0 件であることを grep で確認。Finder バックエンドは `osascript -e 'tell application "Finder" to delete { POSIX file "..." }'`、NsFileManager バックエンドは `NSFileManager::trashItemAtURL` のみ。
- 判断: Finder が uchg 付き項目を解除するか拒否するかはクレートソースからは判定不能のため、trash.rs の doc は「未検証」を明示し、refusal 時の "trash failed: ..." → is_skippable_error スキップ経路を記録。permanent と同様の chflags 前処理は追加しない (trash モードは rm を経由しないため)。trash.rs:95 の主張は「Every PathCleaner-based deletion」に修正し外部ツールの自己削除を注記、delete_path wrapper の doc も test/compat-only (ゼロ呼び出し) に更新。
- 検証: 1023a HEAD 64c44a1 で lib 失敗 ⊆ 既知 TCC セット / bin 30/30 / 統合 38 ターゲット green / clippy 0 warnings / fmt clean。
