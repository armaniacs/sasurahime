出典: holistic-code-improvement round 9 候補 4「expand_tilde の二重実装」+ なぜなぜ分析 round 9（.plan/whywhy-round9-small-dup.md、2026-11-20、main @ c107011、読み取り専用分析）。本 PBI は同分析 §2 裁定（候補 4）を PBI 形状に写したもの

# PBI 2026-11-20-04: expand_tilde の二重実装を Config::expand_tilde に統一する（refactor / 規模 S）

## メタ情報

- 種別: refactor
- 規模: S（実装の実質: explorer.rs の削除 1 + use 1 + 呼び出し変更 1、tests/ に source pin 1 本、削除前の一時性質テスト 1 本）
- 優先度: 4 位（RICE 1.5 = Reach 3 / Impact 0.5 / Confidence 1.0 / Effort 1 SP）
- 依存: なし（Batch 1 で 2026-11-20-01 と並列実施可能）
- 出所: holistic-code-improvement round 9 + whywhy round 9（§2 裁定）

## ユーザーストーリー

sasurahime の MANAGED_PATTERNS（手書きの静的パターン配列）を保守する保守担当者として、チルダ展開の解決契約が pin 済みの 1 実装（Config::expand_tilde）に収束し、配列に非 "~/" 要素が混入したらテストで検出されることがほしい、なぜなら同名の非公開 1 行版が契約違いで並存し、無テストの分岐が残った状態では、将来のパターン追加 1 行で環境依存の managed 分類破れが静かに起こりうるから。

本 PBI は現行のユーザー体験（explore 画面の managed 分類表示、TUI / CLI のすべての出力）を 1 文字も変えない。変更は内部解決の単一化と、将来のパターン追加で分類が壊れる経路の封鎖のみである。

## 背景/問題

### 確定事項（main @ c107011 実測、推測を含まない）

1. 正装版 `Config::expand_tilde(path, home)`（src/config.rs:253-261）: "~/" 接頭辞、"~" 単独、絶対パス（変更なし）の 3 系統を扱い、単体テスト 3 本が pin する（src/config.rs:374-389: expand_tilde_home_relative / expand_tilde_tilde_alone / expand_tilde_absolute_unchanged）
2. 簡略版 explorer.rs:121-123 の非公開 `expand_tilde(pattern, home)`: 実体は `home.join(pattern.trim_start_matches("~/"))` の 1 行。テストは無い
3. 意味論の差分 3 点:
   - "~" 単独: 正装版は home を返す。簡略版は `home.join("~")`（= `<home>/~` という実在しないパス）
   - 連続 "~/"（例: `~/~/x`）: 正装版は strip_prefix で 1 回だけ剥がす（→ `<home>/~/x`）。簡略版は trim_start_matches で繰り返し剥がす（→ `<home>/x`）
   - 絶対パス: 正装版は `PathBuf::from(path)`。簡略版は `home.join("/abs")` = `/abs`（PathBuf::join の絶対パス置換に偶然依存）
4. 簡略版の唯一の呼び出し元は `is_managed()` 内の MANAGED_PATTERNS 展開（explorer.rs:128、grep 実測で他なし）
5. MANAGED_PATTERNS は全 18 件が "~/" 接頭辞（explorer.rs:17-90 実測。ワイルドカード 1 件 "ms-playwright*"）→ 上記差分 3 点は現行入力では発火せず、簡略版 → 正装版への寄せで byte 等価が保証できる
6. 正装版の呼び出し元: config.rs:209、main.rs 4 箇所（628、650、753、1805）
7. MANAGED_PATTERNS の全要素が "~/" で始まることを pin する source pin は実在しない（tests/ grep 実測）→ 非 "~/" 要素の混入は無監視
8. テスト環境の前提: 簡略版の入力は MANAGED_PATTERNS の 18 件のみ（呼び出し元 1 箇所）のため、static 配列の全走査 = 全入力の比較が可能。簡略版が非公開（fn）のため性質テストは src/explorer.rs の tests モジュール（explorer.rs:379 の既存 mod tests）に置く

### なぜ問題か

- 同名関数が契約違いで並存し、簡略版は無テスト。差分入力（"~" 単独、連続 "~/"、非 "~/"）を通すテストは正装版側にのみ存在する
- MANAGED_PATTERNS は手書き配列で、新クリーナー追加時に 1 行ずつ足される。非 "~/" 要素（絶対パス・相対パス）が混入すると、簡略版は相対パスを home 配下に偶然解決し、絶対パスは join の置換で偶然正しく見える。入力ごとに正誤が反転し、home が異なる環境で結果が変わる
- 壊れ方が静か: is_managed が None を返すだけ（= そのパスが managed 表示から消える）で、クラッシュも出力変化も無い。既存 pin は "~/" 入力しか通さないため、実行時テストでは捕まりにくい
- このリポジトリは「1 つの事実は 1 所有者」（cache-dir resolver、HOME resolution、size parser）を着地させてきた。チルダ展開の意味論の重複はその同型の 4 つ目の事例で、正装版の署名 `Config::expand_tilde(&str, &Path)` は簡略版の呼び出し形（explorer.rs:121-123）と同引数のため、寄せは 1 行で済む

## BDD 受け入れシナリオ

```gherkin
Scenario: explore 画面の managed 分類が現行と byte 等価
  Given MANAGED_PATTERNS の全 18 パターンが静的配列に存在する
  When  簡略版の削除前に、全 18 パターンを簡略版と Config::expand_tilde の両方に入力して解決結果を比較する（性質テスト。static 配列の全走査で全入力比較になる）
  Then  全 18 パターンで出力の等価が記録される（実測として本 PBI ファイルに記載する）
  And   explore 画面の managed 分類表示は 1 文字も変わらない

Scenario: source pin が MANAGED_PATTERNS の非 "~/" 要素を検出する
  Given 全要素の先頭が "~/" であることを確認する source pin が tests/ に存在する
  When  仮想的に非 "~/" 要素（例: path_pattern: "/absolute/cache"）を配列に仮配置する
  Then  pin の RED が仮想要素で記録される（手動確認。確認後に配列を復元する）
```

## 受け入れ基準

- [ ] explorer.rs の非公開 `expand_tilde`（explorer.rs:121-123）を削除し、`is_managed()` の展開を `config::Config::expand_tilde` に寄せる。最小差分: `use` 1 追記 + 呼び出し 1 箇所（explorer.rs:128）の変更のみ。正装版の関連関数のまま降格・移動しない
- [ ] 削除前に全 18 パターン × 簡略版/正装版の出力等価を性質テストで確認し、その結果を本 PBI ファイルに記録する。性質テストは src/explorer.rs の tests モジュールに置き、簡略版削除と同じ変更セットで一緒に撤去する（恒久 pin にしない）
- [ ] MANAGED_PATTERNS の全要素が "~/" 接頭辞であることを確認する source pin を tests/ に 1 本追加する。include_str! 走査、needle は `path_pattern:` の definition line にアンカー、pin 自身の組立リテラルへの一致（自己充足）を回避する（needle は split/format で組み立てる）。追加後に仮想要素で RED を 1 度確認する
- [ ] 既存の managed 表示 pin・is_managed 系テスト・E2E は無修正で green。cargo clippy --tests -- -D warnings が 0、cargo fmt --check が clean
- [ ] 新 util モジュール・新抽象を新設しない（YAGNI。寄せ先は既存の `Config::expand_tilde` に限定）

## テスト戦略

- 削除前の性質テスト（一時）: src/explorer.rs の tests モジュールに、MANAGED_PATTERNS 全 18 パターンを簡略版と `Config::expand_tilde` の両方に通して出力を比較する test を追加する。簡略版の入力は MANAGED_PATTERNS の 18 件のみのため、static 配列の全走査が全入力比較になる。この test は「削除前の 1 度の等価確認」が目的で、削除後は比較対象が消えるため恒久 pin にしない（簡略版削除と同じ変更セットで撤去）
- 恒久 pin は source pin のみに最小化: 「2 実装が消えた後も残る pin」は在庫管理の手間だけを残す。tests/AGENTS.md の source pin 規約に従う: include_str! grep は振る舞いとして表面化しない drift の最後の手段、definition line にアンカー、pin 自身のリテラルとの自己充足回避（needle を split/format で組み立て、pin のソースに一致させない）
- round 8 の空振り pin の教訓を適用: needle は rustfmt 安定形状（`path_pattern: "…"` の 1 行形状）で、pin 自身の組立リテラルに一致させない。走査対象は explorer.rs のみと狭い
- 実施順: (1) 性質テスト追加 → green、等価を実測として本 PBI に記録 (2) source pin 追加 → green、仮想要素で RED 確認 → 配列復元 (3) 簡略版削除 + `Config::expand_tilde` への寄せ + 性質テスト撤去 → 全 green
- 正装版の pin 3 本（config.rs:374-389）と既存の managed 表示 pin・E2E は消費のみで無修正（回帰の検知）

## 見積もり

1 SP（explorer.rs 1 ファイルの削除 + use 1 + 呼び出し変更 1、tests/ に pin 1 本、一時性質テスト 1 本。RICE の Effort 入力と同一）

## 等価性実測記録

- 未記録（削除前の性質テスト green をもって全 18 件一致を確認し、実装時にここへ 1 行で追記する）

## DoD

- [ ] cargo test --no-fail-fast が green（既知の TCC 依存 4 失敗を許容: cleaners::huggingface::tests::clean_fallback_deletes_dir、cleaners::library_logs::tests::clean_all_via_opts_processes_all_entries、cleaners::pre_commit::tests::clean_fallback_deletes_dir、trash::tests::delete_ctx_trash_mode_moves_to_trash。実装時に green でない場合はその時点の既知失敗に合わせる）
- [ ] cargo clippy --tests -- -D warnings が 0
- [ ] cargo fmt --check が通る
- [ ] 全 18 パターンの等価性実測が本 PBI ファイル（等価性実測記録）に記録されている
- [ ] PBI アーカイブ（git mv → .plan/archived/ + .plan/00-INDEX.md の追記。統合側が実施）

## scope 外（やらないこと）

1. Config::expand_tilde の降格/移動（config.rs 内 free fn 化、util モジュールへの移設など）: 既存 5 呼び出し元（config.rs:209、main.rs 628/650/753/1805）の書き換えを伴い、最小差分に反する。関連関数のまま `use` で呼ぶ
2. 連続 "~/"（`~/~/x`）入力への寛容化: is_managed の入力は MANAGED_PATTERNS の静的リテラルで、打ち間違いの余地が無く意味が無い
3. MANAGED_PATTERNS の config 化・外部化: 設計変更で別判断事項。静的配列 + pin の文化（コンパイル時 exhaustive pin 等）を維持する

## 備考

- 触れるファイル: src/explorer.rs（非公開 fn 削除 + use 追記 + 呼び出し変更 + 一時性質テスト）、tests/（source pin 1 本）。src/config.rs と src/main.rs は触れない（正装版とその既存 5 呼び出し元は消費のみ）
- 実装は subagent-driven（AGENTS.md の実装ワークフロー）
- 本 PBI の diff はユーザー可視の出力を 1 バイトも変えない refactor のため、CHANGELOG.md は統合側の一括記載に任せる（本 PBI では触れない）
