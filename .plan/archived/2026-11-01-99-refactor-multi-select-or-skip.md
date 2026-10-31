出典: .plan/whywhy-round6-seams.md 収束解 (i) B2 と (iv) B5（ラウンド 6 なぜなぜ分析、2026-10-31、読み取り専用分析・コード変更なし。対象: sasurahime main @ 53f0b88）

# PBI-99: interactive-select の儀式を multi_select_or_skip に集約する

## ユーザーストーリー
sasurahime を保守する開発者として、interactive select の非 TTY 契約（port が非 TTY を解決して None を返す → clean は skip する、空選択 → nothing selected）を 1 つのヘルパ呼び出しに集約できることがほしい、なぜなら現行はこの契約が interface ではなく実装の繰り返しとして 6 箇所に存在し、形状が既に 3 種に分岐しているため、port の None 意味論や remedy 文言が変われば各箇所が各自で追従するか drift し、新規 cleaner 追加時には儀式を写す必要があるから。

## 優先度
RICE: Reach 2 × Impact 1.5 × Confidence 85% / Effort 0.3 人週 = スコア 8.5（PBI 75 契約修正が 1 箇所に集中、7 cleaner × skip テストが port の 1 テストに寄る）

- Reach 2 は interactive select を要する 7 cleaner 分の非 TTY 経路だが、契約修正が port の隣の 1 ヘルパに集中し、7 cleaner 分の skip テストが port の 1 テストに寄る。
- Impact 1.5 は drift 面（None と empty の混同で remedy が二重に出る、空選択で削除照準がずれる）と新規 cleaner の追加コスト（儀式のコピーが「items 文字列と文言だけ」になる）を同時に消す。
- Confidence 85% は port 契約（prompt.rs:116-121）と remedy 文言の単一所有（prompt.rs:44-50）が既に実在するため。動作変更ゼロの寄せで、既存 E2E が守りになる。

## 発見の詳細（なぜなぜ round 6 収束解）

**前提**: この PBI は収束解 (i) B2（interactive_select 儀式のヘルパ集約）と (iv) B5（display_name 一意性 pin 追加）を実装計画に含む。実装は worktree で subagent-driven。並行で進行中の PBI 97 と library_logs.rs・orphan_containers.rs を共有するが別関数領域なので、マージ順は 97 → 99。未実装確認済み: `multi_select_or_skip` は src/ tests/ に存在しない（grep 0 件、2026-11-01 実測）。

**問題**: 「port が非 TTY を解決して None を返す → clean は skip する」「空選択 → nothing selected」の契約が interface ではなく実装の繰り返しとして 6 箇所に存在する:

- 儀式の実在箇所 6 箇所（実測 2026-11-01）: ios_backup.rs:111-117、library_logs.rs:254-260、app_support.rs:585-591（`[{NAME}] nothing selected`、NAME = "app-support" は :222 の const）、library_caches.rs:354-360、apfs_snapshot.rs:283-296（match 形）、ollama.rs:153-163（inline 形、ヘルパなし、PBI-52 コメント :140-146 付き）
- 形状は既に 3 種に分岐: 6/7 が interactive_select ヘルパ（ios_backup.rs:140、apfs_snapshot.rs:366、library_logs.rs:274、app_support.rs:404、library_caches.rs:290、orphan_containers.rs:322）、ollama は inline、apfs_snapshot の clean 側は match 形（`--all` バイパス :283-285）
- port 契約は 1 箇所: `PromptPort::multi_select`（prompt.rs:122-129）は `Result<Option<Vec<usize>>>`。`None` = 非 TTY（port が標準 remedy を 1 回だけ出す）、`Some(vec![])` = 空選択。port doc（prompt.rs:116-121）は「"not a terminal" と "nothing selected" は決して続けて出ない」契約を明文化済み。remedy 文言の単一所有は `write_not_a_terminal_remedy`（prompt.rs:44-50）
- port の None 意味論や remedy 文言が変われば各箇所が各自で追従するか drift する。新規 cleaner 追加時は 3 通りの既存形から選んで儀式を写す必要があり、ollama をコピーすると PBI-52 型の理由コメントごと複製されて違う理由の説明が増殖する
- テストの現状: 「nothing selected」の E2E pin はゼロ（tests/ grep 0 件、実測）。remedy は E2E 3 本（tests/ollama.rs:103「not a terminal - skipping」、tests/app_support.rs:65、tests/apfs_snapshot.rs:50）、per-cleaner 非 TTY 振る舞いテストは library_logs.rs:552-576 の 1 本のみ（7 中 1）

**B5（同 PR で pin 1 本）**: TARGETS（app_support.rs:28-101）と KNOWN_ENTRIES（hint/apps.rs）の join は `hint_entry`（app_support.rs:105-109）の first-match。双方向テスト 3 本（every_target_has_matching_hint_entry :866-877、every_app_support_hint_entry_has_a_target :880-891、targets_cover_every_hint_suffix :894-910、strip_prefix 剥離 panic 付き :906）が「行が無い」を止めるが「同名行が 2 つ」を通す（first-match が別行の process_name/threshold/quit_app を静かに掴む）。AppSupport 基準の display_name 一意性 pin だけが無い。同名行は現在実在せず実害ゼロ ― 予防価値のみ。

**不移行の根拠（orphan_containers）**: 2 arm が genuinely 違う。interactive_select は associated fn（&self なし、:322-343）で items を format_item で内部構築し、Some remedy + 空デフォルト（:335-341）と 2 行の見出し（hint :327 + Put Back 警告 :328-331）が独自。clean 側の empty arm だけ print_unreadable_hint を伴い（:391-394、None arm :388-390 は伴わない）、`--all` も持たない（:383-384）。特殊例を共通形に押し込むとヘルパの契約が濁るため、pin 検索起点（:973-1016）の更新のみをこの PBI が担う。

## BDD 受け入れシナリオ

```gherkin
Scenario: 非 TTY の stdin で interactive clean が走ると何も削除せずスキップする
  Given 移行対象 6 cleaner のいずれかの clean が interactive 経路で走る
  And   stdin が TTY ではない
  When  interactive select に到達する
  Then  port が標準 remedy を 1 回だけ出力して None を返す
  And   clean は skip して何も削除しない
  And   dialoguer のプロンプトで固まらず、remedy は二重に出ない

Scenario: ユーザーが何も選ばないと nothing selected が出てスキップする
  Given 移行対象 6 cleaner のいずれかの clean が interactive 経路で走る
  And   stdin が TTY である
  When  ユーザーが何も選択せず確認する
  Then  "[{name}] nothing selected" が 1 行だけ出力される
  And   clean は skip して何も削除しない
  And   "not a terminal" と "nothing selected" が同時に出ることはない

Scenario: 選択した項目だけが削除照準になる
  Given 移行対象 6 cleaner のいずれかの clean が interactive 経路で走る
  And   stdin が TTY である
  When  ユーザーが項目を選択して確認する
  Then  選択された indices に対応する項目のみが削除照準になる
  And   出力は現行と byte 等価である

Scenario: orphan-containers は現行の明示形を維持する
  Given orphan-containers の clean が interactive select に到達する
  When  非 TTY で clean が走る
  Then  現行の契約（port が remedy を 1 回出して None → clean は skip）が維持される
  And   現行の明示形（associated fn、Some prompt + 空デフォルト、empty arm だけ print_unreadable_hint）が変わらない

Scenario: 既存の pin はすべて現行の出力のまま green する
  Given この PBI の変更がマージされている
  When  既存の E2E 3 本、per-cleaner 非 TTY テスト、port unit test、seam-shape pin を走らせる
  Then  すべて現行の出力のまま green する
  And   Cleaner trait は 11 メソッドのまま変化しない

Scenario: 同名の AppSupport 行を足すと CI が落ちる
  Given KNOWN_ENTRIES に AppSupport 基準の行が登録されている
  When  同じ display_name の AppSupport 行をもう 1 行足す
  Then  display_name 一意性テストが fail して PR が止まる
```

## 受け入れ基準
- [ ] src/prompt.rs に `multi_select_or_skip(name, reporter, prompt, items, defaults) -> Result<Option<Vec<usize>>>` が 1 本追加されている（None = 非 TTY skip、`[{name}] nothing selected` 出力 + None = 空選択 skip、Some(indices) = 続行）
- [ ] Cleaner trait へのメソッド追加がなく 11 メソッドを維持する（multi_select_or_skip は free function）
- [ ] 移行 6 cleaner（ios_backup、library_logs、app_support、library_caches、apfs_snapshot の select 分岐、ollama の inline 置換）がヘルパ呼び出しになり、各 cleaner は自分の items 文字列とプロンプト文言だけを渡している
- [ ] apfs_snapshot は select 分岐のみの移行で `--all` バイパスとその後の防御的 empty check が現行のまま残る
- [ ] orphan_containers は不移行で現行の明示形を維持する（empty arm だけ print_unreadable_hint、Some remedy + 空デフォルト）
- [ ] `[{name}] nothing selected` と remedy 文言がヘルパ/port に単一所有される（cleaner 側の重複出力が消える）
- [ ] 同 PR で pin 更新: prompt.rs:269-298 の include_str list（ヘルパの置き場所を反映）、orphan_containers.rs:973-1016 の検索起点（主張は維持）
- [ ] display_name 一意性テスト 1 本が app_support.rs tests に追加され、同名行 2 行を書くと fail する（表統合・ID 化はしない）
- [ ] 動作変更ゼロ: 出力が現行と byte 等価（E2E 3 本、per-cleaner 非 TTY テスト、port unit test が現行のまま green）
- [ ] PBI 75 契約コメントが消えていない（ヘルパ側に 1 箇所残る）

## テスト戦略
Outside-In TDD で外側から内側へ書く。動作変更ゼロの refactor のため、既存の E2E が守りになり、Red は「新 pin が存在しない」ことと「pin が壊れることの手確認」で立つ。

- Red（新 pin）: 6 cleaner の None/空選択 pin を各 cleaner の tests に先に書く（非 TTY → skip、空選択 → `[{name}] nothing selected` 出力 + skip）。現行の儀式に対して pin が green（振る舞い pin）であることを確認し、ヘルパ unit test が fail（関数が存在しない）ことを確認する。非 TTY テストは library_logs.rs:552-576 の先例（`std::io::stdin().is_terminal()` ガード付き）に合わせ、対話的 cargo test での hang を避ける。
- Red（B5 pin の Red 検証）: `every_app_support_hint_entry_display_names_are_unique` を app_support.rs tests に追加し、KNOWN_ENTRIES の AppSupport 行の display_name を一時的に同名 2 行にすると fail することを手で確認してから戻す（DoD の Red 検証）。
- Green: src/prompt.rs に multi_select_or_skip を実装し、6 cleaner を移行する。Red の pin、既存 E2E 3 本（tests/ollama.rs:103、tests/app_support.rs:65、tests/apfs_snapshot.rs:50）、per-cleaner 非 TTY テスト（library_logs.rs:552-576）、port unit test（prompt.rs:301-330）が現行の出力のまま green になる。
- E2E: tempdir fixture で binary（またはトップレベル関数）を spawn し、出力 byte 等価を確認する（remedy 行と nothing selected 行の文言・位置が現行のまま）。
- 統合: MockRunner で選択 indices → 削除照準の対応を検証する（移行前後で同じ indices が同じ entries に対応する）。
- 単体: multi_select_or_skip の 3 出口（非 TTY → Ok(None)、Some(vec![]) → `[{name}] nothing selected` 出力 + Ok(None)、Some(indices) → Ok(Some)）を Vec<u8>／RecordingReporter で検証する。display_name 一意性テストは KNOWN_ENTRIES の AppSupport 基準行を走査する単体テスト。

## 見積もり
0.3 人週（0.2 から 0.4）

## 実装計画
実装順:

1. src/prompt.rs に `multi_select_or_skip(name, reporter, prompt, items, defaults) -> Result<Option<Vec<usize>>>` を 1 本追加する（None = 非 TTY skip、`[{name}] nothing selected` 出力 + None = 空選択 skip、Some(indices) = 続行）。trait メソッド追加なし、Cleaner trait は 11 メソッド維持。PBI 75 の契約コメントはヘルパ側に 1 箇所残す。
2. 移行 6 cleaner: ios_backup、library_logs、app_support、library_caches、apfs_snapshot の select 分岐、ollama の inline 置換。各 cleaner は自分の items 文字列とプロンプト文言だけを渡す。apfs_snapshot は select 分岐のみで `--all` バイパスとその後の防御的 empty check は現行のまま。
3. orphan_containers は不移行（現行の明示形を維持。empty arm だけ print_unreadable_hint、Some remedy + 空デフォルト等 2 arm が genuinely 違う）。pin 検索起点更新のみ。
4. 同 PR で pin 更新: prompt.rs:269-298 の include_str list、orphan_containers.rs:973-1016 の検索起点（主張「port を通る」「dialoguer を直接呼ばない」は維持、検索対象・起点だけ実装に合わせる）。
5. B5: display_name 一意性テスト 1 本（`every_app_support_hint_entry_display_names_are_unique`）を app_support.rs tests に追加する（表統合・ID 化はしない）。
6. 検証ゲート（DoD の 3 コマンド + Red 検証）。

設計メモ:

- ヘルパの置き場所は src/prompt.rs。None と empty の区別（prompt.rs:116-121）と remedy 文言の単一所有（:44-50）が既にそこにあり、「nothing selected」の文言（E2E pin ゼロ）も remedy と同じ「1 箇所所有」の型に乗る。cleaners/mod.rs に置くと契約が 2 箇所に分裂する。
- trait メソッドにしない。Cleaner trait は実測 11 メソッド（name, detect, clean, is_available, risk, auto_clean_allowed, interactive_gated, quits_running_apps, advisory, per_cleaner_filters, sub_targets）で、interactive select を要するのは 7 cleaner。全 cleaner に interface を強制すると pass-through になる shallow 化。
- app_support.rs:323-332 の show_leftovers は 8 番目の multi_select 席で、契約が port の標準形と違う（`is_interactive` ガードの silent no-op、remedy なし、TtyPrompt 直接）。display-only path の別契約のため本 PBI の対象外 ― この判断の根拠を次に触る者のために残す。
- ollama の PBI-52 コメント（:140-146）は port 契約の説明として 1 箇所に残す。inline 形ではなくヘルパがコピーの対象になることで、理由コメントの複製が構造的に起きなくなる。

## pin（壊してはいけない契約）
- port 契約: `PromptPort::multi_select`（prompt.rs:116-129）の None/Some(vec![]) 意味論。signature 変更なし
- remedy 文言の単一所有: `write_not_a_terminal_remedy`（prompt.rs:44-50）
- call-sites pin（prompt.rs:269-298）: cleaner が literal 参照（&DeepSuppressReporter 等）を port に渡さない。集約後も生きる。include_str list のみ同 PR で更新
- seam-shape pin（orphan_containers.rs:973-1016）: 「port を通る」「dialoguer を直接呼ばない」。主張は維持、検索起点のみ同 PR で更新
- PBI 75 契約: 非 TTY 意味論は port が所有、cleaner は解釈しない。ヘルパ側に契約コメント 1 箇所
- PBI-52 コメント（ollama.rs:140-146）: 複製しない、1 箇所で維持
- 既存テスト: E2E 3 本（tests/ollama.rs:103、tests/app_support.rs:65、tests/apfs_snapshot.rs:50）、per-cleaner 非 TTY テスト（library_logs.rs:552-576）、port unit test（prompt.rs:301-330）は現行のまま
- KNOWN_ENTRIES 22 行の vetted 契約コメント（hint/apps.rs:5-8）と双方向テスト 3 本（app_support.rs:866-910）は現行のまま

## scope fence
1. orphan_containers の集約への押し込みは絶対禁止（2 arm が genuinely 違う）
2. Cleaner trait へのメソッド追加・削除・署名変更はしない（11 メソッド維持）
3. CleanPrompt port（confirm_on_tty/multi_select）の signature 変更はしない
4. remedy 文言の変更はしない
5. "nothing selected" 文言の変更はしない
6. PBI 75 コメントの削除はしない（ヘルパ側に 1 箇所残す）
7. library_logs.rs の delete_entries 領域（:159-160）は触らない（並行で進行中の PBI 97 が触る）。この PBI の編集は interactive_select 領域（:249-260、:274-281）に限る
8. orphan_containers.rs の delete_selected/stale コメント領域（:418-419/:1023）は触らない（PBI 97 が触る）。この PBI の編集は pin 検索起点 :973-1016 に限る
9. KNOWN_ENTRIES・TARGETS の表変更（表統合・ID 列追加）はしない
10. top5/閾値の変更はしない
11. CHANGELOG.md は触らない（マージ後に本体で一括記載）
12. Linux/Windows 対応なし

## DoD
- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 動作変更ゼロ（出力 byte 等価。E2E 3 本・per-cleaner 非 TTY テスト・port unit test が現行の出力のまま）
- [ ] Cleaner trait は 11 メソッド維持
- [ ] display_name 一意性 pin の Red 検証（同名行を一時的に書くと fail することを確認してから戻す）
- [ ] pin 更新が同じ PR に含まれる（prompt.rs:269-298、orphan_containers.rs:973-1016）

## 備考
- 実装は worktree で subagent-driven（AGENTS.md の workflow）。この PBI ファイルは作成のみでコミットしない。
- マージ順は 97 → 99（library_logs.rs・orphan_containers.rs を共有するが別関数領域なので衝突しない）。
- line anchor は 2026-11-01 実測（sasurahime main @ 53f0b88）。実装時は pin と同様に「リネーム・移動時は同じ PR で anchor を更新する」運用に従う。
