出所: holistic-code-improvement round 9（pbi/2026-11-20-00-backlog-holistic-round9.md の RICE 採点表 1 位、候補 1「scan-cache flush 時の期限切れ破棄 + README/Privacy 同期」）+ なぜなぜ分析 round 9（.plan/whywhy-round9-scan-cache-growth.md、3 立場 × 30 段超）。実測アンカーは main @ c107011。

# PBI 2026-11-20-01: scan-cache エントリの flush 時 age 破棄（prune）

- 種別: fix
- 規模: S（src/scan_cache.rs 1 箇所 + README 2〜4 行 + テスト 2 本）
- 優先度: round 9 中 1 位
- RICE: 3.0 = Reach 3 × Impact 1 × Confidence 1.0 / Effort 1 SP
  - Reach 3: scan_cache.rs への関与は四半期頻度
  - Impact 1: 中（構造的リソース規範の解消・実害はゆっくり）
  - Confidence 1.0: コードで確定
- 依存: なし（Batch 1 で 2026-11-20-04 と並列可。依存マップ参照）

## ユーザーストーリー

sasurahime を日常の掃除に使う macOS 開発者として、掃除ツール自身のデータディレクトリ（~/.local/share/sasurahime/）が掃除のたびに太らないことがほしい、なぜならツールが消したはずの旧バージョンディレクトリ（旧ブラウザビルド、mise の旧ランタイム、作り替えられた node_modules 先など）のパスが ghost エントリとして scan-cache.json に溜まり続け、「掃除屋が自分のゴミを溜める」という構図がディスク容量の視点で実在するから。

## 背景/問題

永続スキャンキャッシュには破棄の工程が実在しない。実測（いずれも main @ c107011 のコードで確認済み）:

1. **insert のみで破棄が無い**: `record()` は `self.entries.insert(...)` のみ（src/scan_cache.rs:66-78）。ファイル内に `remove` / `retain` の呼び出しは 0 件。
2. **lookup 時の無視が唯一の「期限」**: `cached_size()` は `scanned_at` が `MAX_AGE_SECS`（= 3600、src/scan_cache.rs:14）を越えたら None を返すが、エントリは map に残り続ける（src/scan_cache.rs:53-64）。期限判定はヒット判定であって在庫管理ではない。
3. **flush は全件保存**: `flush()` → `save()` が `serde_json::to_string_pretty(self)` で map 全体を直列化して上書きする（src/scan_cache.rs:130-135、:47-49）。保存前に古いエントリを落とす工程が契約に入っていない。
4. **保存先**: `~/.local/share/sasurahime/scan-cache.json`（src/main.rs:1314 + src/history.rs:226-228）。
5. **姉妹データは上限を切る**: 削除履歴は `MAX_HISTORY_ENTRIES: usize = 1000` で `entries.truncate(...)` される（src/history.rs:9、:112）。「永続データは上限を切る」という先行規約がリポジトリに実在し、scan-cache だけが漏れた。
6. **削除経路はキャッシュを参照しない**: モジュール doc が「Deletion paths never consult the cache」と明記（src/scan_cache.rs:7-10）。stale サイズが削除判断に流れる筋は実在しない。
7. **永続化の来歴**: 永続化は PBI 2026-10-23-01（実装 44b6c4d）。永続化以前はプロセス終了が暗黙の sweep だったが、永続化時にセッション内キャッシュの意味論を持ち込んだまま生存期間だけ無期限になり、ディスク上の容量という軸が要件から漏れた。
8. **現行テスト**: src/scan_cache.rs:164-284 に record / invalidation / corrupt / roundtrip 系 8 本。在庫（内容量）についての pin は無い。

なぜ問題か: 消滅済みパスのエントリは lookup される機会自体が来ない（lookup は「今存在するディレクトリ」に対してしか呼ばれない）ため期限切れ判定の機会がなく、ghost として flush のたびにディスクに書き戻される。scan-cache.json はこのツールの本来の仕事（削除）の回数に比例して単調増加する。1 エントリは JSON pretty print で約 150 byte と緩やかだが、これはバグではなく設計漏れであり、放置するほど「cache は小さい」という修正時の前提が崩れる。ディスク掃除ツール自身が無上限のファイルを持つ構造は、crates.io 公開・Homebrew tap 配布のツールとして信頼に直接当たる。

根本原因（なぜなぜ round 9 裁定）: 永続化 PBI（2026-10-23-01）がセッション内キャッシュの意味論（プロセス終了 = 暗黙 sweep）を持ち込んだまま生存期間だけを無期限に延長し、history の MAX_HISTORY_ENTRIES=1000（src/history.rs:9、:112）が示す「永続データは上限を切る」規約から scan-cache だけが漏れた。

## BDD 受け入れシナリオ

```gherkin
Scenario: flush 時に 1 時間を越えたエントリが破棄され、保存後に load すると消えている
  Given 期限切れエントリ（scanned_at + MAX_AGE_SECS < now）と新鮮なエントリが 1 件ずつ ScanCache の map に入っている
  When  prune を実行して save し、同じパスから load し直す
  Then  期限切れエントリのキーは load 後の map に存在しない
  And   期限切れエントリのパスは保存された JSON ファイル内に出現しない
  And   新鮮なエントリは load 後の cached_size で元のサイズを返す

Scenario: 新鮮なエントリは flush を跨いで生存する
  Given 新鮮なエントリ（scanned_at が now に近い）が 1 件 ScanCache の map に入っている
  When  prune を実行して save し、同じパスから load し直す
  Then  load 後の cached_size は元の値を返す（save_then_load_roundtrips の往復 pin が無傷）
  And   map のエントリ数は prune 前後で変わらない
```

## 受け入れ基準

- [ ] prune は `ScanCache` の純粋メソッドとして実装される（I/O を含まない map 操作。グローバル `SCAN_CACHE` / `init` / `flush` の署名は変更しない。flush 内部から呼ばれる配置は実装者が裁定し、呼び出し契約は不変）
- [ ] 破棄条件は `scanned_at + MAX_AGE_SECS < now`（既存の期限定数 MAX_AGE_SECS（src/scan_cache.rs:14）と `cached_size()` の判定（:56-58）を再利用。新設定項目・新環境変数を追加しない）
- [ ] 新鮮エントリの挙動は byte 不変（既存 8 テストは無修正で green。prune の Red→Green pin 2 本を追加: 期限切れのみ捨てる・新鮮は残る）
- [ ] prune のテストは `ScanCache` 構造体レベルで完結する（グローバル OnceLock `SCAN_CACHE` に触れない。save_then_load_roundtrips（src/scan_cache.rs:254-267）の先例に倣う）
- [ ] README の scan cache 言及箇所（英語セクション scan-cache.json の項、README.md:174-178 付近 + 日本語セクション README.md:418-422 付近）に保持期間（エントリは 1 時間の age 期限で flush 時に prune される）を 1〜2 行追記する
- [ ] `--no-cache` / `flush` の呼び出し契約は 1 行も変わらない（src/main.rs:1312-1317 の init 条件と flush 呼び出しは無修正）

## テスト戦略（Outside-In TDD）

Red: 構造体レベルの unit pin 2 本 → Green: prune メソッドで閉じる。

- 単体 Red（2 本）: src/scan_cache.rs tests に (1) 期限切れ + 新鮮の混在 map で prune → 期限切れのみ消える pin、(2) 新鮮エントリのみの map で prune → 何も消えない pin を追加。prune が未実装のため Red。テストは `ScanCache::default()` から始め、期限切れは既存テスト `expired_entries_are_ignored`（src/scan_cache.rs:223-233）と同じ手順（entries への直接書き換え）で作る。グローバル `SCAN_CACHE` には触れない
- 単体 Green: `ScanCache::prune(&mut self, now: u64)` 純粋メソッド（retire 条件は `scanned_at + MAX_AGE_SECS < now`）を実装し、Red 2 本を閉じる
- roundtrip 拡張: save_then_load_roundtrips（:254-267）を消さず、その隣に「prune → save → load 往復で新鮮エントリが byte 等価で生きる」pin を置く（BDD2 の担保）。期限切れエントリが保存 JSON に現れない pin（BDD1 の Then）も同じ層で置く
- 回帰の検知: 既存 8 テスト（src/scan_cache.rs:164-284）は無修正で green が pin。新鮮エントリの lookup・record・invalidate・corrupt フォールバックが 1 行も変わらないことを既存テストが担保する
- E2E: flush 経路（グローバル）の自動テストは新設しない（OnceLock のテストリセット脆弱点が実在するため、本 PBI の pin は構造体レベルに留める。flush からの呼び出しは署名不変の統合で担保）

## 見積もり

1 SP（prune 純粋メソッド 1 個 + flush 内 1 行 + pin 2〜3 本 + README 2〜4 行）

## DoD

- [ ] cargo test が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。一覧は PBI-101 の DoD と同一、実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] README 同期: 英語・日本語両セクションの scan cache 言及に保持期間 1〜2 行が追記済み
- [ ] PBI アーカイブ: `git mv` で .plan/archived/ へ + .plan/00-INDEX.md 更新（統合側が実施）

## scope 外（採らない裁定・記録）

- **サイズ上限 cap（YAGNI）**: age sweep で在庫は自然に収束する（期限切れエントリは lookup でも None を返す無価値な在庫で、破棄しても観測可能な挙動が 1 つも変わらない）。エントリ数・ファイルサイズ上限は設けない
- **パス匿名化**: lookup key として絶対パスそのものが必要（hash 化は mtime 照合の key 機能を壊す）。将来の Privacy 論点として別途判断する
- **doctor への表示**: 在庫観測（エントリ数・ファイルサイズ）の可視化は新コマンド面。本 PBI では実装しない
- **load 時 sweep**: flush 時 sweep と重複（flush が走らない run は保存もしないため在庫は増えない）。手で肥大化させたファイルは corrupt フォールバック（src/scan_cache.rs:35-40）の安全網付きで手動削除可能
- **`let _ = SCAN_CACHE.set(...)` のテストリセット脆弱点**（src/scan_cache.rs:273、OnceLock::set の 2 回目以降の沈黙）: 本 PBI の scope 外。test-infra の変更理由なので、台帳送りに記録済み（pbi/2026-11-20-00-backlog-holistic-round9.md「台帳送り」）。scan_cache のテスト基盤に触る機会が来たとき、context.rs の `with_test_lock()` 型ガードへ寄せる判断をする

## 備考

- 触れるファイル: src/scan_cache.rs（prune メソッド + flush 内呼び出し + tests）、README.md（英語・日本語の scan cache 言及各 1〜2 行）
- pin（壊してはいけない契約）: `Deletion paths never consult the cache`（src/scan_cache.rs:7-10 の doc 契約）、`MAX_AGE_SECS = 60 * 60`（:14）と `cached_size()` の期限判定（:56-58）、`record_if_unchanged` の mid-walk 防御（:84-95）、save の best-effort 静黙契約（:42-50）、history 側の MAX_HISTORY_ENTRIES=1000（src/history.rs:9、:112）は消費のみ・変更しない
- 挙動不変の根拠: 期限切れエントリは lookup で None を返すため、破棄はヒット率に対して挙動不変（なぜなぜ round 9、1.a 30）。prune 後も scan の出力・削除の判断が変わる筋は実在しない
