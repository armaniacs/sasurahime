# バックログ一覧: 2026-10-23 レビュー発見のPBI化（新規7件）

2回目のコードレビュー（fbbddb7 + b7b5d08 + 017b2c7 の差分）と rust-engineer レビューで発見された項目のうち、即時修正しなかった7点を新規PBI化した。緊急性の高い安全修正はレビュー後すぐにコミット済み（017b2c7: HOME canonicalize、include_manual 回帰、thin 飽和、非TTY重複出力、killpg、空テスト除去など11項目）。

## 既存実装で対応済みの項目（PBI化しない）

| レビューの指摘 | 既存の対応（017b2c7） |
|---------------|---------------------|
| HOME 未正規化でガード迂回 | `is_safe_delete_target` が HOME を canonicalize |
| `auto` が include_manual を無視 | Moderate で `[policy] include_manual` を復活 |
| `--thin 1e11GB` の飽和 | 乗算後の積を再チェック |
| 非TTY の重複出力 | hidden 時の tick eprintln を削除 |
| 空テスト（hidden） | 対象が消えたため除去 |
| 迷子 `#[allow(dead_code)]` | `tick_message` から除去 |
| XDG_CACHE_HOME="" の相対パス | `is_safe_delete_target` が相対パスを拒否 |
| プラグインタイムアウトのリーク | `libc::killpg` でグループ全体を kill |

## 新規RICE採点表（降順＝着手順）

| 順位 | 候補 | Reach | Impact | Conf | Effort | RICE | ファイル |
|-----:|------|------:|-------:|:----:|:---:|-----:|----------|
| 1 | thin コマンドのサイズ上限 cap | 100 | 3 | 100% | 0.3 | 1000 | `pbi/2026-10-23-11-fix-thin-size-cap.md` |
| 2 | fs2 ロックエラーの黙示的破棄を解消 | 300 | 1 | 80% | 0.3 | 800 | `pbi/2026-10-23-12-fix-history-lock-error-logging.md` |
| 3 | huggingface unsafe-HOME テストの空テスト化を解消 | 100 | 1 | 100% | 0.2 | 500 | `pbi/2026-10-23-13-fix-huggingface-test-quality.md` |
| 4 | plugin.rs プロトコル強化バッチ | 100 | 1 | 100% | 0.5 | 200 | `pbi/2026-10-23-14-fix-plugin-protocol-hardening.md` |
| 5 | CleanOptions trash_mode の enum 化 | 500 | 0.5 | 80% | 1.0 | 200 | `pbi/2026-10-23-15-fix-trash-mode-enum.md` |
| 6 | plugin.rs / progress.rs の型整理 | 100 | 0.5 | 80% | 0.5 | 80 | `pbi/2026-10-23-16-fix-plugin-output-types.md` |
| 7 | lib ターゲット分割で doctest を有効化 | 100 | 1 | 80% | 1.5 | 53 | `pbi/2026-10-23-17-feat-lib-target-doctests.md` |

同点（4と5の RICE 200）はリスク軽減効果で決めた: unsafe ブロックの `// SAFETY:` 文書化は skill MUST であり、型整理よりリスク軽減が上回る。

## 不採用・対応不要とした項目

| レビューの指摘 | 判断 |
|---------------|------|
| スキャンキャッシュが in-place 編集を検出できない | APFS の mtime は ns 粒度で、in-place 内容書き込みはそもそも dir mtime を変えない。影響は表示と検出時のサイズフィルタに限られ、削除はキャッシュ不参照。対応不要 |
| huggingface `bytes_freed` の誤報告（env redirect 時） | 既存の表示バグで削除安全性に影響しない。13のテスト品質修正に包含 |
| plugin stdout の誤ステータス残差 | `PluginCleaner::clean` は stdout を参照しないため削除に至らない。fail-closed で対応済み |

## 疑問の解決メモ
- Q: tiny な項目（16）を PBI 化すべきか → A: スコアは低いが型整理は既存の CleanOptions 変更（15）と連動するため、単独 PBI として分離し、着手順を明示した
- Q: lib 分割（17）は bin-only のまま妥協できるか → A: doctest が走らない現状でも単体テストは網羅済み。Effort が最も重いため最下位に置いた
