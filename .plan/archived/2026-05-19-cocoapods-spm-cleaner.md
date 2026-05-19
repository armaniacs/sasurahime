# PBI: CocoaPods & SwiftPM Cache Cleaner

## ユーザーストーリー
iOS/macOSアプリ開発者として、CocoaPods と SwiftPM のキャッシュを掃除したい、なぜなら pod cache が数百MBに膨れ上がり、SwiftPM のキャッシュも Xcode のバージョンアップごとに肥大化するから。

## ビジネス価値
iOS/macOS 開発者には必須のキャッシュ削除対象。CocoaPods ユーザーは特に archive cache が肥大化しやすい。

## BDD受け入れシナリオ

```gherkin
Scenario: CocoaPods キャッシュを削除する
  Given pod コマンドが PATH に存在する
  When  sasurahime clean cocoapods を実行する
  Then  pod cache clean --all が実行される

Scenario: SwiftPM キャッシュを削除する
  Given ~/Library/Caches/org.swift.swiftpm/ が存在する
  When  sasurahime clean spm を実行する
  Then  キャッシュディレクトリが削除される
```

## テスト戦略

### E2Eテスト
- ダミーの `pod` スクリプトを作成して実行確認
- 空の SwiftPM キャッシュディレクトリを作成して scan が動作することを確認

### 単体テスト
- SwiftPM キャッシュパスの解決
- CocoaPods `pod` 検出

## 実装アプローチ
- CocoaPods: `pod cache clean --all`（brew/bun と同じパターン）
- SwiftPM: `~/Library/Caches/org.swift.swiftpm/` を `fs::remove_dir_all` で削除 + `mkdir` で再作成（node-gyp と同じパターン）
