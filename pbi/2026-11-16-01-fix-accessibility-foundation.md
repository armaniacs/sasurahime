# PBI: アクセシビリティ土台 (GitHub Pages)

## ユーザーストーリー

サイト訪問者として、キーボード操作とスクリーンリーダーでドキュメントを読めるようにしたい。
なぜなら現状の docs 配下ページにはランドマークやスキップリンクがなく、
支援技術の利用者が目的の内容にたどり着けないから。

## ビジネス価値

- キーボードのみの利用者とスクリーンリーダー利用者が全ページを利用できるようになる
- 受け入れ後の簡易アクセシビリティチェック (axe DevTools 等) で重大違反ゼロを目指せる土台ができる
- 測定方法: キーボードのみでトップから本文へ到達できること、スクリーンリーダー読み上げ順序が妥当であること

## BDD受け入れシナリオ

```gherkin
Scenario: キーボード利用者がスキップリンクで本文へ移動する
  Given 訪問者が docs トップページを開いている
  When Tab キーを1回押す
  Then スキップリンクが表示され、Enter で main ランドマークへフォーカスが移動する

Scenario: スクリーンリーダー利用者が表の内容を理解する
  Given 訪問者が targets table のあるページを開いている
  When スクリーンリーダーで表を読み上げる
  Then caption で表の目的が伝わり、scope 属性で行列ヘッダーが関連付けて読まれる

Scenario: 言語切替の状態が支援技術に伝わる
  Given 訪問者が日本語ページを開いている
  When 言語ボタンの状態を確認する
  Then 現言語側に aria-current が付き、言語ボタンに aria-pressed の正しい状態が設定されている

Scenario: 装飾要素が読み上げを妨げない
  Given 訪問者がヒーローセクションのあるページを開いている
  When スクリーンリーダーでページ全体を読み上げる
  Then 装飾 SVG と絵文字が読み上げられず、本文の読み上げ順序が崩れない
```

## 受け入れ基準

- [ ] `docs/index.html`、`docs/ja/index.html`、`docs/_layouts/doc.html` に `<main>` ランドマークとスキップリンクがある
- [ ] targets table に `caption` と `scope` (`scope="col"` / `scope="row"`) が付いている
- [ ] hero-code / install-block が `pre > code` 構成で、スクロール可能な要素に `tabindex="0"`、`role="region"`、`aria-label` が付いている
- [ ] 装飾 SVG と絵文字に `aria-hidden="true"` が付いている
- [ ] nav が `ul` / `li` 構成で `aria-label` を持ち、現言語表示に `aria-current`、言語ボタンに `aria-pressed` が付いている
- [ ] `docs/assets/style.css` に `.skip-link` と `:focus-visible` スタイルがある
- [ ] `bundle exec jekyll build` が成功する

## テスト戦略 (t_wadaスタイル)

静的サイトのため Outside-In を次の形で適用する。Rust コードには触れないため `cargo test` / clippy は対象外。

### E2Eテスト

- ビルド成果物 (`_site/`) に対してスキップリンクから main への到達を手動またはスクリプトで確認する
- axe DevTools (または同等のチェッカー) で対象3ページを走査し、重大違反が出ないこと

### 統合テスト

- ビルド後の全 HTML に `<main>` が1つだけ存在することを `grep` で検証する
- nav の `ul` / `li` 構成と `aria-*` 属性の有無をビルド後 HTML で検証する

### 単体テスト

- 変更差分の目視レビュー: 各属性 (`aria-hidden`、`aria-current`、`aria-pressed`、`tabindex`、`role`) が意図した要素に付いていること
- `style.css` の `.skip-link` が初期状態で視覚的に隠れ、フォーカス時に表示されること

## 検証手順

1. `bundle exec jekyll build` が通ること (docs/ ディレクトリで実行)
2. 目視確認項目:
   - Tab キー押下でスキップリンクが表示され、Enter で本文へ移動すること
   - フォーカスリングが `:focus-visible` で視認できること
   - 横スクロールするコードブロックにキーボードでフォーカスを当ててスクロールできること
   - スクリーンリーダー (VoiceOver 等) で nav、言語ボタン、表が妥当に読まれること

## 実装アプローチ

- **Outside-In**: 先にビルド後 HTML への期待 (grep 検証コマンド) を決め、それが失敗することを確認してから実装
- **Red-Green-Refactor**: ページ単位 (トップ EN、トップ JA、doc レイアウトの順) に小さく適用
- **リファクタリング**: グリーンになるたびに属性の付け忘れがないか全ページ横断で確認

## 見積もり

3 SP (要チームでの見積もり)

## 技術的考慮事項

- 依存関係: なし (本 PBI 単独で出荷可能、S2〜S4 とは独立)
- テスタビリティ: ビルド後 HTML に対する grep 検証が主体
- 非機能要件: 既存の見た目を変えない (スキップリンクはフォーカス時のみ表示)
- 参照元 (modern-web-guidance ガイド): `accessibility`、`html`、`icons` (装飾アイコンの扱い)

## 実装者向け注記

### 現状コードの確認

(着手前に必ず実行すること。2026-11-16 時点ではいずれも未実装であることを確認済み)

```bash
# スキップリンクと main ランドマークが無いことを確認
grep -rn "skip-link\|<main" docs/index.html docs/ja/index.html docs/_layouts/doc.html docs/assets/style.css || echo "NOT FOUND"
# ARIA 属性が無いことを確認
grep -rn "aria-\|tabindex" docs/index.html docs/ja/index.html docs/_layouts/doc.html || echo "NOT FOUND"
# table に caption/scope が無いことを確認
grep -rn "caption\|scope" docs/index.html docs/ja/index.html docs/_layouts/doc.html || echo "NOT FOUND"
```

### 実装手順

1. `docs/_layouts/doc.html` にスキップリンク (`<a class="skip-link" href="#main">`) と `<main id="main">` を追加する
2. `docs/index.html` と `docs/ja/index.html` に同様のスキップリンクと `<main>` を追加する
3. targets table に `<caption>` と `th` への `scope="col"`、行ヘッダーへの `scope="row"` を付ける
4. hero-code / install-block を `pre > code` 化し、スクロール要素に `tabindex="0"`、`role="region"`、`aria-label` を付ける
5. 装飾 SVG と絵文字に `aria-hidden="true"` を付ける
6. nav を `ul` / `li` 化して `aria-label` を付け、現言語表示に `aria-current="page"`、言語ボタンに `aria-pressed` を付ける
7. `docs/assets/style.css` に `.skip-link` (visually-hidden + フォーカス時表示) と `:focus-visible` スタイルを追加する
8. `bundle exec jekyll build` を実行し、受け入れ基準の grep 検証を行う

### 落とし穴

- `tabindex` はスクロール可能な要素にだけ付け、不要な要素に正の値を付けないこと (フォーカス順序が壊れる)
- `aria-hidden="true"` をフォーカス可能な要素や意味のあるテキストに付けないこと
- 日本語ページのスキップリンク文言は日本語にすること (`documentElement.lang` と一致させるのは S3 の範囲)

## Definition of Done

- [ ] 全 BDD シナリオが検証手順として実行されパスする
- [ ] `bundle exec jekyll build` が成功する
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み (本 PBI 自体がドキュメント改善のため追加更新は不要である旨を明記)
