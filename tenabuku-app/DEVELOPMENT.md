# てなブクの開発

## 開発に必要なもの

- Apple シリコンの Mac と Xcode（iOS 17 以降の SDK を含むもの）
- [XcodeGen](https://github.com/yonaskolb/XcodeGen)（`brew install xcodegen`）
- はてなの OAuth consumer key（ログインを使う機能を試すときだけ）

`Packages/TenabukuKit` は、Swift のツールチェーンがあれば Linux でもビルドとテストができます。

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

pull やブランチの切り替えのたびに生成し直すのが面倒なら、git のフックを一度だけ入れておきます。

```bash
cd tenabuku-app
./scripts/install-git-hooks.sh
```

入れたあとは、pull（rebase を含む）とブランチの切り替えのあとに、`xcodegen generate` が自動で走ります。XcodeGen が見つからないときは生成せず、そのことを表示します。同じ名前のフックをすでに使っているときは、上書きせずに止まります。その場合は、そのフックに `xcodegen generate` の呼び出しを自分で足します。

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
├── Design/AppIcon/          アイコンの元の SVG
├── Tenabuku/                アプリ本体（SwiftUI）
├── scripts/                 git のフックを入れるスクリプト
└── Packages/TenabukuKit/    データ取得層と既読の記録（Swift Package）
    ├── Sources/
    │   ├── HatenaShared/      HTTP、エラー、日付、パーセントエンコード
    │   ├── HatenaOfficial/    公式 API（OAuth、REST API、エントリー情報）
    │   ├── HatenaFeed/        RSS
    │   ├── HatenaUnofficial/  非公式のエンドポイント
    │   ├── ReadingHistory/    既読の記録
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

## 既読の記録

開いた記事は、一覧でタイトルを薄く表示します。記録するのは、一覧からアプリ内ブラウザを開いたときです。キーは一覧のエントリーの URL です。記事の中でリンクをたどっても、開いた元のエントリーが既読になります。

記録は `ReadingHistory` モジュールにあり、ネットワークには触りません。エントリーの URL ごとに、最後に開いた日時を持ちます。アプリは Application Support の `ReadHistory.json` に保存します。変わるたびに書き出し、起動のときに 30 日より古い記録を消します。もう一度開くと、その記事の 30 日は開いた日から数え直します。

30 日にしたのは、次の理由です。

- ホットエントリーは数日で一覧から消えるので、それより長く持っても使い道がありません
- 将来ほかの RSS を読めるようにしたとき、更新の遅いフィードでもひと月あれば足ります

ファイルが壊れていて読めないときは、記録を空にして始めます。理由は統合ログ（カテゴリー `read-history`）に書きます。設定画面の「既読の記録を消す」で、すべての記録を消せます。

## 壊れたときの検知

はてなの応答が想定した形でないとき、各クライアントは `HatenaError.parseFailed(source:detail:)` を投げます。`source` は取得元の名前（`feed.hotentry`、`unofficial.entry-bookmarks` など）です。ホットエントリーが 0 件の場合も、フィードの形が変わったとみなしてこのエラーにします。

アプリはこのエラーを赤いバナーで表示し、統合ログにも書きます。Mac のコンソール.app でサブシステム `io.github.y-saeki.tenabuku` を指定すると、詳細を読めます。通信の失敗や HTTP のエラーは別の表示にしています。

## アイコン

アイコンの元は `Design/AppIcon/` にある 3 つの SVG です。`AppIcon.svg` が通常、`AppIcon-Dark.svg` がダーク、`AppIcon-Tinted.svg` がティントのアイコンです。ダークとティントは背景を透明にしています。ダークの背景はシステムが描き、ティントはグレースケールの絵にシステムが色を付けるためです。

「て」の字は M PLUS Rounded 1c ExtraBold（SIL Open Font License）の字形をパスにしたものです。書き出すときにフォントは要りません。

SVG を変えたら、1024×1024 の PNG に書き出し、`Tenabuku/Assets.xcassets/AppIcon.appiconset/` の同じ名前のファイルと置き換えます。書き出しには rsvg-convert（`brew install librsvg`）を使います。

```bash
cd tenabuku-app
for n in AppIcon AppIcon-Dark AppIcon-Tinted; do
  rsvg-convert -w 1024 -h 1024 "Design/AppIcon/$n.svg" -o "Tenabuku/Assets.xcassets/AppIcon.appiconset/$n.png"
done
```

## テスト

テストは `Packages/TenabukuKit/Tests` にあり、アプリ本体にはありません。確かめていることは次のとおりです。

- SHA-1 と HMAC-SHA1 が既知の値と一致すること（RFC 2202 ほか）
- OAuth の署名が RFC 5849 の例と一致すること。はてなに送るリクエストの署名は、Python の標準ライブラリで別に計算した値と比べています
- 各 API が組み立てる URL、メソッド、フォームの中身
- RSS と JSON の読み取り。想定外の形を `parseFailed` として報告すること
- 404 や 401 などのステータスの扱い
- 既読の記録で、30 日より古いものが消えること。開き直すと残ること。ファイルへの保存と読み込み

テストの入力は 2026 年 10 月に実際に取得した応答の構造をまねて、中身を作り替えたものです。実際の応答は含めていません。

テストで確かめられないことは、次のとおりです。

- はてなが今も同じ形で応答するかどうか。これはアプリを使っていて赤い表示が出たときに分かります
- OAuth の認可の流れ全体と、ログインが要る API の実際の応答
- 画面の見た目と操作。一覧で既読のタイトルが薄くなることも、ここに含みます

## iPhone Duo への対応

画面を作るときは、いつも iPhone Duo を意識します。Apple が勧める作り方に沿っていれば、Duo でも大きく崩れません。具体的には、次のことを守ります。

- レイアウトは、機種や画面の向きではなく、サイズクラスと実際に使える大きさで決めます。内側のディスプレイでは、`UISupportedInterfaceOrientations` の縦向きの指定は効きません
- バーは `NavigationStack` の `.toolbar` で作ります。ボタンにはタイトルとシンボルの両方を付けます（`Label` など）。自分で配置したバーは、Duo の縦向きのレイアウトに参加しません
- ボタンなどの操作できるものはセーフエリアの内側に置きます。背景のように全面に広げるものだけ、セーフエリアの外に出します。セーフエリアは左右で幅が違うことがあるので、片側の値を 2 倍して計算しません
- シートやアラートなど、標準の部品を使います。標準の部品は、Duo の姿勢に合わせて表示が変わります

バーが縦に並ぶかどうかで置き場所を変えたいときは、機種ではなく SwiftUI の環境値 `toolbarVerticalEdge` で見分けます。iOS 27.1 から使える値で、バーを縦に置くことのない環境では `nil` になります。一覧の設定ボタンはこれを使い、縦のバーでは一番下に、それ以外では左上に置いています。Apple は、縦のバーの上を戻るボタンや目立たせたい操作に使うよう勧めているためです。

アプリ内ブラウザは、この方針で縦の表示領域を確保しています。バーは `.toolbar` で作り、ナビゲーションのタイトルは出しません。Duo ではボタンが縦に並ぶので、タイトルを出さなければ、記事が画面の上から下まで使えます。`SFSafariViewController` を使わないのは、「ブコメを見る」などの独自のボタンを置けないためです。

`WKWebView` は上下だけセーフエリアの外に広げ、左右はセーフエリアに収めます。左右まで広げると、ページの `viewport-fit` の指定や CSS によっては、ヘッダーやページ全体が横のツールバーの下に入るためです。

スクロールでバーを隠す動きは入れていません。Duo ではバーが横にあるので、隠しても縦の領域は増えず、読んでいる途中で記事の折り返しが変わるだけです。

## 手で確かめること

アプリを変えた Pull Request では、シミュレーターか実機で次を確かめます。iPhone Duo のシミュレーターでは、閉じた状態と開いた状態の両方で確かめます。

- ホットエントリーの全カテゴリーが表示されること
- エントリーをタップすると、記事がアプリ内ブラウザで開くこと
- ブラウザの「ブコメを見る」で、人気と新着のコメントが表示されること
- コメントの画面の「Safari で開く」で Safari が開くこと
- ブラウザの「ブクマする」で、まだ使えないという案内が出ること
- ブラウザの「共有」で共有メニューが開くこと
- 記事を開いて一覧に戻ると、そのタイトルが薄くなること。アプリを開き直しても薄いままのこと
- 一覧の下のバーにある設定で「既読の記録を消す」を押すと、すべてのタイトルが元の濃さに戻ること

## バージョン

バージョンは `project.yml` の `MARKETING_VERSION` です。配布はしていないので、リリースの手順とバージョンの上げ忘れを止める CI はありません。

## 今後の作業

- OAuth でのログイン画面。`ASWebAuthenticationSession` を使い、コールバックは `tenabuku://oauth-callback` にする予定です。トークンは Keychain に保存します
- ブックマークの追加・編集・削除の画面。アプリ内ブラウザの「ブクマする」は、今は案内を表示するだけで、ここから編集画面を開く予定です
- 過去のホットエントリーの検索画面
- スター。取得は `s.hatena.ne.jp/entry.json` が応答を返しますが、どの URI を渡せばよいかがまだ分かっていません。付与は OAuth の API がなく、ログインの Cookie と `rks` を使う方法しかありません
