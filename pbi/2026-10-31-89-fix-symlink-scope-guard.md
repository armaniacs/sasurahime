# PBI-89: symlink スコープ検査を全削除経路に統一する

出典: なぜなぜ分析「hint 経路と app_support / library_caches の機能重複」（.plan/whywhy-round4-hint-duplication.md、ラウンド 4、2026-10-31、main @ d437647、読み取り専用分析）。分析の Phase 1（ガード統一）が本 PBI の実装計画。Phase 2（排他 pin）と Phase 3（計測・意味合わせ）は別 PBI。

## ユーザーストーリー
sasurahime でキャッシュを掃除する macOS 開発者として、削除対象パスの中間に symlink が仕込まれていても HOME 内の別パスの実体を消されないことがほしい、なぜなら ~/Library/Application Support/Slack を ~/Documents/重要 に向けた symlink が置かれた状態で hint 経路の削除が走ると、今日は seam の scope 検査を通って実体（~/Documents/重要/Cache）ごと消えてしまうから。

## 優先度
RICE: Reach 3 × Impact 3 × Confidence 90% / Effort 0.5 人週 = スコア 16.2

- Reach 3: 全削除経路が通る唯一の seam（trash.rs::safe_delete）の利用者。`clean hints`、scan の hint offer、TUI、`clean all` / auto、library-caches と全 PathCleaner の削除が対象。
- Impact 3: ユーザーデータ（~/Documents など HOME 内の別パス）の意図しない削除。攻撃例は dir_size の計上を通り threshold を越えて削除に到達し、canonicalize 後の宛先が HOME 内なのでブロックリストにも掛からない。
- Confidence 90%: per-component 検査の実装が app_support.rs に実在し（resolve_allowed、137-170 行）、safety.rs への切り出しと 2 call site への配線で済む。seam の scope tripwire には既存 pin（src/trash.rs tests の refuses_home_itself / refuses_filesystem_root / refuses_etc）がある。
- 前提: PBI-80（hint 削除の ledger 移行）と PBI-83（hints cleaner 登録）の着地後。なぜなぜ分析の裁定で、完全解消（3 cleaner 統合）は不採用。本 PBI は Phase 1 のみ。

## 背景と問題（実コード裏取り済み、main @ d437647）

1. 検査の非対称: HOME 内の中間 symlink を越える scope 拡大を構造的に拾える per-component 検査（`fs::symlink_metadata` でコンポーネントを順に検査し symlink を即座に拒否）が、src/cleaners/app_support.rs の resolve_allowed（137-170 行、symlink 拒否は 149-163 行）にだけ局所実装されている。hint 経路は auto_clean_hint の `path.exists()` のみ（src/hint/cleaner.rs:138。exists() は symlink を辿って存在を判定するため、攻撃に対して何も検出しない）、library-caches と他の PathCleaner は共有 seam での中間 symlink 検査を持たない。共有 seam の安全網 is_safe_delete_target（src/safety.rs:14-81）は「canonicalize した後の root ブロックリスト照合」しかしない。
2. 攻撃例: ~/Library/Application Support/Slack を ~/Documents/重要 に向けた symlink + 中間 symlink + 実在リーフ（Slack/Cache）の構成では、collect_hints の dir_size（src/hint/detector.rs:23-27。walkdir は root 自体の symlink は辿らないが、中間 symlink はパス解決で解決するため実体が計上される）が threshold（64MB）を越え、auto_clean_hint が `path.exists() = true` で delete_entries("hints") → safe_delete に進む。canonicalize が symlink を解決した宛先は HOME 内（~/Documents/...）なので、HOME/祖先ガード（safety.rs:32-40）にも root ブロックリスト（safety.rs:46-81）にも掛からず、削除は通る。同じ構成を app-support 経路に対して行うと resolve_allowed が最初の symlink コンポーネントで `GuardError::Symlink` を返して落ちる。
3. 攻撃面の残存: 同一パスに 2 経路（hint と cleaner）が存在する限り、弱い経路（hint 側）から攻撃面が残る。完全解消（3 cleaner 統合）は top5 pin・1 cleaner = 1 name pin・risk 分担・TUI shape を破壊するため不採用（なぜなぜ分析の裁定）なので、ガードの統一で弱い経路を塞ぐのが中間解。
4. 維持でよい部分: 同一実行内の後続 cleaner は clean 時に再スキャンするため二重削除は原則起きない。残る窓は (i) 各 clean 内の scan → delete 間、(ii) 複数プロセス同時実行だが、どちらも seam の分類（NotFound → permanent は fatal で prose skip / TrashRefused → trash は skippable）で閉じられている。この分類は本 PBI で変更しない（維持）。

## BDD 受け入れシナリオ

```gherkin
Scenario: 中間 symlink を越えた削除は fail closed で拒否される
  Given ~/Library/Application Support/Slack が ~/Documents/重要 への symlink として置かれている
  And   実体側（~/Documents/重要/Cache）に threshold を越える残骸入りの実在ディレクトリがある
  When  hint 経路の削除が走る
  Then  symlink スコープ検査が中間 symlink を検出して削除を拒否する
  And   実体（~/Documents/重要/Cache）のファイルは 1 つも変わらない

Scenario: dir_size は中間 symlink と実在リーフの構成で実体を計上する
  Given 中間 symlink + 実在リーフ（Slack/Cache）の構成で残骸が置かれている
  When  dir_size の計測（collect_hints と同 metric）が走る
  Then  実体のバイト数が計上され threshold を越えて削除フローに到達する
  And   それでも削除は拒否される（計上が通っても削除は fail closed）

Scenario: 正常なパスは従来どおり削除される
  Given symlink を含まない通常の hint 候補（Slack/Cache など）に残骸がある
  When  hint 経路の削除が走る
  Then  削除は従来どおり成功し、freed と履歴行は今日と同じ契約で記録される
```

## 受け入れ基準
- [ ] per-component symlink 検査ヘルパが src/safety.rs に切り出され、fail closed（検査不能な場合は拒否）の意味論を持つ
- [ ] src/trash.rs::safe_delete の scope tripwire からヘルパが呼ばれ、中間 symlink を含むパスの削除が拒否される（既存の scope tripwire と同じ warn + bail の分類）
- [ ] src/hint/cleaner.rs::auto_clean_hint の `path.exists()` 確認がヘルパに置換され、symlink を含む候補は ledger に載らない
- [ ] src/cleaners/app_support.rs の resolve_allowed はヘルパ利用に寄せるか、現状維持（二重検査は fail closed なので無害）のどちらか。既存 guard テスト群（resolve_rejects_symlinked_root / resolve_rejects_symlinked_leaf / resolve_rejects_symlinked_middle_component / scan_excludes_symlinked_cache_dir など）が無修正で green
- [ ] symlink 攻撃の E2E pin が追加され、Red（現在の実装は削除を通す）→ Green（拒否される）の反転を確認済み
- [ ] dir_size 計上の pin: 中間 symlink + 実在リーフの構成で dir_size が実体を計上することを pin する
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy --tests -- -D warnings が 0、cargo fmt --check が通る
- [ ] hint の prompt/dry-run 表示は無変更（別 PBI 91 の管轄）

## テスト戦略
Outside-In TDD。E2E で Red を確認してから、内側へ向かって Red → Green → Refactor を繰り返す。

- E2E: tests/trash.rs または tests/hints.rs に symlink 攻撃 pin 1 本。tempdir HOME に Library/Application Support/Slack を Documents/重要 への symlink として作り、実体側に Cache を残骸入りの実在ディレクトリで作る。削除実行後に (a) dir_size が実体を計上したこと（collect_hints と同 metric の呼び出しで確認）、(b) 実体のファイルが生存すること、を検証する。Red は「現在の実装が削除を通し、実体が消える」ことで実証する。
- 統合: src/trash.rs tests に中間 symlink 拒否の pin。既存の scope tripwire 3 本（safe_delete_refuses_home_itself、safe_delete_refuses_filesystem_root、safe_delete_refuses_etc）と同型で、中間 symlink を含む tempdir パスが拒否されることを pin する。
- 単体: safety.rs に切り出したヘルパの検査（symlink コンポーネントの拒否、正常パスの許可、検査不能時の fail closed）。app_support.rs の既存 guard テストは無修正で green が pin（移動/寄せの回帰検知）。
- モック: 外部コマンドは CommandRunner 注入（pbi/README.md の設計方針）。削除の fs 操作は tempdir で駆動し、実 Trash 操作は既存テストと同じ backend override で固定する。

## 見積もり
0.5 人週
内訳: ヘルパの切り出しと 2 call site への配線 0.2 / E2E + 統合 + 単体の pin 追加 0.2 / Red 再現と検証ゲート 0.1

## 実装計画

### 実装順
1. E2E Red: symlink 攻撃 pin を追加する（中間 symlink + 実在リーフ + 実体生存 assert）。現在の実装が削除を通すことを実証する。
2. 統合 Red: src/trash.rs tests に中間 symlink 拒否の pin を追加する。実装前に落ちることを確認する。
3. Green: per-component symlink 検査ヘルパを src/safety.rs に切り出す（app_support.rs::resolve_allowed の実装を移動、fail closed）。
4. Green: src/trash.rs::safe_delete の scope tripwire からヘルパを呼ぶ。全削除経路が通る唯一の seam。app_support の resolve_allowed はヘルパ利用に寄せるか現状維持（二重検査は fail closed なので無害。dedup するかは実装時に design-it-twice で決める）。
5. Green: src/hint/cleaner.rs::auto_clean_hint の `path.exists()` 確認をヘルパに置換する。dir_size の計上条件と metric は変更しない。
6. 検証ゲート（DoD の 3 コマンド）と Red → Green 反転の確認。

### 対象ファイル
src/safety.rs（ヘルパ本体 + 単体テスト）、src/trash.rs（scope tripwire の呼び出し + 統合 pin）、src/hint/cleaner.rs（exists 確認の置換のみ）、src/cleaners/app_support.rs（resolve_allowed の移動または寄せ。dedup しない場合は無変更）、tests/trash.rs または tests/hints.rs（E2E pin 1 本）。

### 期待する Red → Green
E2E pin と統合 pin が先に Red（現在の実装は中間 symlink 攻撃の削除を通し、HOME 内の実体が消える）。実装後に全 Green。既存テスト（app_support guard 群、trash の scope tripwire 3 本、hint 契約）は無修正で green が pin（回帰の検知）。

### design-it-twice
- Option A（採用）: safety.rs にヘルパを切り出し、seam から呼ぶ。全削除経路が通る唯一の場所に 1 実装で効き、safety.rs のモジュール doc（is_safe_delete_target と resolve_env_dir を切り出した理由）と同じ構え。app_support は移動元として同一理由（削除安全）で変更される。
- Option B（却下）: 各 cleaner（hint、library-caches など）に個別の検査を追加。経路ごとのコピーが増え、次の経路追加で drift が復活する（本 PBI の原因そのもの）。
- Option C（却下）: 3 cleaner 統合による完全解消（対象集合の単一真実化）。top5 pin、1 cleaner = 1 name pin、risk 分担、TUI shape を同時に破壊するため、なぜなぜ分析の裁定で不採用。
- ヘルパのシグネチャ（base を取るか path 単独か、リーフコンポーネントの扱い）は Negotiable。検査範囲は「信頼できる base より下のコンポーネント」を resolve_allowed と同型にする（後述の落とし穴）。

### 落とし穴
- macOS では /var、/tmp、/etc が /private/* への symlink のため、パス全体を root から全コンポーネント検査すると tempdir（/var/folders/...）を含むあらゆる削除が拒否され、tempdir を使うテストが全滅する。検査は「信頼できる base より下のコンポーネント」（hint は base_path(home, entry.base_dir) より下、seam は HOME より下）に限定する。HOME 自体の symlink は既存の canonicalize ガード（safety.rs tests の is_safe_delete_target_canonicalizes_home）が扱う領域で、本 PBI の対象と混同しない。
- exists() の置換で dir_size の計上条件と metric を変えない。計上の pin と削除拒否は別の断言で、置換で計上が消えると E2E pin が別の理由で落ちる。
- safe_delete の拒否は既存の scope tripwire と同じ分類（log::warn + bail）にする。新しいエラー型を足すと DeleteError 分類と ledger 契約に触れる（scope fence）。
- walkdir の root 判定と中間 symlink の差（root 自体の symlink は辿らないが中間は解決される）を理解してから pin を書く。root 自体が symlink の構成では dir_size が 0 になり threshold を越えないため、E2E pin は「中間 symlink + 実在リーフ」の構成で作る。

## pin（壊してはいけない契約）
- trash.rs の scope tripwire pin: safe_delete_refuses_home_itself、safe_delete_refuses_filesystem_root、safe_delete_refuses_etc が無修正で green。safe_delete は pub(crate) のまま（the_ledger_users_may_not_call_safe_delete_directly が pin）
- app_support.rs の既存 guard テスト群: resolve_rejects_symlinked_root、resolve_rejects_symlinked_leaf、resolve_rejects_symlinked_middle_component、scan_excludes_symlinked_cache_dir、apply_does_not_follow_symlinks_inside_cache、apply_rejects_dir_replaced_by_symlink_after_scan が無修正で green
- hint 契約: PBI-80 の freed 再計測（auto_clean_hint が dir_size metric で削除時再計測し、freed と tip を一致させる）、PBI-70 の ledger 契約（1 seam 呼び出し = 1 履歴行）、quit/relaunch 経路
- 削除 seam の分類: NotFound → permanent は fatal で prose skip / TrashRefused → trash は skippable、の現行分類は無変更
- KNOWN_ENTRIES（src/hint/apps.rs:6-255）・TARGETS・EXCLUDED の表は無変更。top5 truncate（src/hint/detector.rs:44）と threshold は触れない
- pbi/README.md の設計方針（dry-run 全クリーナー / Cleaner trait 共通 / CommandRunner モック / Outside-In TDD / macOS 専用）

## scope fence（絶対禁止）
1. fail-closed 意味論の弱体化はしない（検査不能時の skip 化、symlink の許可、warn only 化など）
2. 削除経路のその他の変更はしない（delete_entries、seam_delete、delete_ledger、DeleteError 分類、DeleteCtx、chflags 経路）
3. KNOWN_ENTRIES の表変更・top5 truncate・threshold の変更はしない。library_caches の EXCLUDED も触れない
4. explorer.rs には接触しない
5. 3 cleaner 統合（なぜなぜ分析の裁定で不採用とされた完全解消）はしない。Phase 2 の排他 pin と Phase 3 の計測・意味合わせも別 PBI
6. hint/cleaner.rs の prompt/dry-run 表示変更はしない（別 PBI 91 の管轄）。auto_clean_hint への変更は exists 確認の置換に限定する
7. CHANGELOG.md は触らない（マージ後に本体で一括記載）
8. Linux / Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] symlink 攻撃 E2E pin が Red → Green 反転を確認済み（実装前の Red 実行記録つき）
- [ ] BDD 受け入れシナリオ 3 本が自動テストとして実装され、パスする
- [ ] pin の列挙がすべて無修正で green していることを確認する

## 備考
- 実装は worktree で subagent-driven（AGENTS.md の実装ワークフロー）。ブランチ案: fix/symlink-scope-guard
- PBI 91 と src/hint/cleaner.rs を共有するが別関数領域（本 PBI は auto_clean_hint の exists 確認のみ、91 は prompt/dry-run 表示）。マージ順は 89 → 91
