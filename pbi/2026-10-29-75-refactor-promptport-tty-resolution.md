# PBI: PromptPort が非 TTY を自身の状態として解決する

## ユーザーストーリー
sasurahime の保守担当者として、「gate_interactive を先に呼べ」という interface の外の順序制約が消え、非 TTY 解決が port の implementation に移ることがほしい、なぜなら順序制約の根拠が doc comment だけで、呼ばないと dialoguer が不透明に失敗するから。

## 優先度
- 順位: 7 / 8
- RICE スコア: 2.4（Reach=3 / Impact=1.5 / Confidence=80% / Effort=1.5 人週）
- 根拠: 順序制約の消滅と死に adapter（NonInteractive）の回収。PBI 69 が generic.rs colima サイトの安全側の意味を先に決定してから着手（colima サイトは本 PBI の scope 外に維持）。

## 発見の詳細（深掘り結果）
- gate 分岐は実測 10 箇所・4 形態。port に回収できるのは形態 A（skip + remedy）の 7 箇所のみ: ios_backup.rs:105、library_logs.rs:258、apfs_snapshot.rs:240（!self.all 条件付き）、library_caches.rs:302、ollama.rs:154、orphan_containers.rs:484、app_support.rs:574
- 形態 B（app_support.rs:306 無音 no-op、表示のみ経路）/ C（apfs_snapshot.rs:404 cancelled）/ D（generic.rs:557-574 colima、PBI 69 が安全床を修正）は call site 残置
- 現行 trait は Result<Vec<usize>> で「空選択」は正当な結果。非 TTY を Ok(vec![]) にすると「not a terminal」と「nothing selected」が連続出力されるため Result<Option<Vec<usize>>> へのシグネチャ変更が必須（multi_select 8 サイトに波及）
- confirm 4 サイトは全て TTY-only で到達不能（B/C/D の guard が先にある）ため Result<bool> 維持が可能
- remedy 行には cleaner 名が要る（prompt.rs:46-49）ため trait に name パラメータを追加（design-it-twice で推奨選択）
- source-pin テスト（prompt.rs:207-236）は 6 ファイルの唯一の pin なので保持

## BDD受け入れシナリオ
Scenario: 非 TTY で multi_select が unavailable を返す
  Given 非 TTY stdin で ollama がモデルを持つ
  When  `clean ollama` を実行する
  Then  標準 remedy が 1 行だけ出る（"nothing selected" は出ない）
  And   何も削除されず exit 0

Scenario: TTY では従来どおり prompt が出る
  Given TTY stdin
  When  `clean ollama` を実行する
  Then  既存 E2E（tests/ollama.rs:75-110）が無修正で green

Scenario: PBI 69 後の colima 安全床が維持される
  Given 非 TTY stdin（PBI 69 適用後）
  When  colima を含む掃除を実行する
  Then  colima は非 TTY で拒否される（PBI 69 の意味、generic.rs サイトは call site 残置）

## 受け入れ基準
- [ ] multi_select を Result<Option<Vec<usize>>> + name パラメータに変更
- [ ] NonInteractive 契約設計: multi_select は remedy を stderr に 1 回出し Ok(None)。confirm は Result<bool> 維持（TTY-only、非 TTY は Err と doc）
- [ ] CleanPrompt::for_current_run()（ambient 解決、IsTerminal を読む、threading 不要）
- [ ] 7 gate 呼び出しを port 直呼び + None arm に置換し gate_interactive を削除
- [ ] 形態 B/C/D は現行維持 + pin テストで固定（B: app_support.rs:306、C: apfs_snapshot.rs:404、D: PBI 69 後の colima）
- [ ] library-logs は all=true で無掃除プロンプトなしに掃除する pin（PBI 74 の前提、R-3）
- [ ] prompt.rs:207-236 source-pin と interactive.rs の 3 テストは保持
- [ ] cargo test --no-fail-fast / clippy -D warnings 0 / fmt --check

## テスト戦略
- E2E: tests/ollama.rs の既存 E2E が無修正で green（文言完全維持）
- 統合: pin テスト 6 本（colima、thin cancelled、app-support leftovers、library-logs all、unavailable + remedy 1 回、TTY prompt）
- 単体: prompt.rs の suspension テストが生存

## 見積もり
1.5 人週（prompt.rs 契約変更 0.5、7 gate 廃止 + 8 multi_select サイト 0.5、pin テスト 6 本 + E2E 再確認 0.5）

## Definition of Done
- [ ] 全 BDD シナリオが自動テストとして実装されパスする
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み（prompt.rs の doc、CHANGELOG の remedy 位置変更）
