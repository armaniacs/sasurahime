# PBI: colima の非 TTY 安全床と --yes --permanent の終了コード

## ユーザーストーリー
スクリプトで sasurahime を実行する開発者として、Risky な掃除が非 TTY で無確認に進まないことがほしい、なぜなら `clean all --include-manual` が `~/.colima`（VM ディスク含む）を確認なしに削除し得る状態は取り返しがつかないから。

## 優先度
- 順位: 1 / 8
- RICE スコア: 12.0（Reach=3 / Impact=3 / Confidence=100% / Effort=0.75 人週）
- 根拠: 安全上の実害が 2 つ（colima の無確認全削除、`--yes --permanent` の偽成功）。深掘りで行番号まで実証済みで確信度が最大。他 7 候補は構造改善だが、これは危険の即時封塞。

## 発見の詳細（深掘り結果）
1. generic.rs:557-558: `!confirm_suppressed() && is_interactive()` により非 TTY では colima の confirm が静かにスキップされ、colima 未インストールならフォールバック（generic.rs:576-628）で `~/.colima` 全削除が無確認で走る。ios_backup.rs:105 と apfs_snapshot.rs:240 は gate_interactive により空結果で拒否するのに colima だけ続行する = Risky 3 種で安全床が不整合。
2. interactive.rs:302-305: `--yes --permanent` が非 TTY で "Aborted." + exit 0。スクリプトが成功と誤認する。main.rs:330 の CLI doc「without prompting」とも齟齬。

## BDD受け入れシナリオ
Scenario: 非 TTY の clean all --include-manual で colima の confirm が拒否される
  Given 非 TTY stdin と colima 未インストールの環境
  When  `clean all --include-manual` を実行する
  Then  colima は "[colima] not a terminal" 系の remedy 行を出して skip される
  And   `~/.colima` への削除は一切発生しない
  And   ios-backup / apfs-snapshot と同じ「非 TTY で拒否」の安全床になる

Scenario: --yes --permanent が非 TTY で非ゼロ終了する
  Given 非 TTY stdin と trash mode オフ
  When  `--yes --permanent` を実行する
  Then  "Aborted." が表示され exit コードは非ゼロ
  And   スクリプトが失敗を検出できる

Scenario: TTY では従来どおり confirm が出る
  Given TTY stdin と SkipConfirmGuard 未抑制
  When  colima を含む掃除を実行する
  Then  確認プロンプトが出る（現行挙動の維持）

## 受け入れ基準
- [ ] colima の confirm は非 TTY で拒否 + remedy 行（ios-backup/apfs と整合）
- [ ] `--yes --permanent` の非 TTY 中止は非ゼロ終了
- [ ] TTY の confirm 挙動は不変（tests/interactive.rs の既存 pin が生存）
- [ ] generic.rs:557-558 の静的スキップ gate が「なぜ拒否か」の WHY コメントを持つ
- [ ] CHANGELOG に挙動変更を記載
- [ ] cargo test --no-fail-fast（既知 TCC 4 件のみ失敗）/ clippy -D warnings 0 / fmt --check

## テスト戦略
- E2E: 非 TTY `clean all --include-manual` で colima skip + `~/.colima` 無削除を tempdir HOME で pin
- E2E: `--yes --permanent` 非 TTY で非ゼロ終了を pin
- 統合: 既存 tests/interactive.rs:304-335（write_stdin "y\n"）が生存すること
- 単体: generic.rs の gate 条件の分岐

## 見積もり
0.75 人週

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（CHANGELOG + CLI doc の「without prompting」整合）
