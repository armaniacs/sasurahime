# PBI-005: Playwright / Puppeteer 旧ブラウザバージョン削除

## ユーザーストーリー
**Playwright や Puppeteer を使う開発者**として、古いブラウザバイナリを自動検出して削除したい、なぜなら複数バージョンが同居して合計 2GB 超になることがあるから。

## ビジネス価値
- 今回のセッションで Puppeteer 5バージョン + Playwright 旧版で約 2.4GB 回収
- ブラウザキャッシュは更新するたびに旧版が残り続ける

## BDD 受け入れシナリオ

```gherkin
Scenario: Puppeteer の旧 Chrome バージョンを削除する
  Given ~/.cache/puppeteer/chrome に複数バージョンが存在する
  When `sasurahime clean browsers` を実行する
  Then 最新バージョン（最も高いビルド番号）以外が削除候補として表示される
  And 確認後に削除される

Scenario: Playwright の旧 chromium ビルドを削除する
  Given ~/Library/Caches/ms-playwright に chromium-1208 と chromium-1217 が存在する
  When `sasurahime clean browsers` を実行する
  Then chromium-1208（古い方）が削除候補として表示される
  And chromium-1217 は保持される

Scenario: ms-playwright-go の旧バージョンを削除する
  Given ~/Library/Caches/ms-playwright-go に 1.50.1 と 1.57.0 が存在する
  When `sasurahime clean browsers` を実行する
  Then 1.50.1 が削除候補として表示される

Scenario: ブラウザキャッシュが存在しない
  Given ~/.cache/puppeteer が存在しない
  When `sasurahime clean browsers` を実行する
  Then "puppeteer: not found" と表示して正常終了する
```

## 受け入れ基準
- [ ] Puppeteer: `~/.cache/puppeteer/chrome/` と `chrome-headless-shell/` の旧版を検出・削除
- [ ] Playwright: `~/Library/Caches/ms-playwright/` の旧 chromium / headless_shell を検出・削除
- [ ] ms-playwright-go: `~/Library/Caches/ms-playwright-go/` の旧バージョンを検出・削除
- [ ] 「最新版」の判定はビルド番号（数値）の最大値
- [ ] `--dry-run` で削除せず一覧表示のみ
- [ ] 各ディレクトリが存在しない場合はスキップ

## t_wada スタイル テスト戦略
```
E2Eテスト:
- tmpdir に mac_arm-131.x / mac_arm-140.x を作成し、clean 後に 131.x のみ削除されることを確認

統合テスト:
- BrowserVersionDetector::find_old_versions(dir) が最新以外を返すことを確認

単体テスト:
- extract_build_number("mac_arm-137.0.7151.119") => 7151119 (または類似の比較可能な値)
- find_latest(["131.0.6778.204", "137.0.7151.119", "140.0.7339.80"]) => "140.0.7339.80"
```

## 実装アプローチ
- **Outside-In**: tmpdir E2E テストから開始
- バージョン比較: ディレクトリ名から semver 風にパースして最大値を保持

## 見積もり
3 ストーリーポイント

## Definition of Done
- [ ] 全受け入れシナリオが通る
- [ ] 最新バージョンが誤削除されないことを確認
- [ ] `cargo clippy` 警告ゼロ
