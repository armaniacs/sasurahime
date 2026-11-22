出所: テスト手法レビュー(2026-11-21、アンカー main @ 0481291)で `cargo test --lib` 3 回中 1 回再現した実フレークの恒久対応。フェーズ 0 の既実装確認済み(2026-11-21 時点で thread_local 化は未実装)。

# PBI 2026-11-21-06: trash テストオーバーライドを thread ローカルにしてレースを根絶する

## ユーザーストーリー

保守担当者として、trash バックエンドのテストオーバーライドが他のテストに漏れ出ない挙動がほしい、なぜなら並列実行のタイミングで DeleteCtx trash テストが偶発 red になるフレークを防ぎ、赤は本当に壊れた時だけ出る状態にするから。

## ビジネス価値

- **価値**: 並列 `cargo test` の信頼性。オーバーライド利用テストが 1 本増えるたびに広がるフレーク面積を、フラグのスコープ変更で根絶する
- **測定方法**: `cargo test --lib trash` を連続 10 回実行して全 green。`cargo test --lib` 全体を 10 回実行して `delete_ctx_trash_mode_moves_to_trash` の偶発 red が 0 回

## 実測の事実(2026-11-21 レビュー)

- `cargo test --lib` 3 回中 1 回、`trash::tests::delete_ctx_trash_mode_moves_to_trash` が `TrashRefused("simulated refusing backend (test override)")` で偶発 red
- 原因: `TRASH_BACKEND_OVERRIDE` がプロセスグローバル(`static AtomicBool`, src/trash.rs:74)。`DeleteCtx::delete_path` は Trash モードでこのフラグを読む(src/trash.rs:49)が、`delete_ctx_trash_mode_moves_to_trash`(src/trash.rs:390)は TEST_LOCK を持たずに走る。ガード保持テスト(src/trash.rs:801 / src/cli_fallback.rs:406 / src/cleaners/mise.rs:864)が並列で走ると、ロックフリーテストが `true` を読む
- src/trash.rs:386-387 のコメント「no global, no TEST_LOCK, safe to run fully in parallel」は、delete_path がグローバルを読むため誤り

## BDD 受け入れシナリオ

```gherkin
Scenario: 隣のスレッドのオーバーライドは見えない(ハッピーパス)
  Given 別スレッドのテストが TrashBackendOverrideGuard をインストールして保持している
  When  このスレッドで DeleteCtx(Trash) の delete_path を呼ぶ
  Then  実バックエンドで削除が成功し、TrashRefused にならない

Scenario: オーバーライド自身の契約は変わらない(エラー/境界ケース)
  Given このスレッドで TrashBackendOverrideGuard がインストールされている
  When  同一スレッドで DeleteCtx(Trash) の delete_path を呼ぶ
  Then  TrashRefused が返る(現行契約どおり)
```

## 受け入れ基準

- [x] `TRASH_BACKEND_OVERRIDE` を `thread_local!`(`Cell<bool>`)に変更し、読み書きを同一スレッドに限定する
- [x] `TrashBackendOverrideGuard` は現行どおり TEST_LOCK を保持する(他の TEST_LOCK 利用者との既存の直列化を変えない)
- [x] 下記テスト戦略の決定的 red-first テストが、変更前のコードで red、変更後は green であることを確認する
- [x] オーバーライド利用 3 箇所(trash.rs:801 / cli_fallback.rs:406 / mise.rs:864)は無修正で green
- [x] src/trash.rs:386-387 の誤ったコメントを事実に合わせて修正する(自スレッドのオーバーライドのみを見る旨)
- [x] ソースピン `delete_ctx_delete_path_selects_the_nsfilemanager_backend`(src/trash.rs:683)が無修正で green する
- [x] 本番コード(`#[cfg(test)]` 外)は 1 行も変更しない

## テスト戦略(t_wada スタイル)

### E2Eテスト
- なし(本 PBI は `#[cfg(test)]` 内に閉じた変更のため)

### 統合テスト
- スレッド分離の決定的 pin(trash.rs の tests モジュール内):
  1. 子スレッドでガードを set し、チャネルで「インストール済み」を通知する
  2. 親スレッドは通知を受けてから `DeleteCtx::new(TrashMode::Trash)` の delete_path で実削除を行い、成功を主張する(現行のグローバル実装ならここが red)
  3. 親スレッドが「検証済み」を通知し、子スレッドがガードを drop する
  - チャネル同期により「並列の偶然」に依存しない決定的テストになる。sleep ベースの待ち合わせは使わない

### 単体テスト
- 既存の seam テスト(safe_delete の refusal 系)は無修正で green
- 境界: ガード drop 後は同一スレッドでも実バックエンドへ戻る(現行 Drop 契約)

## 実装アプローチ
- **Outside-In**: 決定的 red-first pin を先に書いて red を確認 → thread_local 化で green
- **Red-Green-Refactor**: green 後にコメント修正と既存ピン再確認

## 見積もり
1 SP(要チームでの見積もり)

## 技術的考慮事項
- 依存関係: なし(PBI 07 / 08 とファイル排他で並列可。trash.rs を触るのは本 PBI のみ)
- テスタビリティ: チャネル同期で決定的化
- 非機能要件: 本番コード非変更のため挙動リスクゼロ

## 実装者向け注記

### 現状コードの確認
(2026-11-21 時点で実施済み。着手時に再確認すること)
```bash
rg -n "TRASH_BACKEND_OVERRIDE|thread_local" src/trash.rs
rg -n "TrashBackendOverrideGuard" src/
```
thread_local 化は未実装。オーバーライド利用は 3 箇所のみ。

### 実装手順
1. red-first テストを src/trash.rs の tests モジュールに追加(上記「統合テスト」)
2. `static TRASH_BACKEND_OVERRIDE: AtomicBool` を削除し、`thread_local!` + `Cell<bool>` に置換する
3. `set_trash_backend_override` / `trash_backend_override_enabled` を thread_local の `with()` 経由に書き換える
4. src/trash.rs:386-387 とオーバーライド節の doc コメントを事実どおりに修正する
5. `cargo test --lib trash` → `cargo clippy --tests -- -D warnings` → `cargo fmt --check`

### 落とし穴
- ソースピン(src/trash.rs:683)の終端アンカーは `// ── test-only backend override ──` という見出し行そのもの。これを消す・改名するとピンが `unwrap_or(src.len())` で本文全域を grep して意味が変わる。見出しは維持すること
- `AtomicBool` の `Ordering::Relaxed` からの置換で `RefCell` にする誘惑があるが不要。set/get だけなら `Cell<bool>` で足りる(可変参照を渡す箇所がない)
- ガードから TEST_LOCK を外したくなる誘惑に注意。外すと verbose フラグなど他の TEST_LOCK 利用者との相互排除の意味論が変わり、影響調査が本 PBI のスコープを超える

## Definition of Done
- [x] 全 BDD シナリオが自動テストとして実装されパスする
- [x] `cargo test --lib` 10 連続 green(フレーク 0 回)
- [x] コードレビュー完了(2026-11-22 /code-review: ブランチ diff を Approve、SUGGESTION 1 件は `4b5c0b4` で反映済み。コミット単位の最終確認で `e8401d5` 問題なし)
- [x] リファクタリング完了(green 後)
- [x] ロールバック手段: 本番コード非変更のため revert 1 コミットで完全に戻る
- [x] ドキュメント更新: 不要(本番挙動不変、TEST_LOCK 規約は変更しない)

## 実績(2026-11-21 クローズ)

- red-first: 決定的 pin テスト(`delete_ctx_trash_mode_is_unaffected_by_another_threads_override`)が旧グローバル実装で `TrashRefused` により確実に red
- green: `cargo test --lib` 916 passed(thread_local 化後)× 10 連続、フレーク 0
- clippy -D warnings / fmt --check クリーン。本番コード無変更
- なぜなぜ記録: `.plan/whywhy-round11-trash-override-race.md`
