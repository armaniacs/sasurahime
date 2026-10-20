# PBI: apfs-snapshot の軽微な積み残しを整理する

## ユーザーストーリー
sasurahime の保守者として、apfs-snapshot のレビューで見送った軽微事項に、やる/やらないの判断を付けて片付けたい。なぜなら、個別には小さくても放置すると、不正な名前を受け付ける、失敗理由が消える、clippy が通らないといった問題が積み重なるから。

## ビジネス価値
- 失敗の原因 (権限不足以外) がユーザーに届く
- `cargo clippy --all-targets` を CI や手元で使えるようにする
- 「やらない」と決めた項目は理由つきで残り、再議論を避けられる

## BDD受け入れシナリオ

```gherkin
Scenario: あり得ない日付のスナップショット名は一覧に出ない
  Given tmutil の出力に "com.apple.TimeMachine.2026-99-99-999999.local" がある
  When  スナップショット名を解析する
  Then  その名前は対象に含まれない

Scenario: 削除失敗の理由が skipped に残る
  Given tmutil deletelocalsnapshots が stderr に "Operation not permitted" を出して失敗する
  When  ユーザーが `sasurahime clean apfs-snapshot --all` を実行する
  Then  skipped の reason に tmutil の出力が含まれる
  And   root が必要な失敗では従来どおり sudo のコマンドが案内される

Scenario: clippy が全ターゲットで通る
  Given 現在のブランチ
  When  `cargo clippy --all-targets -- -D warnings` を実行する
  Then  警告ゼロで終了する
```

## 受け入れ基準
各項目の推奨は「やる / やらない」で、受け入れ対象は「やる」の項目のみ。

- [ ] (a) 日付の妥当性検査 (月 01-12、日 01-31、時 00-23、分秒 00-59) を `snapshot_date` に追加する (やる、優先度 低)
- [ ] (c) 削除失敗の理由を `SkippedEntry.reason` に含める (やる、優先度 中)
- [ ] (e) `cargo clippy --all-targets -- -D warnings` を通す (やる、優先度 中)
- [ ] (b) Time Machine 以外の local snapshot は一覧に出さない現仕様を維持し、docs の記述が正しいことを確認する (やらない、理由は注記)
- [ ] (d) 混在入力のテスト: 既存テストで足りるため追加しない (やらない)。不足分だけ補う判断は注記

## テスト戦略（t_wadaスタイル）

### E2Eテスト
- `tests/apfs_snapshot.rs` の既存テストが通ること。新規追加は不要

### 統合テスト
- fake runner が失敗の stderr を返す構成で、`clean` の `skipped[].reason` に原因が入ることを検証する (c)

### 単体テスト
- `snapshot_date`: 月 13、日 32、時 24、分 60 などを None にする (a)
- 正常日付とうるう日 (2024-02-29) は Some のまま (a)

## 実装アプローチ
- **Outside-In**: (c) の統合テストを先に Red にする
- **Red-Green-Refactor**: (a) は表形式の単体テストから
- **リファクタリング**: (e) は最後に、テスト構成の変更だけを行う

## 見積もり
2 ストーリーポイント。理由: (a)(c)(e) は各々小さいが、(c) は runner の `Output.stderr` と `Err` の扱いを揃える判断がある。(b)(d) は作業なし。

## 技術的考慮事項
- 依存関係: なし。対象コードはブランチ feat/apfs-snapshot-enhancement で、現在のブランチ docs/system-data-specs に祖先として取り込み済み (`git merge-base --is-ancestor feat/apfs-snapshot-enhancement HEAD` が成功する)
- テスタビリティ: `CommandRunner` のモックで足りる
- 非機能要件: なし

## 実装者向け注記

### 現状コードの確認
```bash
cargo test
cargo clippy --all-targets 2>&1 | tail -15
grep -n "fn snapshot_date\|fn parse_snapshot_names\|Ok(_) | Err(_)" src/cleaners/apfs_snapshot.rs
grep -n "parse_mixed_input" src/cleaners/apfs_snapshot.rs
sed -n 1170,1176p docs/SUPPORTED.md
```

事実確認の結果 (HEAD 時点):

(a) `src/cleaners/apfs_snapshot.rs:46` の `snapshot_date` は、`com.apple.TimeMachine.` を外し、`.local` は `strip_suffix(".local").unwrap_or(rest)` で任意扱い、残りが長さ 17 で位置 4, 7, 10 が `-`、他が数字であれば受け付ける。範囲検査が無いので `2026-99-99-999999` は通る。想定と違う点: ".local 無し" を受け付けるのは意図的な実装 (`unwrap_or`) で、`tmutil deletelocalsnapshots` が日付形式だけを受け付けるための寛容さと読める。バグとは言い切れない。
推奨: 範囲検査だけ「やる」(優先度 低)。実害は、`tmutil` が不正な日付を拒否してその項目が failed 扱いになるだけで、誤削除にはつながらない。".local 無し" は仕様として維持する。

(b) `parse_snapshot_names` (`apfs_snapshot.rs:60`) は `snapshot_date(l).is_some()` の行だけを残すので、`com.apple.os.update-*` などは黙って捨てられる。`docs/SUPPORTED.md` の 1174 行付近に "Only Time Machine snapshots (...) are listed; other snapshots are ignored" と明記済み。
推奨: やらない (優先度 なし)。`os.update` などは OS 更新が使うスナップショットで、消すと更新に影響し得る。表示するとユーザーが消せないものを見せることになる。docs の記述で十分。

(c) `apfs_snapshot.rs:247-253` の `delete_snapshots` は `Ok(o) if o.status.success()` 以外を `Ok(_) | Err(_) => report.failed_dates.push(date)` で一括して捨て、`o.stderr` も `Err` の内容も保持しない。`clean` の `skipped` の reason (200 行) は常に "requires root: sudo tmutil deletelocalsnapshots {d}" 固定なので、権限以外の失敗 (`tmutil` が見つからない、スナップショットが既に無い、タイムアウト) でも root が必要という誤った案内になる。
推奨: やる (優先度 中)。`DeleteReport.failed_dates` を (date, reason) に変え、reason に stderr の 1 行目か `Err` の文字列を入れる。sudo 案内 (`sudo_hint`) は維持する。

(d) 想定と違う点: 混在入力のテストは既にある。`apfs_snapshot.rs:639` の `parse_mixed_input_keeps_only_time_machine_lines` が、ヘッダ行、正常な TimeMachine 行、`com.apple.os.update-ABC123`、`com.apple.TimeMachine.bogus.local` の混在を検証している。`parse_ignores_header_line` と `snapshot_date_extracts_timestamp` も関連する。
推奨: やらない。(a) を実装する際に、範囲外の日付を混在入力のケースへ 1 行足すだけにする。

(e) `cargo clippy --all-targets` は警告 1 件: `src/cleaner.rs:60` の `#[expect(dead_code, reason = "pub API - callers may read")]` (`name` フィールド) が "this lint expectation is unfulfilled"。テストビルドでは `name` がテストから読まれるため dead_code が出ず、expect が満たされない。通常の `cargo clippy -- -D warnings` は通る。`--all-targets` に `-D warnings` を付けると失敗する (今回の作業と無関係の既存問題)。同じ構造体の 70 行付近の `deleted_paths` の `#[expect(dead_code)]` も同じ理由で出る可能性があるので、実行して確認する。
推奨: やる (優先度 中)。`#[cfg_attr(not(test), expect(dead_code, reason = "..."))]` に変えるか、`#[allow(dead_code)]` に統一する (`uses_trash` は既に allow)。`expect` を使うのは未使用が解消されたときに気付くためなので、前者を推奨する。

### 再現手順
- (a): `snapshot_date("com.apple.TimeMachine.2026-99-99-999999.local")` が `Some` を返す (単体テストを書けば再現できる)
- (c): fake runner の `deleteLocalsnapshots` を exit 1 + stderr 付きで失敗させ、`skipped[0].reason` が固定文言であることを確認する
- (e): `cargo clippy --all-targets -- -D warnings`

### 落とし穴
- (a) で検査を厳しくしすぎない。月ごとの日数 (2 月 30 日) まで検査する必要は無く、`tmutil` に任せてよい
- (c) `sudo_hint` は root 起因の失敗だけに出すべきか (現状は全失敗に出す) を決める。stderr に "not permitted" 等が含まれるかで分岐するのは文言依存で壊れやすいので、案内は全失敗に出したまま reason だけ正確にするのが安全
- (e) `cfg_attr` の `expect` は、`--all-targets` と通常ビルドの両方で警告ゼロを確認する (片方だけ直して他方を壊しやすい)

### 検証コマンド
```bash
cargo test apfs
cargo test --test apfs_snapshot
cargo clippy --all-targets -- -D warnings
cargo clippy -- -D warnings
cargo fmt --check
```

## Definition of Done
- [ ] 「やる」と決めた項目 (a)(c)(e) の BDDシナリオが自動テストとして実装されパスする
- [ ] 「やらない」項目 (b)(d) の理由が PBI に残っている
- [ ] `cargo clippy --all-targets -- -D warnings` と `cargo clippy -- -D warnings` がパスする
- [ ] `cargo fmt --check` がパスする
- [ ] コードレビュー完了
- [ ] `docs/SUPPORTED.md` の apfs-snapshot 節が (c) の変更後の挙動と一致している
