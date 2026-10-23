# PBI: クリーナー層の重複バンドル（library_logs 双子 / is_app_bundle / base_path / バージョン保持）

## ユーザーストーリー
library_logs を `clean_with_opts` で使う macOS ユーザーとして、filter と trash_mode が doc コメントどおり効いてほしい、なぜなら `clean_with_opts` が `opts.all` だけを読み、`clean` との双子（~90 行 verbatim）が filter 統合をしない2実装になっている。加えて gradle の「keep all but highest」コピーには browser の symlink ガードがなく、`is_app_bundle` が大文字小文字判定で乖離し、`base_path` が hint/ 内で2重定義されている。

## 優先度
- 順位: 7 / 8
- RICEスコア: 24（Reach=30 / Impact=1 / Confidence=80% / Effort=1.0人日）
- 根拠: ~200 行の重複削除 + gradle symlink ギャップの閉鎖 + library_logs の filter 挙動修正。impact は小さいが局所性の失敗が明確

## BDD受け入れシナリオ
Scenario: clean_with_opts が filter を統合する
  Given `[logs] older_than_days = 5` と trash_mode 設定
  When  `clean logs --all` が clean_with_opts を通る
  Then  older_than_days と trash_mode が効いた結果が返る

Scenario: gradle の保持ロジックが symlink をスキップする
  Given symlink 化された古いバージョンディレクトリ
  When  keep-highest ヘルパーが走る
  Then  symlink は削除候補から除外される（browser と同じ規則）

Scenario: is_app_bundle の判定が1実装になる
  Given `.App` と `.app` の両方の拡張子
  When  判定が走る
  Then  大文字小文字非依存の1実装が使われる

## 受け入れ基準
- [ ] library_logs.rs の clean / clean_with_opts 双子が1実装に統合される（filter 統合 + dry-run タグ出力の重複解消）
- [ ] library_logs.rs の interactive 前に is_terminal ガードが追加される（library_caches.rs:347-353 と同じ規則）
- [ ] keep-highest ヘルパーが1実装になり、gradle も symlink ガードを得る
- [ ] is_app_bundle / list_app_bundles が1実装に統合される（大文字小文字非依存）
- [ ] base_path の2重定義（hint/detector.rs:7 と hint/mod.rs:58）が解消される
- [ ] cleaner.rs:132-133 の stale doc コメントが実態に合う1実装の doc に置き換わる

## テスト戦略
- 単体: keep-highest（symlink 含む）、is_app_bundle、filter 統合の表テスト
- 統合: library_logs / gradle / browser の既存テストが green を維持

## 見積もり
1ストーリーポイント

## Definition of Done
- [ ] 全BDDシナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み
