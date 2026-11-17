# PBI: meta と SEO 整備 (GitHub Pages)

## ユーザーストーリー

検索経由の訪問者として、検索結果と SNS シェアで正しいタイトルと説明を見たい。
なぜなら現状は `og:title` / `og:description` / `og:type` が無く、doc レイアウトに canonical と hreflang が無く、ページ固有 description も無いから。

## ビジネス価値

- 検索結果での表示が改善し、EN / JA の重複扱いリスクが下がる (canonical + hreflang、x-default 含む)
- SNS シェア時のタイトルと説明が正しく表示される
- 測定方法: ビルド後 HTML の head 内容確認、SNS カードバリデータでの表示確認

## BDD受け入れシナリオ

```gherkin
Scenario: 検索エンジンが正規 URL と言語別 URL を知る
  Given クローラーが doc レイアウトのページを取得している
  When head を解釈する
  Then canonical と hreflang (en、ja、x-default) があり、言語別の正規 URL がわかる

Scenario: SNS でシェアしたときに正しく表示される
  Given 利用者が docs のページ URL を SNS に貼り付けている
  When SNS がプレビューを生成する
  Then og:title と og:description と og:type に基づいたカードが表示される

Scenario: 検索結果でページ固有の説明が出る
  Given 利用者が検索エンジンで sasurahime のドキュメントを探している
  When 検索結果の一覧が表示される
  Then 各ページ固有の description と先頭ユニークな title が表示される

Scenario: モバイルブラウザの UI と調和する
  Given 訪問者がモバイルブラウザでページを開いている
  When ブラウザの UI 領域が表示される
  Then theme-color に基づいた配色で表示される
```

## 受け入れ基準

- [ ] 全ページ head に `og:title`、`og:description`、`og:type` がある
- [ ] 全ページ head に `meta name="theme-color"` がある
- [ ] doc レイアウトに canonical と hreflang (en、ja、`x-default` 含む) がある
- [ ] 各ページにページ固有 description がある (全ページ同一文の使い回しは不可)
- [ ] title が先頭ユニークになっている (例: ページ名が先頭、サイト名は後方)
- [ ] `bundle exec jekyll build` が成功する
- [ ] og:image の扱いが決定され、下記の見送り判断が記録されている

## og:image 見送り判断

現時点では og:image 用のアセットが存在しないため、新規画像の作成は本 PBI の範囲外として見送る。
理由: 画像のデザインとサイズ (1200x630 推奨) の用意が必要で、本 PBI の主目的 (title / description / canonical / hreflang の整備) とは独立に出荷できるため。
将来画像を用意した場合は `og:image` と `og:image:alt` の追加 PBI を起票すること。

## テスト戦略 (t_wadaスタイル)

静的サイトのため Outside-In を次の形で適用する。Rust コードには触れないため `cargo test` / clippy は対象外。

### E2Eテスト

- ビルド後サイトの代表ページ URL を SNS カードバリデータ (X Card Validator 等) で確認する
- 検索エンジンのリッチリザルトテスト等で head の解釈に問題が無いことを確認する

### 統合テスト

- ビルド後の全 HTML に `og:title` / `og:description` / `og:type` / `theme-color` / `canonical` があることを `grep` で検証する
- hreflang に `x-default` が含まれることを `grep` で検証する
- 各ページの description が互いに異なることをスクリプトで検証する (同一文使い回しの検出)

### 単体テスト

- 変更差分の目視レビュー: title の先頭がページごとに異なり、サイト名の位置が統一されていること
- doc レイアウトの canonical / hreflang が `site.url` 等の変数で正しく組み立てられること (ベタ書きの場合は URL 誤記が無いこと)

## 検証手順

1. `bundle exec jekyll build` が通ること (docs/ ディレクトリで実行)
2. 目視確認項目:
   - ビルド後 HTML の head に上記タグが全ページあること
   - EN ページと JA ページで hreflang が相互に指し合っていること
   - タブに表示される title がページごとに区別できること
   - SNS カードバリデータでタイトルと説明が正しく出ること

## 実装アプローチ

- **Outside-In**: 先に期待 head タグ (grep 検証コマンド) を決め、失敗を確認してから実装
- **Red-Green-Refactor**: トップ 2 ページ (EN / JA)、doc レイアウトの順に小さく適用
- **リファクタリング**: グリーンになるたびに head の重複記述をレイアウトや include に寄せる

## 見積もり

2 SP (要チームでの見積もり)

## 技術的考慮事項

- 依存関係: なし (本 PBI 単独で出荷可能)
- テスタビリティ: ビルド後 HTML への grep 検証が主体
- 非機能要件: 既存の canonical / hreflang (トップ 2 ページにあり) と矛盾させない。`og:url` と canonical は一致させる
- 既知の現状 (2026-11-16 確認): `docs/index.html` と `docs/ja/index.html` には `og:locale`、`og:url`、canonical、hreflang (en/ja) が既にあるが、`og:title` / `og:description` / `og:type`、`theme-color`、`x-default` が無い。`docs/_layouts/doc.html` にはいずれも無い
- 参照元 (modern-web-guidance ガイド): `html` (head / meta)

## 実装者向け注記

### 現状コードの確認

(着手前に必ず実行すること。2026-11-16 時点では部分実装であることを確認済み)

```bash
# 既存の og / canonical / hreflang を確認
grep -rn "og:\|canonical\|hreflang\|theme-color" docs/index.html docs/ja/index.html docs/_layouts/doc.html
# 不足分が無いことを確認 (何も出なければ未実装)
grep -rn "og:title\|og:description\|og:type\|theme-color\|x-default" docs/index.html docs/ja/index.html docs/_layouts/doc.html || echo "NOT FOUND"
```

### 実装手順

1. `docs/index.html` の head に `og:title`、`og:description`、`og:type` (トップは `website`)、`theme-color` を追加する
2. `docs/ja/index.html` も同様に追加する (description は日本語のページ固有文にする)
3. `docs/_layouts/doc.html` の head に canonical、hreflang (en、ja、`x-default`)、`og:title`、`og:description`、`og:type` (`article`)、`theme-color` を追加する。本文ページの front matter から title / description を取る形にする
4. 各ページの title を先頭ユニークにする (例: `使い方 | sasurahime` 形式に統一)
5. 各ページ固有の description を用意する (トップ EN / トップ JA / 各 doc ページで異なる文にすること)
6. `bundle exec jekyll build` を実行し、受け入れ基準の grep 検証とカードバリデータ確認を行う

### 落とし穴

- `og:description` と `meta name="description"` の内容を食い違わせないこと (両方置く場合は同一文にする)
- hreflang の URL は末尾スラッシュの有無を含め canonical と一致させること
- `x-default` は言語選択なしの既定 (EN トップ等) を指すこと
- Jekyll の `site.url` が未設定だと絶対 URL が組み立てられない。`docs/_config.yml` の `url` 設定を確認すること

## Definition of Done

- [ ] 全 BDD シナリオが検証手順として実行されパスする
- [ ] `bundle exec jekyll build` が成功する
- [ ] description の使い回しが無いことがスクリプトで確認済みである
- [ ] コードレビュー完了
- [ ] og:image 見送り判断が本 PBI に記録済みである (上記セクション)
