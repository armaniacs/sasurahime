# PBI: テーブルとレスポンシブ改善 (GitHub Pages)

## ユーザーストーリー

モバイル端末の訪問者として、小さい画面でも表とナビゲーションが崩れずに読めるようにしたい。
なぜなら現状の docs ページは横幅の狭い画面で表がはみ出し、ダークモードやハイコントラスト環境への配慮も無いから。

## ビジネス価値

- スマートフォンからの訪問者が表の内容 (対応ツール一覧など) をスクロールで確認できるようになる
- ダークモード利用者と Windows ハイコントラスト (forced-colors) 利用者の可読性が上がる
- 測定方法: 幅 360px の表示で横はみ出しが無いこと、ダークモードと forced-colors での目視確認

## BDD受け入れシナリオ

```gherkin
Scenario: モバイル利用者が表を横スクロールで読む
  Given 訪問者が幅 360px の画面で targets table のあるページを開いている
  When 表の領域を横にスクロールする
  Then 表全体が横スクロールでき、ヘッダー行が追従して列の意味がわかる

Scenario: ダークモード利用者が違和感なく読む
  Given 訪問者の OS がダークモードに設定されている
  When docs のいずれかのページを開く
  Then ページがダーク配色で表示され、フォーム部品やスクロールバーも極端な反転を起こさない

Scenario: キーボード利用者がスクロール領域を操作する
  Given 訪問者がデスクトップで表のあるページを開いている
  When Tab キーで表のラッパーにフォーカスを当てる
  Then ラッパーがフォーカス可能で、矢印キーで表内をスクロールできる

Scenario: 長いページの後半が軽く表示される
  Given 訪問者が長いドキュメントページを開いている
  When ファーストビューより下の section までスクロールする
  Then ファーストビューの表示がブロックされず、スクロールに応じて下部が描画される
```

## 受け入れ基準

- [ ] `:root` に `color-scheme: dark` があり、全ページ head に `meta name="color-scheme"` がある
- [ ] `.targets-wrap` と `.doc-body table` が `overflow-x: auto` で、ラッパーに `tabindex` / `role` / `aria-label` が付いている
- [ ] 表ヘッダーが sticky で追従する
- [ ] `details summary` に自作シェブロン (マーカー置換) がある
- [ ] nav が `height` 固定ではなく `min-height` + `flex-wrap` になっている
- [ ] features-grid が container queries による段階移行になっている
- [ ] フォールド以下 section に `content-visibility: auto` がある
- [ ] hero-code / install-block に `overflow-x: auto` がある
- [ ] 見出しに `text-wrap: balance` がある
- [ ] グローバル `*` リセットが分割 (margin/padding と box-sizing を分離など) されている
- [ ] forced-colors 対応 (`forced-color-adjust` 等) がある
- [ ] `bundle exec jekyll build` が成功する

## テスト戦略 (t_wadaスタイル)

静的サイトのため Outside-In を次の形で適用する。Rust コードには触れないため `cargo test` / clippy は対象外。

### E2Eテスト

- レスポンシブモード (360px / 768px / 1280px) で対象ページを開き、横はみ出しが無いことを目視確認する
- OS をダークモードにして全ページを確認する
- Windows ハイコントラスト (またはエミュレーション) で forced-colors の表示を確認する

### 統合テスト

- ビルド後 CSS に `color-scheme`、`content-visibility`、`container-type`、`text-wrap`、`forced-colors` の各指定が含まれることを `grep` で検証する
- ビルド後 HTML に `meta name="color-scheme"` が全ページあることを `grep` で検証する

### 単体テスト

- 変更差分の目視レビュー: `*` リセット分割が意図しない副作用 (フォーム部品の崩れなど) を起こしていないこと
- sticky ヘッダーが `overflow` のある祖先で正しく追従すること

## 検証手順

1. `bundle exec jekyll build` が通ること (docs/ ディレクトリで実行)
2. 目視確認項目:
   - 360px 幅で表が横スクロールでき、ヘッダーが追従すること
   - nav が折り返して表示され、項目が隠れないこと
   - ダークモードで本文・表・コードブロックが読めること
   - `details` の開閉シェブロンが表示され、開閉できること
   - 見出しの改行バランスが不自然でないこと
   - ハイコントラスト環境で文字と背景が潰れないこと

## 実装アプローチ

- **Outside-In**: 先に期待 CSS/HTML (grep 検証コマンド) を決め、失敗を確認してから実装
- **Red-Green-Refactor**: テーブル系、ナビ系、配色系の順に小さく適用し、各段階で 360px 目視を行う
- **リファクタリング**: グリーンになるたびに重複指定や詳細度の競合を整理する

## 見積もり

5 SP (要チームでの見積もり)

## 技術的考慮事項

- 依存関係: なし (本 PBI 単独で出荷可能、S1 の main ランドマーク等とは競合しない。S1 と同時実装する場合は `role="region"` 付きラッパーとの二重指定に注意)
- テスタビリティ: ビルド後 CSS/HTML への grep 検証と実機の目視が主体
- 非機能要件: デスクトップ表示の見た目を変えない。`content-visibility` はファーストビュー直下には付けない (CLS 悪化防止)
- 参照元 (modern-web-guidance ガイド): `responsive-table`、`dark-mode`、`navbar`、`html`

## 実装者向け注記

### 現状コードの確認

(着手前に必ず実行すること。2026-11-16 時点ではいずれも未実装であることを確認済み)

```bash
# color-scheme が無いことを確認
grep -rn "color-scheme" docs/index.html docs/ja/index.html docs/_layouts/doc.html docs/assets/style.css || echo "NOT FOUND"
# targets ラッパーの有無を確認
grep -rn "targets-wrap\|content-visibility\|container-type\|text-wrap\|forced-colors" docs/assets/style.css || echo "NOT FOUND"
```

### 実装手順

1. `docs/assets/style.css` の `:root` に `color-scheme: dark` (ライト配色との併記なら `light dark`) を追加する
2. 全ページ head (index、ja/index、doc レイアウト) に `meta name="color-scheme"` を追加する
3. targets table を `.targets-wrap` で囲み (または既存ラッパーに付与し)、`overflow-x: auto` と `tabindex="0"` / `role="region"` / `aria-label` を付ける。`.doc-body table` も同様に扱う
4. `th` / `thead` に `position: sticky; top: 0` を付ける
5. `details summary` の既定マーカーを消し (`::marker` / `::-webkit-details-marker`)、自作シェブロン (`::after` 等) を付ける
6. nav の `height` 固定を `min-height` に変え `flex-wrap: wrap` を付ける
7. features-grid を container queries (`container-type: inline-size` + `@container`) による段階移行にする
8. フォールド以下の section に `content-visibility: auto` と `contain-intrinsic-size` を付ける
9. hero-code / install-block に `overflow-x: auto` を付ける
10. 見出し (`h1`〜`h3` 程度) に `text-wrap: balance` を付ける
11. グローバル `*` リセットを分割する (例: `*, *::before, *::after { box-sizing }` と `body, h1... { margin }` の分離)
12. `@media (forced-colors: active)` でリンク・ボタン・表罫線の視認性を確保する
13. `bundle exec jekyll build` を実行し、受け入れ基準の grep 検証と 3 幅での目視を行う

### 落とし穴

- `content-visibility: auto` を `position: sticky` の祖先に付けると追従が壊れる。表を含む section への付与は避けること
- `overflow-x: auto` を `body` や全体ラッパーに付けると sticky が効かなくなる。表ラッパーに限定すること
- container queries を使う要素には `container-type` が必要。親子関係の取り違えに注意すること

## Definition of Done

- [ ] 全 BDD シナリオが検証手順として実行されパスする
- [ ] `bundle exec jekyll build` が成功する
- [ ] 360px / 768px / 1280px の目視確認が完了している
- [ ] コードレビュー完了
- [ ] ドキュメント更新済み (本 PBI 自体がドキュメント改善のため追加更新は不要である旨を明記)
