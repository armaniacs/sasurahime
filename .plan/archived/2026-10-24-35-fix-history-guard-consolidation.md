# PBI: history ガードの統合と 0 バイト削除の扱い

## ユーザーストーリー
clean 履歴を監査するユーザーとして、ガードが 1 箇所に集まり 0 バイト削除の扱いが明確であってほしい、なぜなら batch.rs:96 の `!dry_run && bytes_freed > 0` と history.rs:236-238 の内側の `freed_bytes == 0` ガードが二重に存在し、空ディレクトリや brew の parse が 0 を返す実削除が監査証跡から落ちるから。

## 優先度
- 順位: 8 / 10
- RICEスコア: 16（Reach=20 / Impact=0.5 / Confidence=80% / Effort=0.5人日）
- 根拠: 監査証跡の一貫性。既存挙動の保持が前提で、二重ガードの統合が主目的。

## BDD受け入れシナリオ
Scenario: ガードの1実装
  Given run_batch と history の書き込み経路
  When  履歴書き込みを判断する
  Then  ガード式は 1 実装にのみ存在する

Scenario: 0 バイト実削除の扱い
  Given 実際に削除が走ったが解放バイト数が 0 の実行
  When  履歴に書き込むか判断する
  Then  現行挙動（書き込まない）を保持するか、統一の判断が WHY コメントで記録される

## 受け入れ基準
- [x] batch.rs:96 と history.rs:236-238 の二重ガードが 1 実装に統合される
- [x] 0 バイト実削除の扱いが現行どおりか変更か、WHY コメントまたは検証記録で明確になる
- [x] 既存の履歴書き込みテストが green を維持する

## テスト戦略
- 単体: ガード式の統合後テスト
- 統合: 既存の履歴書き込みテスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit c1705da。
- ガード統合: batch.rs の write_history_entry_if_needed（run_batch と単一ターゲット経路の両方の唯一の「記録するか判断」所有者、`if !dry_run && bytes_freed > 0`）を維持し、history.rs の内側の二重ガード（freed_bytes == 0 の return）を削除。grep で write_history_entry の非テスト呼び出しは 1 件（batch.rs:97）のみと証明され、内側ガードは他の呼び出し元を保護していなかった。history.rs には「best-effort 行レコーダ、ポリシーの再判断はしない」doc を付与。
- 0バイトポリシー: 現行（記録しない）を保持し、統合ガードに WHY コメントを明記（zero-byte 実行 — 全削除スキップ、空ディレクトリ、CLI ツールの 0 freed 報告 — は記録されない。履歴テーブルは回収バイトを追跡し、バイトなしの削除は行を持たない）。新規テストで pin。
- テスト: 新規1件（zero-byte real run → no row）。既存テストの変更なし。新テストは EnvGuard レース回避のため EnvGuard+tempdir-HOME パターンを意図的に omit（zero-byte ガードは I/O 前に短絡するためレースフリーで観測可能）。
- 検証: lib 651 passed / 4 failed（既知 TCC のみ、3連続実行で安定）、統合 36 ターゲット green、clippy 0、fmt クリーン。
