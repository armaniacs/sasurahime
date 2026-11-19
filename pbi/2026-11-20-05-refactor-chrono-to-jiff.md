出所: ユーザー指示「chrono をやめ、jiff へ置き換え」（2026-11-20）。実測アンカーは main @ 7c8cddf。

# PBI 2026-11-20-05: chrono の直接依存を廃止し jiff へ置き換える

- 種別: refactor（挙動不変・timestamp 形式 byte 等価）
- 規模: S
- 出所: ユーザー指示。holistic round 9 の対象外（独立依頼）
- 依存: なし

## 背景（実測済みの事実。推測で書き足さない）

- chrono の直接依存宣言は `Cargo.toml:48`（features = ["serde"]）が唯一で、**コード上の利用は `src/history.rs:242-244` の 1 箇所のみ**: `chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()`
- このタイムスタンプ文字列の消費経路は 3 つ（どれもパース無し・文字列処理のみ）:
  1. `history.json` への書き込み（`HistoryEntry.timestamp`、src/history.rs:13 の String フィールド）
  2. `stats` 表示の先頭 16 文字スライス（src/history.rs:209-213）
  3. エントリの降順ソート（src/history.rs:111 / 151 / 166 の `b.timestamp.cmp(&a.timestamp)`、ISO 形式の辞書順がソートの正しさの根拠）
- 既存履歴ファイルのタイムスタンプは chrono 形式 `2026-05-25T10:30:00+09:00`。fixture と pin が tests/history.rs:5-14、src/history.rs:268 以降に多数実在し、**既存ファイルとの互換が回帰 pin として効いている**
- jiff の strtime `%:z` は chrono と同じ `+09:00` 形（コロン付きオフセット）を生成するため、`%Y-%m-%dT%H:%M:%S%:z` の出力は byte 等価を維持できる
- **scope の事実**: `trash 5.2.9` が chrono に依存している（Cargo.lock の trash の dependencies に chrono 実在）。したがって chrono は `trash` 経由の推移依存として Cargo.lock に残る。本 PBI の範囲は「sasurahime の**直接依存**から chrono を外し、自分のコードから chrono を使わなくする」まで。trash の置き換えは対象外

## ユーザーストーリー

As a 保守担当者, I want the timestamp source to be jiff (a maintained, sound crate) instead of chrono, so that the direct dependency set stays small and one crate (trash) owns the chrono requirement.

## BDD シナリオ

1. **新規エントリの timestamp 形式 byte 等価**: Given tempdir HOME で古い kilo ログを工作し、When `sasurahime clean logs` が 0 バイト超を解放する, Then `history.json` の新規エントリの `timestamp` は `YYYY-MM-DDTHH:MM:SS±HH:MM`（オフセットはコロン付き）の形状で書かれる
2. **既存履歴との互換**: Given chrono 形式のタイムスタンプを持つ既存 `history.json`, When `sasurahime stats` を実行する, Then ロード・ソート・表示が現行どおりで、表示の先頭 16 文字スライス（`YYYY-MM-DDTHH:MM`）も変わらない

## 受け入れ基準

- [ ] `Cargo.toml` から `chrono` 行を削除し、`jiff = "0.2"` を追加する（serde feature 不要: `HistoryEntry.timestamp` は String のまま。`Zoned::now()` に必要な既定 feature を維持）
- [ ] `src/` と `tests/` から `chrono` 参照が 0 件になる（grep 実測）
- [ ] タイムスタンプの生成は jiff の strtime `%Y-%m-%dT%H:%M:%S%:z` に置き換え、**出力形式は現行と byte 等価**（書式文字列自体は現行どおり保持）
- [ ] BDD 1 の E2E pin（書き出された timestamp の形状 assert）を追加する。pin は壁時計の値に依存せず形状のみを主張する（regex 相当の検査または 16 文字スライス + コロン付きオフションの存在確認）
- [ ] 既存テスト（tests/history.rs、src/history.rs tests 等）を 1 行も修正せずに green
- [ ] `cargo tree -i chrono` の結果を記録する: chrono は `trash` 依存としてのみ残り、sasurahime の直接依存から消えること
- [ ] CHANGELOG に Unreleased 節を設けて Changed に 1 行追記（直接依存の chrono 廃止・jiff 置換。タイムスタンプ形式は不変である旨）
- [ ] `cargo clippy --tests -- -D warnings` が 0、`cargo fmt --check` が clean、`cargo build` が green

## テスト戦略

- parity pin 先行: 現行（chrono）出力の形状を E2E で pin してから置換する（置換前後で同一 assert が green であることで byte 等価を証明）
- Outside-In TDD: E2E（tests/history.rs の tempdir HOME ハーネスを流用。解放が 0 バイト超になる工作として kilo ログ fixture のパターンを tests/log.rs から参照）→ ユニット（history.rs の tests に形状 pin。`Zoned::now()` の返す値は壁時計依存のため、テストは出力文字列の形状のみを検査する）
- 既存の fixture 互換 pin（tests/history.rs:5-14 等）は無修正で green が回帰保証

## 見積もり

0.5 SP（1 call site の差し替え + Cargo.toml + E2E pin 1 本 + CHANGELOG 1 行）

## DoD

- [ ] cargo test が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] cargo build が green
- [ ] PBI アーカイブ（統合側が実施: git mv → `.plan/archived/`、INDEX 追記）

## scope 外（やらないこと）

- `trash` クレートの置き換え・chrono 推移依存の根絶（Cargo.lock に chrono が残ることは trash の要件として許容し、記録する）
- `HistoryEntry` の serde 契約変更（timestamp は String のまま。chrono の serde feature が担っていた用途は現行コードに実在しない）
- タイムスタンプの解析・パース追加、表示形式の変更
- README / docs の変更（タイムスタンプ形式はユーザー可視の変更が無いため）

## 備考

- 実装は subagent-driven（AGENTS.md の実装ワークフロー）。git 操作・pbi/ 編集は統合側が実施
- 触れるファイル: `Cargo.toml`、`Cargo.lock`（cargo add/update 由来）、`src/history.rs`、`tests/history.rs`（pin 追加のみ）、`CHANGELOG.md`（Unreleased 1 行）
- 触れてはいけないファイル: その他の src/、tests/ 既存ファイルの変更（pin の needle が壊れた場合のみ最小修正で、理由を報告に記す）
