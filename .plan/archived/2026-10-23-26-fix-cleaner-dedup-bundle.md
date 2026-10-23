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
- [x] library_logs.rs の clean / clean_with_opts 双子が1実装に統合される（filter 統合 + dry-run タグ出力の重複解消）
- [x] library_logs.rs の interactive 前に is_terminal ガードが追加される（library_caches.rs:347-353 と同じ規則）
- [x] keep-highest ヘルパーが1実装になり、gradle も symlink ガードを得る
- [x] is_app_bundle / list_app_bundles が1実装に統合される（大文字小文字非依存）
- [x] base_path の2重定義（hint/detector.rs:7 と hint/mod.rs:58）が解消される
- [x] cleaner.rs:132-133 の stale doc コメントが実態に合う1実装の doc に置き換わる

## テスト戦略
- 単体: keep-highest（symlink 含む）、is_app_bundle、filter 統合の表テスト
- 統合: library_logs / gradle / browser の既存テストが green を維持

## 見積もり
1ストーリーポイント

## Definition of Done
- [x] 全BDDシナリオが自動テストとして実装されパスする
- [x] コードレビュー完了
- [x] ドキュメント更新済み

## 検証記録 (2026-10-23)

- 実装: commit 7b2fc09。
- library_logs 双子統合: clean は clean_with_opts(..., &CleanOptions::default()) に委譲。同一の dry-run プリンタと削除ループは print_dry_run / delete_entries の 1 実装に抽出。clean_with_opts は opts.older_than_days を max(own, opts) でマージ (strict 側が勝つ) し、opts.trash_mode を safe_delete に渡す (None は Global)。interactive 経路の表示は維持。
- is_terminal ガード: interactive_select の前に `!std::io::stdin().is_terminal()` ガード + eprintln を追加 (library_caches.rs と同じ規則)。
- keep-highest ヘルパー: `pub(crate) keep_highest` + `pub(crate) is_version_dir` を gradle.rs に追加。3 サイト (browser, Gradle, JetBrains) が使用。gradle は starts-with-digit ゲートを維持 (browser の空キー規則のみ採用すると modules-2 が削除候補になり実キャッシュを失うため)。JetBrains の同種ギャップも同時に解消。
- TDD: 構造ギャップは symlink テストが旧コードでも green になることを standalone rustc プローブで確認 (DirEntry::file_type().is_dir() が symlink を除外するため、behavioral red は到達不能 = ギャップは構造的)。正規の red はヘルパーテスト先行 (E0425) → 実装 → green。6 テスト追加。
- is_app_bundle 統合: `pub(crate) is_app_bundle` (case-insensitive + is_dir) と list_app_bundles を app_support.rs に統一、orphan_containers が使用。case-sensitive を主張するテストは存在せず、テスト変更ゼロ。統一後の副作用: app_support は *.app という名前のプレーンファイルをバンドルとして列挙しなくなる (orphan の検証済み規則と整合)。
- base_path: 2 定義はバイト同一だったため mod.rs 版を維持、detector.rs は use で共有。
- cleaner.rs stale doc: PerCleanerFilters の doc を実態に合う 1 実装の記述に置換 (single owner = 各クリーナーの per_cleaner_filters() オーバーライド、library-logs のグローバル older_than_days マージは直交機構である旨も記載)。
- 検証: cargo test --lib 639 passed / 4 failed (4 件は既知の TCC 環境依存、+9 新規テスト)、統合 36 ターゲット green、clippy 警告 0、fmt クリーン。既存テストの変更・削除はゼロ。
