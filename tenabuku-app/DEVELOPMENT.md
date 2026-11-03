# てなブクの開発

## 開発に必要なもの

- Apple シリコンの Mac と Xcode 27.1 以降（iOS 27.1 の SDK を含むもの。アプリが動くのは iOS 17 以降です）
- [XcodeGen](https://github.com/yonaskolb/XcodeGen)（`brew install xcodegen`）
- はてなの OAuth consumer key（ログインを使う機能を試すときだけ）

`Packages/TenabukuKit` は、Swift のツールチェーンがあれば Linux でもビルドとテストができます。

## 初回の準備

新しい Mac で開発を始めるときの手順です。上から順に進めます。シミュレーターで動かすだけなら、手順 1 から 5 までで足ります。手順 6 は、自分の iPhone で動かすときだけ行います。

コマンドは「ターミナル」アプリに貼り付けて実行します。ターミナルは、Finder の「アプリケーション」>「ユーティリティ」にあります。

### 1. Xcode を入れる

1. Mac App Store で Xcode を探して入れます。
2. Xcode を一度起動します。追加のコンポーネントを入れるか聞かれたら、iOS を選んで入れます。
3. ターミナルで次を実行し、コマンドラインの開発ツールが Xcode を使うようにします。Mac のログインパスワードを聞かれます。

   ```bash
   sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
   ```

4. 次を実行し、`/Applications/Xcode.app/Contents/Developer` と表示されれば完了です。

   ```bash
   xcode-select -p
   ```

   `/Library/Developer/CommandLineTools` と表示されたら、上の `sudo xcode-select` のコマンドがまだ効いていません。このままだと、あとで `swift test` を実行したときに `XCTest` が見つからずに失敗します。

起動したときに iOS を入れ忘れたら、あとから次のコマンドで入れられます。Xcode の設定の Components からも入れられます。

```bash
xcodebuild -downloadPlatform iOS
```

### 2. Homebrew と XcodeGen を入れる

XcodeGen は、Xcode のプロジェクトを `project.yml` から作るツールです。Homebrew で入れます。

1. Homebrew が入っているかを確かめます。バージョンが表示されれば、次の 2 を飛ばして 3 に進みます。

   ```bash
   brew --version
   ```

2. 「command not found」と表示されたら、Homebrew を入れます。コマンドは [Homebrew の公式サイト](https://brew.sh/)に載っているものです。

   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

   終わると、最後に「Next steps」として、`brew` を使えるようにするためのコマンドが表示されます。表示されたとおりに実行し、ターミナルを開き直します。

3. XcodeGen を入れます。

   ```bash
   brew install xcodegen
   ```

4. 次を実行し、バージョンが表示されれば完了です。

   ```bash
   xcodegen --version
   ```

### 3. リポジトリを取得する

置きたい場所に移動してから、リポジトリを取得します。`git` は Xcode と一緒に入っています。

```bash
git clone https://github.com/y-saeki/vibe-applications.git
cd vibe-applications/tenabuku-app
```

ここから先のコマンドは、どれも `tenabuku-app` の中で実行します。

### 4. 自分用の設定ファイルを作る

見本をコピーして、自分用の設定ファイル `Config/Secrets.xcconfig` を作ります。このファイルは git の管理外です。

```bash
cp Config/Secrets.example.xcconfig Config/Secrets.xcconfig
```

ファイルには次の 3 種類の値を書きます。どれも、必要になるまでは空のままでかまいません。シミュレーターで動かすだけなら、何も書かずに次へ進めます。

- `HATENA_CONSUMER_KEY` と `HATENA_CONSUMER_SECRET`: はてなのログインを使う機能を試すときだけ要ります。取り方は下に書きます
- `DEVELOPMENT_TEAM`: 実機で動かすときだけ要ります。手順 6 で書きます
- `TENABUKU_BUNDLE_ID`: 既定のバンドル ID がチームで使えないときだけ書きます。手順 6 で書きます

はてなの consumer key と secret は、次の手順で取ります。

1. はてなにログインし、[OAuth 開発者向け設定ページ](https://www.hatena.ne.jp/oauth/develop)を開きます。
2. 利用上の注意に同意し、アプリケーションを登録します。名前、Web サイトの URL、ロゴの画像を求められます。審査はなく、登録した直後から使えます。
3. 登録したアプリケーションの設定で、scope の `read_public`、`write_public`、`read_private`、`write_private` を使えるようにします。
4. 同じページにある consumer key と consumer secret を、`Config/Secrets.xcconfig` の `HATENA_CONSUMER_KEY` と `HATENA_CONSUMER_SECRET` に書きます。

consumer key と secret はアプリの Info.plist に入ります。ビルドしたアプリを他人に渡すと、secret も一緒に渡ります。自分で使う前提の作りです。

### 5. シミュレーターで動かす

1. Xcode のプロジェクトを作って開きます。

   ```bash
   xcodegen generate
   open Tenabuku.xcodeproj
   ```

2. Xcode のツールバーの左側で、スキームが `Tenabuku` になっていることを確かめます。その右の実行先のメニューで、iPhone のシミュレーターを選びます。
3. 「Product」>「Run」を選ぶか、ツールバーの実行ボタンを押します。シミュレーターが開き、ホットエントリーの一覧が表示されれば完了です。

続けて、データ取得層のテストが通ることも確かめておきます。

```bash
cd Packages/TenabukuKit
swift test
cd ../..
```

pull のたびに `xcodegen generate` を実行するのが面倒なら、「ビルドと実行」の節にある git のフックを入れておきます。

### 6. 実機で動かす（必要なときだけ）

1. Xcode の「Xcode」>「Settings」>「Apple Accounts」で、Apple アカウントでサインインします。有料の Apple Developer Program に入っていない、個人の Apple アカウントでもかまいません。
2. iPhone を Mac につなぎます。実行先のメニューの「Manage Devices」から Device Hub を開き、iPhone を Mac とペアリングします。
3. iPhone の「設定」>「プライバシーとセキュリティ」で、「セキュリティ」の下にあるデベロッパモードをオンにします。再起動を求められるので再起動し、再起動のあとに出る確認で有効にして、パスコードを入れます。デベロッパモードの項目は、iPhone を Mac とペアリングするまで表示されません。
4. Xcode でプロジェクトのターゲット `Tenabuku` を選び、「Signing & Capabilities」でチームを選びます。
5. 選んだチームの ID を、ターミナルで確かめます。

   ```bash
   xcodebuild -showBuildSettings -project Tenabuku.xcodeproj -scheme Tenabuku | grep DEVELOPMENT_TEAM
   ```

   `DEVELOPMENT_TEAM = ` のあとに表示された英数字が、チームの ID です。

6. その ID を、`Config/Secrets.xcconfig` の `DEVELOPMENT_TEAM` に書きます。Xcode の画面で選んだチームは、`xcodegen generate` を実行すると消えるためです。設定ファイルに書いておけば、作り直しても残ります。
7. もう一度 `xcodegen generate` を実行します。実行先に iPhone を選んで、「Product」>「Run」を選びます。

「Signing & Capabilities」に、バンドル ID が使えないという警告が出ることがあります。そのときは、`Config/Secrets.xcconfig` の `TENABUKU_BUNDLE_ID` の行の先頭にある `//` を消します。そのうえで、自分だけが使う ID（`com.example.tenabuku` の `example` を自分の名前にしたものなど）を書き、`xcodegen generate` を実行し直します。

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

Pull Request では `.github/workflows/tenabuku-app.yml` が同じことを実行します。データ取得層のテストは Linux と macOS の両方で、アプリのビルドは macOS で行います。アプリのビルドには iOS 27.1 の SDK が要るので、Xcode 27.1 が入ったランナー（`xcode-27`）を使います。GitHub ではまだプレビューとして提供されているランナーです。

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

2026 年 10 月 31 日に、ログインの要らないエンドポイントへ実際にリクエストを送り、形式を確かめました。ログインが要るもの（REST API、全文検索、OAuth のトークン発行）は、公式ドキュメントの記述に合わせて作ってあります。2026 年 11 月 4 日に、OAuth のトークン発行と、ブックマークの取得・保存・削除、タグの一覧を実際に試し、どれも読み取れることを確かめました。全文検索、ユーザー情報（`/rest/1/my`）、401 の応答はまだ見ていません。

タグの一覧（`/rest/1/my/tags`）は、ドキュメントではタグの配列を返すことになっています。`{"tags": [...]}` の形で返るという話もあります。実際の応答は読み取れましたが、どちらの形だったかは記録していないので、両方を受け付けたままにしています。

方針として、次のことを決めています。

- 書き込みは公式の OAuth API だけで行います。
- キーワード検索は `sort`、`date_begin`、`date_end`、`users` で期間と最低ブックマーク数を絞れます。過去のホットエントリーはこれで探す予定です。日付別のホットエントリーのページは HTML しかないので、使うなら `HatenaUnofficial` に置きます。
- 自分のブックマークの検索は公式の全文検索 API を OAuth で使います。

## ログイン

ログインは OAuth 1.0a で行います。承認ページはアプリの中の `WKWebView` で開き、はてなが送るコールバックへの移動をそこで止めて、`oauth_verifier` を受け取ります。

`ASWebAuthenticationSession` を使わないのは、はてなが `tenabuku://` のような独自スキームのコールバックを受け付けないためです。2026 年 11 月 4 日に試すと、`/oauth/initiate` は `oauth_parameters_rejected=oauth_callback` を返しました。通るのは http(s) の URL と `oob` だけです。https のコールバックをアプリに戻すには、中継するサーバーかユニバーサルリンクが要ります。どちらも用意しないことにしました。

コールバックは `https://tenabuku.invalid/oauth-callback` です。`.invalid` は名前が引けないと決まっているドメインで、はてなはこの URL も受け付けます。アプリが移動の前に止めるので、端末の外へは出ません。はてなが `oauth_verifier` を付けずにここへ戻したときは、利用者が許可しなかったものとして扱います。

アクセストークンとはてな ID は、Keychain に 1 つの項目として保存します（サービス `io.github.y-saeki.tenabuku`、アカウント `hatena`）。端末のロックを一度解除したあとから読めるようにしてあり、他の端末には同期しません。ログアウトはこの項目を消すだけです。はてなにはトークンを無効にする API がないので、連携をやめるときは、はてなの設定で取り消します。

ログインが要る API が 401 を返したら、保存したトークンは使えないとみなします。それ以降はリクエストを送らず、ログインし直すよう表示します。ログインし直すと、保存したトークンを置き換えます。

## ブックマークの編集

ブックマークの画面を開くと、その記事の自分のブックマークとタグの一覧を同時に読みます。タグの一覧が読めなくても、編集はできます。

タグの候補は、入力した文字を含むものを、入力の先頭と一致するもの、それ以外の順に並べます。それぞれの中では、使った回数の多いものが先です。大文字と小文字、全角と半角は区別しません。入力が空のときは、使った回数の多い順に 20 個だけ並べます。すべて並べると、タグを長く使っている人では候補が数百行になり、その下にある削除のボタンまで届かなくなるためです。保存するとき入力欄に文字が残っていれば、それもタグとして足します。

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
- タグの一覧を、配列と `{"tags": [...]}` のどちらの形でも読めること
- タグを 10 個より多く付けられないこと。空のタグと重複を足さないこと
- タグの候補の絞り込みと並び順
- RSS と JSON の読み取り。想定外の形を `parseFailed` として報告すること
- 404 や 401 などのステータスの扱い
- 既読の記録で、30 日より古いものが消えること。開き直すと残ること。ファイルへの保存と読み込み

テストの入力は 2026 年 10 月に実際に取得した応答の構造をまねて、中身を作り替えたものです。実際の応答は含めていません。

テストで確かめられないことは、次のとおりです。

- はてなが今も同じ形で応答するかどうか。これはアプリを使っていて赤い表示が出たときに分かります
- OAuth の認可の流れ全体と、ログインが要る API の実際の応答
- Keychain への保存と読み込み
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
- 設定の「はてなでログイン」で承認ページが開き、許可すると、はてな ID が表示されること。アプリを開き直してもログインしたままのこと
- ブラウザ、コメントの画面、一覧のスワイプのそれぞれから、ブックマークの画面が開くこと
- コメント、タグ、非公開を付けて保存でき、もう一度開くとその内容が入っていること。削除できること
- タグの入力に合わせて、候補が絞り込まれること。11 個目のタグを足せないこと
- 設定でログアウトすると、ブックマークの画面がログインを求めること
- ブラウザの「共有」で共有メニューが開くこと
- 記事を開いて一覧に戻ると、そのタイトルが薄くなること。アプリを開き直しても薄いままのこと
- 一覧の下のバーにある設定で「既読の記録を消す」を押すと、すべてのタイトルが元の濃さに戻ること

## バージョン

バージョンは `project.yml` の `MARKETING_VERSION` です。配布はしていないので、リリースの手順とバージョンの上げ忘れを止める CI はありません。

## 今後の作業

- 過去のホットエントリーの検索画面
- スター。取得は `s.hatena.ne.jp/entry.json` が応答を返しますが、どの URI を渡せばよいかがまだ分かっていません。付与は OAuth の API がなく、ログインの Cookie と `rks` を使う方法しかありません
