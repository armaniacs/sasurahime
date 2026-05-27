# AGENTS.md — AI エージェント向けクイックリファレンス

このディレクトリ (`pbi/`) は **プロダクトバックログ** を管理する場所。
詳細プロセスは `PBI-process.md` を読むこと。

---

## PBI に着手する前に必ずやること

```bash
# 1. テストがグリーンか確認
cargo test

# 2. 対象機能が既に実装済みでないか確認（例）
grep -r "fn detect" src/
grep -r "CleanerName" src/

# 3. アーカイブ済み PBI を確認（重複実装を避ける）
ls .plan/archived/
```

## 新規 PBI を作成するとき

スキル `pbi:create-bdd` を使う。手動で書かない。

命名規則:
```
pbi/YYYY-MM-DD-NN-<type>-<slug>.md
例: pbi/2026-05-28-09-feat-auto-update.md
```

type は `fix` / `feat` / `backlog` のいずれか。

## ブランチ・コミット規則

```
ブランチ: fix/<slug>  または  feat/<slug>
コミット: Conventional Commits 形式
  例: feat(core): add auto-update check
      fix(cli): handle missing config gracefully
```

## 実装の進め方（TDD）

1. `cargo test` でグリーンを確認してから始める
2. E2E テスト（tempdir fixture）→ Integration → Unit の順で書く
3. 各レイヤーで Red → Green → Refactor を繰り返す
4. `cargo clippy -- -D warnings` と `cargo fmt --check` を通してから PR を出す

## 完了・アーカイブ

```bash
git mv pbi/YYYY-MM-DD-NN-<type>-<slug>.md .plan/archived/
# .plan/00-INDEX.md の「アーカイブ一覧」に1行追記する
```

---

詳細: `PBI-process.md`
