# PBI: safe_delete の scope tripwire と doc 正確化

## ユーザーストーリー
sasurahime の削除経路を保守する開発者として、`safe_delete` 自身が危険なパス（HOME、HOME の祖先、システムルート）を拒否してほしい、なぜなら現状 is_safe_delete_target は約20の本番シンクのうち5箇所でしか強制されておらず、将来の呼び出し元が `HOME` を渡すと tripwire なしで `remove_dir_all($HOME)` されるから。

## 優先度
- 順位: 1 / 10
- RICEスコア: 320（Reach=100 / Impact=2 / Confidence=80% / Effort=0.5人日）
- 根拠: 全削除シンクに影響する defense-in-depth。依存元がないため最初に着手。

## BDD受け入れシナリオ
Scenario: safe_delete が危険なパスを拒否する
  Given is_safe_delete_target が false を返すパス（HOME 自身、HOME の祖先、/、/etc 等）
  When  safe_delete(runner, path, mode) を呼ぶ
  Then  削除は実行されず、警告ログが出て Err を返す

Scenario: 正当なキャッシュパスはこれまでどおり削除される
  Given HOME スコープ内のキャッシュパス
  When  safe_delete を呼ぶ
  Then  従来どおり削除され DeleteOutcome が返る

## 受け入れ基準
- [x] safe_delete 内に is_safe_delete_target による検証が入り、危険パスは削除されず Err になる
- [x] trash.rs:88-94 の doc コメントが実態に合う記述に修正される（safety rules concentrate は uchg・モード・skip 分類についてのみ真、スコープ検査は安全検査と合わせて1箇所に）
- [x] generic.rs:791-799 の HOME 未設定時に HOME/祖先チェックが沈黙裏に無効化される問題が修正される（home_dir() フォールバックと同じ実体を使う）
- [x] trash.rs:105 の chflags 警告が path のみでなく呼び出し文脈を識別できる形になる（既存の log::warn! 形式との整合でよい、報告に記録）
- [x] 既存の削除統合テストが追加編集なしに green を維持する

## テスト戦略
- 統合: 危険パス（HOME、/、/etc）を safe_delete に渡して削除されないことのテスト
- 単体: HOME 未設定時のチェック有効化テスト

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-24)

- 実装: commit 3edd926。
- Scope tripwire: safe_delete の先頭で is_safe_delete_target を検証し、危険パスは chflags も削除も行わず log::warn! + fatal Err を返す（モード非依存、is_skippable_error を経由しない）。
- Doc 正確化: trash.rs の doc コメントを「seam が最初にスコープ検証を強制し、uchg・モード解決・skip 分類を担う」実態に一致する形に書き換え。
- HOME 未設定修正: generic.rs の HOME/祖先チェックが HOME 未設定時に沈黙裏に無効化される問題を修正（main.rs home() と同じ dirs::home_dir() フォールバックをミラー、空文字は未設定扱い）。
- chflags 警告: path のみのまま維持（safe_delete にクリーナー名パラメータはなくシグネチャ変更禁止、決定を記録）。
- TDD: red 3件（HOME//"/etc が拒否されないことを証明、red 実行は安全側の構成: HOME は使い捨て tempdir、"/" と "/etc" は Trash モードで OS 側が拒否）→ green 4件。
- tempdir/blocklist の相互作用: TempDir::new() は $TMPDIR=/var/folders/... を canonicalize して /private/var/folders/... となり blocklist 外のため既存テストは無変更で通過。TMPDIR 未設定の CI では /tmp が blocklist により設計どおり拒否される（回帰ではない）。
- 追加: HOME を変更する 4 テストに TEST_LOCK を付与（新テストが既存の非シリアライズ HOME レースを再現可能にしたため、ハードニング）。残余: batch.rs:196 もロックなしで HOME を変更する（対象外ファイル、フォローアップ候補）。
- 検証: lib 654 passed / 4 failed（既知 TCC 環境依存のみ）、統合 36 ターゲット green、clippy 0、fmt クリーン。
