# てなブクの開発

## 開発に必要なもの

- Apple シリコンの Mac と Xcode（iOS 17 以降の SDK を含むもの）
- [XcodeGen](https://github.com/yonaskolb/XcodeGen)（`brew install xcodegen`）
- はてなの OAuth consumer key（ログインを使う機能を試すときだけ）

データ取得層（`Packages/TenabukuKit`）は、Swift のツールチェーンがあれば Linux でもビルドとテストができます。

## 初回の準備

```bash
cd tenabuku-app
cp Config/Secrets.example.xcconfig Config/Secrets.xcconfig
```

`Config/Secrets.xcconfig` に次の値を書きます。このファイルは git の管理外です。

- `DEVELOPMENT_TEAM`: 実機で動かすときの Apple の開発チーム ID
- `HATENA_CONSUMER_KEY` と `HATENA_CONSUMER_SECRET`: [OAuth 開発者向け設定ページ](https://www.hatena.ne.jp/oauth/develop)でアプリケーションを登録して得た値。scope は `read_public`、`write_public`、`read_private`、`write_private` を使えるようにしておきます
- `TENABUKU_BUNDLE_ID`: 既定のバンドル ID がチームで使えないときだけ

consumer key と secret はアプリの Info.plist に入ります。ビルドしたアプリを他人に渡すと、secret も一緒に渡ります。自分で使う前提の作りです。

## ビルドと実行

`Tenabuku.xcodeproj` は `project.yml` から生成するもので、リポジトリには入れていません。pull したあとや `project.yml` を変えたあとは、生成し直します。

```bash
cd tenabuku-app
xcodegen generate
open Tenabuku.xcodeproj
```

Xcode でスキーム `Tenabuku` を選び、シミュレーターか実機で実行します。

ファイルを足したり消したりしたときも `xcodegen generate` を実行します。`project.yml` の `sources` はディレクトリ単位で指定しているので、`Tenabuku/` の下に置いたファイルは自動で入ります。

## 検証コマンド

```bash
cd tenabuku-app/Packages/TenabukuKit
swift test
```

```bash
cd tenabuku-app
xcodegen generate
xcodebuild build -project Tenabuku.xcodeproj -scheme Tenabuku -destination 'generic/platform=iOS Simulator' CODE_SIGNING_ALLOWED=NO
```

Pull Request では `.github/workflows/tenabuku-app.yml` が同じことを実行します。データ取得層のテストは Linux と macOS の両方で、アプリのビルドは macOS で行います。

## 構成

```
tenabuku-app/
├── project.yml              XcodeGen の定義
├── Config/
│   ├── Base.xcconfig        既定値。Secrets.xcconfig があれば読み込む
│   └── Secrets.example.xcconfig
├── Tenabuku/                アプリ本体（SwiftUI）
└── Packages/TenabukuKit/    データ取得層（Swift Package）
    ├── Sources/
    │   ├── HatenaShared/      HTTP、エラー、日付、パーセントエンコード
    │   ├── HatenaOfficial/    公式 API（OAuth、REST API、エントリー情報）
    │   ├── HatenaFeed/        RSS
    │   ├── HatenaUnofficial/  非公式のエンドポイント
    │   └── HatenaTestSupport/ テスト用のスタブ
    └── Tests/
```

## データ取得の層

データ取得層は、取得元がどれだけ変わりにくいかで 3 つのモジュールに分けています。3 つは互いに依存せず、共通の部品だけを `HatenaShared` から使います。はてな側が変わって読めなくなったときに、直す範囲を 1 つのモジュールに収めるためです。

| モジュール | 取得元 | 認証 | 変わりにくさ |
|---|---|---|---|
| HatenaOfficial | REST API（`bookmark.hatenaapis.com/rest/1/my/bookmark` ほか）、マイブックマーク全文検索（`/my/search/json`）、エントリー情報（`/entry/jsonlite/`） | REST API と全文検索は OAuth | 高い（公式ドキュメントあり） |
| HatenaFeed | ホットエントリー（`/hotentry/{category}.rss`）、キーワード検索（`/q/{keyword}?mode=rss`）、ユーザーのブックマーク（`/{user}/bookmark.rss`） | 不要 | 高い |
| HatenaUnofficial | 人気・新着コメント（`/api/entry/{url}/bookmarks?sort=popular`） | 不要 | 低い（非公開） |

2026 年 10 月 31 日に、ログインの要らないエンドポイントへ実際にリクエストを送り、形式を確かめました。ログインが要るもの（REST API、全文検索、OAuth のトークン発行）は、公式ドキュメントの記述に合わせて作ってあり、実際の応答はまだ見ていません。

方針として、次のことを決めています。

- 書き込みは公式の OAuth API だけで行います。
- キーワード検索は `sort`、`date_begin`、`date_end`、`users` で期間と最低ブックマーク数を絞れます。過去のホットエントリーはこれで探す予定です。日付別のホットエントリーのページは HTML しかないので、使うなら `HatenaUnofficial` に置きます。
- 自分のブックマークの検索は公式の全文検索 API を OAuth で使います。

## 壊れたときの検知

はてなの応答が想定した形でないとき、各クライアントは `HatenaError.parseFailed(source:detail:)` を投げます。`source` は取得元の名前（`feed.hotentry`、`unofficial.entry-bookmarks` など）です。ホットエントリーが 0 件の場合も、フィードの形が変わったとみなしてこのエラーにします。

アプリはこのエラーを赤いバナーで表示し、統合ログにも書きます。Mac のコンソール.app でサブシステム `io.github.y-saeki.tenabuku` を指定すると、詳細を読めます。通信の失敗や HTTP のエラーは別の表示にしています。

## テスト

テストはデータ取得層だけにあります（`Packages/TenabukuKit/Tests`）。確かめていることは次のとおりです。

- SHA-1 と HMAC-SHA1 が既知の値と一致すること（RFC 2202 ほか）
- OAuth の署名が RFC 5849 の例と一致すること。はてなに送るリクエストの署名は、Python の標準ライブラリで別に計算した値と比べています
- 各 API が組み立てる URL、メソッド、フォームの中身
- RSS と JSON の読み取り。想定外の形を `parseFailed` として報告すること
- 404 や 401 などのステータスの扱い

テストの入力は 2026 年 10 月に実際に取得した応答の構造をまねて、中身を作り替えたものです。実際の応答は含めていません。

テストで確かめられないことは、次のとおりです。

- はてなが今も同じ形で応答するかどうか。これはアプリを使っていて赤い表示が出たときに分かります
- OAuth の認可の流れ全体と、ログインが要る API の実際の応答
- 画面の見た目と操作

## 手で確かめること

アプリを変えた Pull Request では、シミュレーターか実機で次を確かめます。

- ホットエントリーの全カテゴリーが表示されること
- エントリーを開き、人気と新着のコメントが表示されること
- 「記事を開く」で Safari が開くこと

## バージョン

バージョンは `project.yml` の `MARKETING_VERSION` です。配布はしていないので、リリースの手順とバージョンの上げ忘れを止める CI はありません。

## 今後の作業

- OAuth でのログイン画面。`ASWebAuthenticationSession` を使い、コールバックは `tenabuku://oauth-callback` にする予定です。トークンは Keychain に保存します
- ブックマークの追加・編集・削除の画面
- 過去のホットエントリーの検索画面
- スター。取得は `s.hatena.ne.jp/entry.json` が応答を返しますが、どの URI を渡せばよいかがまだ分かっていません。付与は OAuth の API がなく、ログインの Cookie と `rks` を使う方法しかありません
