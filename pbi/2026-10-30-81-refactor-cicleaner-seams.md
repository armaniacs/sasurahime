# PBI-81: CliCleaner::clean_with_mode を 2 つの internal seam に分割する

出典: /improve-codebase-architecture ラウンド 2 候補 5（深掘り 2026-10-30）

## ユーザーストーリー
sasurahime の保守担当者として、generic.rs の clean_with_mode（本体 250 行）に同居する「外部ツールが掃除する」と「ツール不在 → detect_dir ツリーの削除」という 2 つの deep 概念が、run_tool と fallback_delete_dir の 2 つの internal seam に分かれることをほしい、なぜなら shallow な 1 関数境界のままでは CLI 実行ブロックに「tool exited non-zero」の帰結が 3 種に drift し続け、次に触る人がどちらの概念が何を必要とするかを型から読めないから。

## 優先度
RICE: Reach 2 × Impact 2 × Confidence 90% / Effort 1.25 人週 = スコア 2.9（本ラウンド 5 位）

- Reach 2: 触るのは generic.rs / cli_fallback.rs / prompt.rs / command.rs の 4 ファイルで、generic.rs 内 3 コピー + cli_fallback print 部の warn 語彙が 1 オーナーに寄る（uv / brew は scope fence で保持）
- Impact 2: 250 行 match 本体の分割と warn 行 1 オーナー化が PBI 77（ledger 迂回）の土台になる。PBI 75 の port 解決（CleanPrompt::for_current_run）を colima サイトだけがバイパスしている状態も解消する
- Confidence 90%: clean_with_mode の呼び出し元は同 impl の clean()（generic.rs:480）のみで private で、既存 pin テストが契約を固定済み。挙動を変えないリファクタとして手順が確立している

## BDD 受け入れシナリオ
Scenario: TTY で clean colima を実行すると confirm が dispatcher で 1 回だけ出る
  Given TTY stdin で colima が PATH にあり confirm_message が設定されている
  When  `clean colima` を実行する
  Then  confirm prompt が dry-run 判定より前に 1 回だけ表示される
  And   ツール実行経路と fallback 削除経路のどちらに進んでも confirm は dispatcher の 1 回だけである

Scenario: 非 TTY で clean colima を実行すると削除到達時に床 gate が拒否する
  Given 非 TTY stdin で colima が PATH になく ~/.colima に VM disk が存在する
  When  `clean colima` を実行する
  Then  confirm prompt は表示されない（confirm_on_tty が None を返し黙って進む）
  And   fallback 削除到達時に床 gate が拒否し exit 0 で "[colima] not a terminal" が出る
  And   ~/.colima と VM disk は生存する

Scenario: 汎用 cache cleaner の CLI が非零 exit で終わると警告 1 行で clean が継続する
  Given bun の CLI が stderr に 1 行出して exit 1 で終わる
  When  `clean bun` を実行する
  Then  warn_tool_failure が警告行 + exit code + stderr リレーを 1 回だけ出す
  And   clean は継続して exit 0 で終わる（fatal でない）

Scenario: warn 行の手書きコピーが generic.rs / cli_fallback.rs / uv.rs に現れない
  Given generic.rs と cli_fallback.rs と uv.rs のソース
  When  source pin テストを実行する
  Then  「exited with code」の warn 行が手書き eprintln としてどのファイルにも現れない
  And   警告文言のオーナーは command.rs の warn_tool_failure 1 箇所のみである

Scenario: detect_dir を持たない cleaner は extras を実行しない
  Given detect_dir を持たない bun / go / pip に extra_commands が設定されている
  When  `clean bun` を実行する
  Then  extras は 1 つも実行されない
  And   extras を走らせるのは detect_dir 付き cleaner のみである（generic.rs:121-124 契約）

## 受け入れ基準
- [ ] clean_with_mode を dispatcher（confirm gate + ツール存在判定 + dry-run print、約 25 行）+ run_tool + fallback_delete_dir に分割し、外部 seam（Cleaner::clean / clean_with_mode のシグネチャ）は不変
- [ ] run_tool(dry_run, detect_dir: Option<&Path>) が no-detect_dir 分岐（498-530）と detect_dir 分岐の実行部（652-735）を 1 実装に統一する（uses_trash を主張しない、非零 exit は warn+継続）
- [ ] fallback_delete_dir(dry_run, detect_dir, reporter, mode) が PBI 69 非 TTY 拒否 gate / TOCTOU 再検証 / seam-truth の CleanResult を運ぶ
- [ ] confirm gate（549-566）は dispatcher に残り、dry-run より前の現行順序（549 > 642）を維持する
- [ ] CleanPrompt::confirm_on_tty を prompt.rs に追加し、dispatcher の confirm gate を port 経由にする（generic.rs:550-566 の手組 2 述語と TtyPrompt 直参照が消える）
- [ ] generic.rs:591 の床 gate は colima 固有ポリシーとして cleaner 側に残し WHY コメントを維持する
- [ ] warn_tool_failure を command.rs に追加し、generic.rs の CLI 実行ブロック 3 コピー（507-521 / 658-669 / 671-703）の warn 部を置き換える。cli_fallback.rs は bail を維持したまま print 部のみ共通化する
- [ ] notice 部（508+522-528 と 656+721-729）を maybe_print_after_clean_notice(success, image_before) に集約する
- [ ] CliFallbackCleaner は統合せず skeleton 共有に留める（cache_dir() の遅延解決と EnvGuard ベースの unsafe-path テスト群を壊さない）
- [ ] generic inline :1738-1768 confirm_prompt_ownership_lives_in_the_port pin を更新する（TtyPrompt.confirm 要求の削除、TTY 解決の不在への反転）
- [ ] colima の confirm_suppressed 時に黙って進む経路の pin を新設し、extras 契約（:121-124）の pin も維持する

## テスト戦略
Outside-In TDD: E2E → 統合 → 単体 の順で書く。

- E2E（最初に契約を固定）: tests/generic.rs の既存 pin が無修正で green。fake-tool E2E、colima prune→fstrim 順序（:457-474）、fstrim 失敗で非 fatal（:477-507）、dry-run 非起動（:242、:316）、not-found matrix（:51-84）、非 TTY refusal（:510-547）
- 統合: 分割後の dispatcher / run_tool / fallback_delete_dir の pin テスト。extras 契約、warn+継続ポリシー、seam-skip の typed TrashRefused（cli_fallback.rs:255-304）、source pin（warn 行の手書きコピー不在）、confirm_suppressed 時に黙って進む経路の新 pin
- 単体（最後に内側で書く）: warn_tool_failure（writer パラメータ化で出力を検査、remedy あり/なし、stderr 空/非空）と confirm_on_tty（非 TTY で None、TUI 抑制中は None）を command.rs / prompt.rs の単体テストで

## 見積もり
1.25 人週（1.0 から 1.5）

内訳: Step 1 warn_tool_failure + 3 コピー置換 0.4 / Step 2 dispatcher + 2 seam 分割 0.5 / Step 3 confirm_on_tty + pin 更新 0.35 = 1.25 人週

## 実装計画

### 問題

src/cleaners/generic.rs:489-738（本体 250 行、唯一の呼び出し元は同 impl の clean()（:480）のみで private）に「外部ツールが掃除する」と「ツール不在 → detect_dir ツリーの削除」という 2 つの deep 概念が同居（shallow な境界）。CLI 実行ブロックは generic.rs 内だけで 3 コピー（507-521 / 658-669 / 671-703 の extras ループ内に 674-688 と 691-700 の 2 型）で、repo 全体では 5 変種（+cli_fallback.rs:142-154 bail、+uv.rs:131-141 log::warn!、+brew.rs:105-113 exit→empty）。「tool exited non-zero」の帰結が 3 種に drift（warn+継続で empty / warn+継続で delta freed / bail で fatal）。

### evidence 補正（正確な問題定義）

「TTY 判定が 2 機構」は正確には 2 ポリシー: generic.rs:550 `!confirm_suppressed() && is_interactive()`（確認可能なら聞く）と generic.rs:591 `confirm_message.is_some() && !is_interactive()`（聞けないなら削除を拒否）。両方とも prompt.rs:36 の同一プリミティブ is_interactive() を読んでいる。問題は判定の重複ではなく「PBI 75 の port 解決（CleanPrompt::for_current_run）を colima サイトだけがバイパスしている」こと。

### 解

1. **clean_with_mode を dispatcher（confirm gate + ツール存在判定 + dry-run print、約 25 行）+ 2 つの internal seam に分割**（外部 seam は不変のまま）:
   - `fn run_tool(&self, dry_run: bool, detect_dir: Option<&Path>) -> Result<CleanResult>` ― 概念 A: 外部ツールが掃除する。削除しない（fs diff で freed 報告のみ）。契約: uses_trash を主張しない / 非零 exit は warn+継続（fatal でない）。no-detect_dir 分岐（498-530）と detect_dir 分岐の実行部（652-735）を 1 実装に統一。違いは Option<&Path> で条件化される delta 計算（704-709）・extras（671-703）・notice（713-729）のみ。mode は不要（uses_trash: false）、reporter も不要（prompt なし、println のみ）― シグネチャが「どちらの概念が何を必要とするか」を型で語る
   - `fn fallback_delete_dir(&self, dry_run: bool, detect_dir: &Path, reporter: &dyn ProgressReporter, mode: TrashMode) -> Result<CleanResult>` ― 概念 B: ツール不在 → detect_dir ツリーの削除。契約: PBI 69 非 TTY 拒否 gate / TOCTOU 再検証 / seam-truth の CleanResult
2. **confirm gate（549-566）は dispatcher に残す**: colima の confirm はツール実行経路と fallback 削除経路の両方をカバーする。fallback 側だけに移すと confirm の意味が半分失われる。dry-run より前に gate が走る現行順序（549 > 642）も維持（TTY の `clean colima --dry-run` は現行でも prompt が出る仕様で、変更は範囲外の挙動変更）
3. **CleanPrompt::confirm_on_tty を prompt.rs に追加（第 3 セマンティクス）**: `pub fn confirm_on_tty(reporter, prompt) -> Result<Option<bool>>` ― TTY が付いており TUI 未抑制のときだけ聞く。None = 聞けなかった（呼び出し側の非 TTY 床 ― PBI 69 の拒否 gate ― が拒否を担う）。colima サイトへの CleanPrompt::for_current_run().confirm() の verbatim 採用は却下（NonInteractive.confirm() が bail するため非 TTY の clean colima が壊れる。colima の意味は「聞けるなら聞く、聞けない場合はここでは黙って進み、削除が到達可能なら床が拒否する」― port が未モデルの第 3 セマンティクス）。generic.rs:550-566 の手組 2 述語 + 直参照 TtyPrompt.confirm が消え、TTY 解決が prompt.rs に 1 箇所化。591 の床 gate は colima 固有ポリシーなので cleaner 側に残し WHY コメント維持
4. **warn_tool_failure ヘルパを command.rs に追加（語彙のみ共有、帰結ポリシーはサイトに残す）**: `warn_tool_failure(display_name, program, args, &output, remedy: Option<&str>)` ― 警告行+exit code、stderr リレー、省略可能 remedy の 1 オーナー。generic.rs の 3 コピーが全部これに置き換わる。write_not_a_terminal_remedy（prompt.rs:44）と同じ writer パラメータ化パターンで単体テスト可能に。「1 関数に帰結まで押し込む」のは却下 ― 3 帰結は実ドメインポリシー（汎用 cache prune の失敗は許容 / cli_fallback は失敗を黙殺すると freed を嘘報告するから bail / uv は fs diff が真実だから log のみ）。帰結を 1 関数に押し込むと shallow な interface が生える
5. **notice 部（508+522-528 と 656+721-729 の image_before 測定 + success-only after_clean_message）は小さな private メソッド maybe_print_after_clean_notice(success, image_before) に集約**
6. **CliFallbackCleaner は統合せず skeleton 共有**: 統合の決定的な反対理由 ― CliCleaner の detect_dir は構築時に eager 解決の Option<PathBuf>、CliFallbackCleaner の cache_dir() は呼び出し時に closure で遅延解決（cli_fallback.rs:57-59）。huggingface/pre_commit の EnvGuard ベースの unsafe-path テスト群はこの遅延解決に依存。共有するのは warn print と fallback 削除 1 関数化（PBI 77 が作る delete_entries 1-call 行）と契約一致

### 実装順

- Step 1: command.rs warn_tool_failure + generic.rs の 3 コピー置換（cli_fallback.rs:142-154 は bail を維持したまま print 部のみ共通化）
- Step 2: clean_with_mode を dispatcher + run_tool + fallback_delete_dir に分割 + confirm_prompt_ownership_lives_in_the_port pin の更新
- Step 3: prompt.rs confirm_on_tty + 単体テスト + dispatcher の confirm gate を port 経由に
- Step 4: 検証ゲート（cargo test --no-fail-fast / clippy -D warnings / fmt --check）

## pin（壊してはいけない契約）

1. **PBI 69 colima 安全床**: generic.rs:591-597 非 TTY 拒否 gate（remedy 行は prompt.rs:44 の 1 オーナー）、E2E tests/generic.rs:510-547（refusal → exit 0 + "[colima] not a terminal" + disk 生存）、src inline :1708-1736、tests/auto_policy.rs:259（auto は colima に触れない）。refusal は empty（exit 0）であること（skipped を付けない）
2. **PBI 75 / prompt port pins**: prompt.rs:242-270 source-pin（generic.rs を含む 8 ファイルの literal 参照検査）― 無修正で green 維持。prompt.rs:206-239 suspension テスト。generic inline :1738-1768 confirm_prompt_ownership_lives_in_the_port は **更新対象**（TtyPrompt.confirm 要求の削除、`!body.contains("dialoguer::")` + TTY 解決の不在への反転 ― pin の意図（cleaner が dialoguer を直呼びしない）はむしろ強化される）。形態 D pin の実体は PBI 69 の拒否 pin（保存）
3. **TOCTOU pin**: is_safe_delete_target 再検証が削除の前（generic.rs:598-610、cli_fallback.rs:169-176）+ unsafe path 生存テスト。validate-then-use（非アトミック）のコメント正直さも保持
4. **既存 generic/cli_fallback テスト = warn+継続ポリシーの pin**: tests/generic.rs 全体（fake-tool E2E、colima prune→fstrim 順序 :457-474、fstrim 失敗で非 fatal :477-507、dry-run 非起動、not-found matrix :51-84）と cli_fallback.rs:255-304（seam-skip は typed TrashRefused を運び trash を主張しない）
5. **confirm gate の順序**: detect_dir 経路で dry-run より前に confirm（テスト未 pin だが変更は範囲外の挙動変更）
6. **extras は detect_dir 付きのみ**（generic.rs:121-124 契約）― run_tool 統一で bun/go/pip などが extras を走らせ始めないこと
7. **CleanOptions.trash_mode 契約**: mode は run 1 回解決で pin。fallback_delete_dir のみ mode を受ける
8. **registry pin**: define_cleaners! 表・cleaner 名・detect() の Unknown 契約（generic.rs:460-467）不変
9. **colima の confirm_suppressed 時に黙って進む経路**: 現状 pin なし → リファクタが壊さない契約として pin を新設する

## scope fence

1. 挙動の変更なし: 出力文言・終了コード・gate 順序・dry-run 時の prompt 有無
2. uv.rs / brew.rs の変種統合なし（uv は log::warn! が PBI 53 根拠コメント付きで「fs diff が真実」ポリシーの正しい表現。eprintln に寄せると出力チャネル・フィルタ挙動が変わる）
3. CliFallbackCleaner の統合・新しい cleaner 名・registry/detect()/Cleaner trait/CleanResult/TrashMode 契約の変更なし
4. TUI（interactive.rs）の変更、hint/ の ledger 外ループ、Linux/Windows 対応なし
5. PBI 77（ledger 迂回）の delete_entries 差し替えは本 PBI に混ぜない（意図的に合流する場合のみ別途決定。本 PBI は fallback_delete_dir を作るまで）
6. CHANGELOG.md は触らない（マージ後に本体で一括記載）

## DoD

- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を除く）
- [ ] cargo clippy -- -D warnings が 0
- [ ] cargo fmt --check が通る
