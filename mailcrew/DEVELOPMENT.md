# Mail Crew の開発

Mail Crew をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

Mail Crew は SwiftUI の macOS アプリです。Gmail との通信、ログイン、メール本文の解析とサニタイズは Rust のクレート（`core/`）が受け持ち、アプリは UniFFI のバインディングを通してそれを呼びます。画面に出すこと、ブラウザやファイルを開くことは Swift の側（`MailCrew/`）が受け持ちます。

配布対象は macOS（Apple Silicon）だけで、GitHub Actions でビルドします。0.7 までは Tauri で作ったアプリを macOS と Windows に配布していました。0.8 でこのアプリに置き換え、Windows 版の配布はやめました。`core/` の Windows 向けのコード（資格情報マネージャーへの保存など）は、Windows のアプリを作るときのために残しています。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)（stable） | `rustup` でインストール。`rustup target add aarch64-apple-darwin` |
| Xcode | アプリ（`MailCrew/`）と、Swift から Rust を呼ぶ部分（`core/swift/`）に使います |
| [XcodeGen](https://github.com/yonaskolb/XcodeGen) | Xcode のプロジェクトを `project.yml` から作ります。`brew install xcodegen` |

動かして確かめるには、README の「初回の準備」で作る OAuth クライアントも要ります。

## セットアップと実行

Xcode のプロジェクト（`MailCrew.xcodeproj`）はコミットしないので、最初に一度作ります。作り直すのは、`project.yml` が変わったときだけです。

```sh
cd mailcrew
xcodegen generate         # project.yml から MailCrew.xcodeproj を作る
open MailCrew.xcodeproj   # Xcode で開き、⌘R で実行、⌘U でテスト
```

ふだんは、pull したら Xcode で ⌘R を押すだけです。

- スキームのビルド前の処理が `core/swift/build.sh` を呼び、Rust のライブラリとバインディングを作ります。Rust 側が変わっていなければ、cargo も `build.sh` も何もしないので、数秒で終わります。
- Xcode から呼ばれるときは PATH に cargo がないので、`build.sh` は `~/.cargo/bin` を足し、それでもなければ `rustup which cargo` で探します。Homebrew の rustup は cargo を PATH に置かないためです。
- スキームのビルド前の処理は、失敗してもビルドを止めません。そこで、`build.sh` が失敗したときは出力を `core/target/xcode-build.failed` に残します。続くアプリのターゲットの最初のビルド フェーズ（Check the Rust build）が、それをビルドのエラーとして出します。
- `build.sh` は、生成したバインディングのうち中身が変わったファイルだけを書き換えます。同じ中身で書き直すと、毎回パッケージがビルドし直されるためです。
- `MailCrew/` と `MailCrewTests/` は同期フォルダ（Xcode 16 の synchronized folders）です。Swift のファイルを足したり消したりしても、`xcodegen generate` は要りません。

配布用のビルドは、CI と同じく次のとおりです。

```sh
xcodebuild build -project MailCrew.xcodeproj -scheme MailCrew -configuration Release \
  -destination 'generic/platform=macOS' -derivedDataPath build/DerivedData
```

アプリは `build/DerivedData/Build/Products/Release/Mail Crew.app` にできます。CI はこれと「アプリケーション」フォルダへのリンクを `hdiutil` でディスクイメージ（`Mail Crew_<version>_aarch64.dmg`）にまとめます。名前は Tauri 版のディスクイメージと同じ形にしています。Release に添付すると、ファイル名の空白は GitHub が `.` に置き換えます。

アプリの名前は「Mail Crew」で、`project.yml` の `PRODUCT_NAME` と `CFBundleDisplayName` がこれを決めます。ディレクトリ名、クレートの名前、バンドル識別子（`com.ysaeki.mailcrew`）、Release のタグ（`mailcrew-v<version>`）は `mailcrew` のままです。

アイコンは Icon Composer のデータ `MailCrew/AppIcon.icon` です。白い封筒の後ろに、シアンから紫へ色の移るカードが 3 枚、扇のように重なります。手前の封筒と 3 枚のカードを別々の層で持ち、光沢や透け感は描き込まずに macOS 26 の Liquid Glass に任せています。Xcode に入っている Icon Composer で開いて編集できます。層は 1024 × 1024 の SVG（`Assets/`）で、カードはどれも封筒と同じ大きさです。1 枚ごとに同じ距離だけ右上へずらし、同じ角度だけ傾けています。

ビルドでは `actool` がこれを `Assets.car`（macOS 26 以降）と `AppIcon.icns`（それより前の macOS）に変換し、`Info.plist` の `CFBundleIconName` と `CFBundleIconFile` がそれぞれを指します。変換はビルドのたびに Xcode が行うので、変換の結果はコミットしません。

## 検証コマンド

```sh
cd core && cargo fmt --check && cargo clippy --all-targets --features ffi -- -D warnings && cargo test --features ffi
```

Swift から Rust を呼ぶ部分と、アプリのテストは次のとおりです。

```sh
cd core/swift
./build.sh                                        # Rust の静的ライブラリと Swift のバインディングを作る
swift test -Xlinker -L"$(./build.sh --lib-dir)"   # Swift から呼んで確かめる
cd ../..
xcodebuild test -project MailCrew.xcodeproj -scheme MailCrew -destination 'platform=macOS,arch=arm64'
```

## 構成

| 場所 | 役割 |
|---|---|
| `core/src/auth.rs` | Google へのログイン（OAuth 2.0、ループバックのリダイレクトと PKCE）、トークンの取得と更新 |
| `core/src/credentials.rs` | アカウント情報の保存先（macOS は設定フォルダの `accounts.json`、Windows は資格情報マネージャー）と、以前のバージョンの保存の形からの移し替え |
| `core/src/accounts.rs` | ログイン中のアカウントの一覧と、全アカウントの受信トレイを 1 つの列にまとめる処理 |
| `core/src/gmail.rs` | 1 アカウント分の Gmail API の呼び出し。受信トレイのスレッド、ラベル、ラベルの付け外し |
| `core/src/views.rs` | カスタムビューの保存先（アプリの設定ディレクトリの `views.json`） |
| `core/src/image_senders.rs` | 外部画像を常に表示する差出人の保存先（アプリの設定ディレクトリの `image-senders.json`） |
| `core/src/card.rs` | スレッドから 1 枚のカードに載せる内容を組み立てる |
| `core/src/mime.rs` | Gmail が返す MIME 構造の読み取り。本文の選択、文字コード（宣言がないか誤っているときは推測する）、RFC 2047 のヘッダー |
| `core/src/sanitize.rs` | HTML 本文のサニタイズ（ammonia）と、埋め込み画像の `cid:` を `data:` URL に置き換える処理 |
| `core/src/attachments.rs` | 添付ファイルの書き出し。保存するファイル名、上書きしない書き出し先、開いてよい種類 |
| `core/src/service.rs` | アプリができることの一覧（`Core`）。バインディングはここを呼ぶ |
| `core/src/links.rs` | アプリの外で開いてよいリンク（http、https、mailto） |
| `core/src/authres.rs` | Gmail が付けた `Authentication-Results` ヘッダーから、SPF・DKIM・DMARC の結果を読み取る |
| `core/src/bimi.rs` | BIMI のレコードとマーク証明書（VMC・CMC）の検証。ロゴは証明書から取り出す |
| `core/bimi-roots/` | マーク証明書の検証に使うルート証明書 |
| `core/src/preferences.rs` | 設定の保存先（アプリの設定ディレクトリの `preferences.json`）。いまは BIMI のロゴを表示するかどうかだけ |
| `core/src/ffi.rs` | アプリが Swift から呼ぶためのバインディング（UniFFI） |
| `core/swift/` | `core/` を Swift から使うためのパッケージ（`MailCrewCore`）と、そのテスト |
| `project.yml` | Xcode プロジェクトの定義（XcodeGen）と、アプリのバージョン |
| `MailCrew/AppModel.swift` | アプリの状態。どの画面を出すか、ログイン、受信トレイの取得と失敗の扱い、ビュー、設定。`MailCrew` を呼ぶのはここだけ |
| `MailCrew/SignInView.swift`、`InboxView.swift`、`CardView.swift` | 画面。ログイン、サイドバーとカードの列と列の終わり、カード |
| `MailCrew/PullToRefresh.swift` | 引っ張って再読み込み。指の状態と引っ張った量の判定と、すき間に出す表示 |
| `MailCrew/SettingsView.swift` | 設定のウィンドウ。アカウント、ビュー、画像を常に表示する差出人、BIMI |
| `MailCrew/CardToolbar.swift` | カードの下のボタンの行。タグの一覧と、添付ファイルの一覧 |
| `MailCrew/TrustRow.swift` | 件名の下の検証結果と、BIMI のロゴを描く WKWebView |
| `MailCrew/MessageFrame.swift` | HTML 本文を表示するときの決まり。文書と CSP、`WKContentRuleList` のルール、遷移の判定、テキスト本文のリンク |
| `MailCrew/MessageWebView.swift` | HTML 本文の WKWebView。高さと縮小の計算、リンクを外で開く処理 |
| `MailCrewTests/` | アプリの単体テスト |

### Rust の中身と Swift からの呼び出し

`core/` は、画面に関わらない処理をまとめたライブラリのクレート（`mailcrew-core`）です。入口は `Core`（`core/src/service.rs`）です。ブラウザを開く、ファイルを OS に渡す、ダウンロード フォルダを探すといった OS の機能だけは、アプリが受け持ちます。たとえばログインは、`start_sign_in` が待ち受けのポートと同意画面の URL を用意し、アプリがその URL をブラウザで開いてから `finish_sign_in` を待ちます。

設定ファイル（`views.json`、`image-senders.json`、`preferences.json`）を置くディレクトリは `Core::new` に渡します。アプリは、Tauri 版と同じ `~/Library/Application Support/com.ysaeki.mailcrew` を渡します。そのため、Tauri 版の設定がそのまま引き継がれます。アカウント情報（`accounts.json`）もここに置きます。0.8.0 まではキーチェーンに入れていて、初回の起動でここに移します。

アプリは `core/src/ffi.rs` を通して `Core` を呼びます。これは `ffi` フィーチャーを付けたときだけビルドされ、[UniFFI](https://mozilla.github.io/uniffi-rs/) が Swift のバインディングを生成します。

- `MailCrew` が `Core` にあたるオブジェクトです。メソッドは Swift の `async throws` になり、処理は Rust 側で持つ Tokio のランタイムで動きます。失敗は `MailCrewError` で返ります。
- ログインは `startSignIn` が返す `SignIn` の `url()` をブラウザで開き、`finish()` を待ちます。
- 受け渡す型（カード、本文、添付ファイルなど）は `ffi.rs` の中で別に定義し、Rust の型から変換します。添付ファイルの取得元やメールの差出人のように、Rust の中でだけ使う値は Swift に渡しません。

`core/swift/build.sh` は、`core/` を Apple Silicon 向けの静的ライブラリ（`libmailcrew_core.a`）としてビルドし、そこから C のヘッダーと Swift のソースを生成します。生成したファイルはコミットしません。`core/swift/Package.swift` はそれらを `MailCrewCore` というライブラリにまとめます。

`core/` のバージョンは `0.0.0` のままです。単独では配布しないためで、リリースのバージョンは `project.yml` が決めます。

### アプリ

アプリは `project.yml` から XcodeGen で Xcode のプロジェクトを作ります。プロジェクトの形式は、同期フォルダを使うために `xcode16_0` にしています。`MailCrewCore`（`core/swift/`）をローカルのパッケージとして使い、`build.sh` が作った `libmailcrew_core.a` をリンクします。Apple Silicon 向けだけで、対応する macOS は 14 以降です。`build.sh` は Rust のライブラリも同じ macOS 14 向けにビルドします。

バンドル識別子は Tauri 版と同じ `com.ysaeki.mailcrew` です。証明書では署名せず、Tauri 版と同じくアドホック署名でビルドします。

`MailCrewCore` の `View` と `Label` は、SwiftUI の同じ名前の型と衝突します。そのため画面のファイルは `MailCrewCore` を読み込まず、Rust 側を呼ぶのは `AppModel` だけにしています。画面が使う型（`Card` など）には、`AppModel.swift` で別名を付けています。

設定は、macOS のほかのアプリと同じく、別のウィンドウ（⌘,）で開きます。Tauri 版では、受信トレイと同じウィンドウの中に出していました。

### カードの列のスクロール

横にスワイプした列はカードの位置で止まります（`StripScroll.swift`）。止まる位置は、スクロールが始まった位置から、指を離したあとに慣性で進む先までの距離で決めます。カード 1 枚分の 15% を越えて進むと次のカードに移り、それより短ければ元のカードに戻ります。さらに長く進んだときは、15% を越えて入ったカードの枚数だけ移ります。SwiftUI の `.viewAligned` だけでは、進む先にいちばん近いカードで止まるので、半分を越えるまで次のカードに移らず、スクロールが重く感じられました。そこで、移る先のカードを決めてから、止まる位置の細かい合わせ方は `.viewAligned` に任せています。カードが止まる位置は以前と変わりません。

### 引っ張って再読み込み

列を先頭のカードより右へ引っ張り、指を離すと再読み込みします（`PullToRefresh.swift`）。引っ張りとして扱うのは、列が先頭のカードの位置にあるときに始まり、最初に右へ動いた操作だけです。指の状態はスクロールのイベントの `phase`（`began`、`changed`、`ended`、`cancelled`）で分かります。`phase` のないマウスのホイールは数えません。古いメールから勢いよく戻ってきて先頭を越えても、その操作はほかの位置で始まっているので、跳ね返るだけです。

`NSScrollView` の端のバウンスは、指をどれだけ動かしても数十ポイントで止まり、引っ張るには短すぎます。そのため、引っ張っている間は、イベントのモニター（`NSEvent.addLocalMonitorForEvents`）がスクロールのイベントを `NSScrollView` に渡さず、カードを自分でずらします。ずらす量は指の動いた量から決め、最初は指にほぼついていき、784 ポイントに近づくほど遅くなります。`began` と `ended` のイベントは `NSScrollView` にも渡します。始まりを見たまま終わりを見ないと、`NSScrollView` が操作の途中だと思い続けるためです。引っ張ったあとの慣性のイベント（`momentumPhase` があるもの）も渡しません。指を左へ戻して先頭のカードの位置を越えると、その先は列が普通にスクロールします。

カードの位置を決めるときは、クリップ ビューの `bounds.origin.x` に `contentInsets.left` を足します。サイドバーは列の上に浮いていて、`NSScrollView` はサイドバーの下まで広がります。そのため、内容にはサイドバーの幅の余白が付き、先頭のカードの位置でも `bounds.origin.x` はその幅だけマイナスになります。

336 ポイント以上引っ張ったところで指を離すと再読み込みし、届く前に戻したときは何もしません。引っ張っている間は、開いたすき間に矢印と「引っ張って再読み込み」を出します。矢印は SF Symbols の `arrow.clockwise` ではなく、`ReloadArrow` で描いています。`arrow.clockwise` は矢じりが円からはみ出していて、枠の中心が円の中心からずれます。そのため、回すと軸がぶれて見えます。言葉は、すき間が言葉の幅より広くなってから出します。336 ポイントに届くと、塗りつぶした矢印と「離すと再読み込み」をアクセントの色で出し、トラックパッドを振動させます（`NSHapticFeedbackManager`）。戻して届かなくなったときも、弱く振動させます。離して再読み込みが始まると、すき間を 120 ポイント開けたまま「読み込み中」を出し、終わったら閉じます。

### ログインとアカウント情報

OAuth クライアントは利用者が自分の Google Cloud プロジェクトで作ったものを使い、配布物には何も埋め込みません。ログインは「インストール型アプリの OAuth 2.0」の流れです。`127.0.0.1` の空いているポートで待ち受け、既定のブラウザで同意画面を開き、戻ってきたリダイレクトから認可コードを受け取ります。PKCE（S256）と `state` を使い、5 分で待ち受けをやめます。スコープは `gmail.modify` の 1 つです。

クライアント ID、クライアント シークレット、リフレッシュトークン、メールアドレスは、アカウントごとに 1 つの JSON にまとめます。保存先は OS で異なります。

- macOS: 全アカウントの JSON を配列にして、設定フォルダ（`~/Library/Application Support/com.ysaeki.mailcrew/`）の `accounts.json` に入れます。ファイルは権限 0600 で作り、隣に書いてから置き換えるので、途中で落ちても前の内容が残ります。暗号化はしません。Thunderbird もマスターパスワードがなければ、ログイン情報を同じプロファイルにある鍵で暗号化するだけなので、守り方はほぼ同じです。
- Windows（今は配布していません）: 資格情報マネージャーに入れます（サービス名 `com.ysaeki.mailcrew`）。資格情報マネージャーは 1 項目を 2,560 バイトまでに制限しています。全アカウントをまとめると数アカウントで超えるので、アカウントごとに `google-account:<メールアドレス>` という項目に入れます。資格情報ストアには保存した項目の一覧を問い合わせられないため、メールアドレスを追加した順に並べた一覧を `accounts` という別の項目に入れます。

macOS をキーチェーンにしないのは、配布物を Apple が発行した証明書で署名していないからです。キーチェーンは、更新したアプリが項目を初めて読むときに確認を出します。Apple が発行していない証明書の署名は cdhash で区別されるので（Apple が公開している securityd のソース）、ビルドが変わるたびに別のアプリとみなされ、自分で作った証明書で署名しても毎回パスワードを求められます。Developer ID で署名するようになったら、キーチェーンに戻すかを考え直します。

古い形からの移し替えは起動時に行います。macOS は `accounts.json` がなければキーチェーンを読みます。0.4 から 0.8.0 は `all-accounts` という 1 つの項目に、0.3 までは Windows と同じ形で入れていました。見つかったアカウントを `accounts.json` に書いてから、読んだ項目を消します。キーチェーンが削除を断った項目は残しますが、`accounts.json` ができたあとは読みません。キーチェーンを読むときに 1 回だけ確認が出ます。アカウントを全部外しても `accounts.json` は空の配列として残るので、キーチェーンを読み直すことはありません。0.1 までのバージョンは 1 アカウントを `google-account` に入れていました。Windows で `accounts` がなく `google-account` があれば、新しい形へ移して古い項目を消します。

設定からアカウントを追加するとき、クライアント ID とシークレットを空欄のままにすると、Rust 側は最初のアカウントのクライアントを使います。クライアントは Swift の側に渡しません。

アクセストークンはアカウントごとにメモリにだけ持ち、期限の 1 分前か、API が 401 を返したときに取り直します。Google があるアカウントのリフレッシュトークンを受け付けなくなる（`invalid_grant`）と、そのアカウントの保存した情報を消し、アプリには `SignedOut` を返します。アプリはそのアカウントのカードを外し、ほかのアカウントが残っていれば受信トレイのまま知らせを出し、残っていなければログイン画面に戻ります。

### 受信トレイとボタン

`list_inbox` は、対象のアカウント（ビューで選んだもの。「受信トレイ」と「あとで読む」のタブなら全アカウント）ごとに `threads.list`（50 件。ビューに検索条件があれば `q` に渡す）で並びを取り、各スレッドを `threads.get`（`format=metadata`、`From` と `Subject` のヘッダーだけ）で 8 件ずつ並行して取ります。アカウントどうしは並行して取り、全アカウントのカードを日時の新しい順に並べて 50 件に切ります。各アカウントの新しい 50 件をそろえてから切るので、全体の新しい 50 件が漏れません。`threads.list` の `labelIds` は、「あとで読む」のタブなら `SetAside` ラベル、それ以外のタブなら `INBOX` です。`SetAside` ラベルがまだないアカウントは、ラベルを作らずに空の一覧を返します。カードには下書きを除いた最新の 1 通の送信者、件名、日時と、スレッドの既読やスターの状態を載せ、どのアカウントのスレッドかを `account` に持たせます。

本文と添付ファイルの一覧は、カードが画面に近づいたときにアプリが `get_message` で取ります。Rust 側は `messages.get`（`format=full`）で 1 通を取り、本文を選んでサニタイズして返します。取るまでのカードにはスニペットが出ます。取った本文はアカウントごとに 200 通までメモリに持ちます。

Gmail API の上限は、1 ユーザーあたり 1 分に 6,000 単位です（[Usage limits](https://developers.google.com/workspace/gmail/api/reference/quota)）。`threads.get` は `format=full` だと 120 単位を使うので、50 件のスレッドを本文ごと取ると、それだけで上限を超えます。`format=metadata` なら 40 単位なので、一覧の読み込みは `threads.list` の 10 単位と合わせて 2,010 単位です。本文の `messages.get`（`format=full`）は 1 通 60 単位で、画面に近づいたカードの分だけかかります。

さらにアカウントごとに、取得したスレッドのカードを `threads.list` が返す `historyId` と一緒にメモリに持ちます。再読み込みやビューの切り替えで `threads.get` を呼ぶのは、`historyId` が変わったスレッドだけです。持つのは 1 アカウントあたり 1,000 件までで、超えたら直近の一覧にないものを捨てます。

Google が上限を理由に断ったとき（429、または理由が `rateLimitExceeded` や `userRateLimitExceeded` の 403）は、上の Usage limits のページが勧めるとおり、1、2、4 秒と倍に延ばした間（それぞれ最大 1 秒を足し、32 秒で頭打ち）を置いて、同じリクエストを 6 回まで送り直します。待つ時間は合わせて 1 分を少し超えます。それでも断られたら、少し待ってから再読み込みするよう知らせます。

一部のアカウントだけが失敗したときは、ほかのアカウントのカードと、失敗したアカウントとその理由（`failures`）を一緒に返します。アプリは理由を知らせに出します。

ボタンの操作はカードの `account` のアカウントに送ります。まとめて完了はアカウントごとに `archive` を呼びます。一部のアカウントだけが失敗したときは、受信トレイを読み込み直してから理由を知らせます。タグの一覧には、カードと同じアカウントのラベルが並びます。

ボタンを押すと、カードを読み込んだときのメッセージ ID を対象に `messages.batchModify` を呼びます。スレッド単位の `threads.modify` を使わないのは、カードを表示している間に届いた返信まで受信トレイから外さないためです。

| ボタン | 付けるラベル | 外すラベル |
|---|---|---|
| 完了 / まとめて完了 | なし | `INBOX`、`UNREAD`（「あとで読む」のタブでは `SetAside` も） |
| タグ | 選んだラベル | `INBOX`（「あとで読む」のタブでは `SetAside` も） |
| あとで読む | `SetAside` | `INBOX` |
| スター（付ける） | 表示中の 1 通に `STARRED` | なし |
| スター（外す） | なし | スレッドの全メッセージから `STARRED` |

「あとで読む」のタブでは、カードに「あとで読む」のボタンを出しません。アプリは `archive` と `apply_label` に、カードを読み込んだタブの `mailbox`（`inbox` か `setAside`）を渡します。

アプリはボタンを押した時点でカードを消し、Rust 側が失敗を返したら元の位置に戻します。

### 添付ファイルと埋め込み画像

添付ファイルの一覧に載るのは、ファイル名があるパートのうち、HTML 本文が `cid:` で表示している画像を除いたものです。以前は `Content-Disposition: inline` で `Content-ID` があるパートを一律に除いていましたが、本文が表示しない画像まで見えなくなるので、本文が実際に参照しているかで分けています。一覧の各項目はメッセージの中のパート ID（`partId`）で区別します。

埋め込み画像は `get_message` の中で取ります。HTML 本文が `cid:` で参照する `image/*` のパートを、`messages.attachments.get`（1 回 20 単位）で取り、`data:` URL にしてサニタイズのときに `src` と `background` を置き換えます。Gmail が本文と一緒に中身を返したパートは、追加の呼び出しなしで使います。取るのは 1 通につき 20 枚、合わせて 5 MB までで、それを超える画像と、取れなかった画像は表示しません。取れなかった画像があるときは、本文をメモリに残さず、次にカードを表示したときに取り直します。本文の CSP は前から `img-src data:` を許しているので、ネットワークへの読み込みは増えず、開いたことは送り主に伝わりません。

添付ファイルを開くとき（`open_attachment`）と保存するとき（`save_attachment`）は、Rust 側がメモリにある本文から添付ファイルを探し、`messages.attachments.get`（20 単位）で中身を取ります。ファイル名は `attachments.rs` の `safe_file_name` で、パスの区切りや OS が受け付けない文字を `_` にし、先頭と末尾の `.` と空白を取り除いてから使います。末尾の `.` や空白は Windows がエラーを出さずに取り除くので、残すと拡張子が変わってしまいます。

- 開く: 一時ディレクトリの `mailcrew-attachments/<乱数>/` に書き出し、アプリが `NSWorkspace` で OS の選ぶアプリに渡します。このディレクトリは起動のたびに消します。開けるのは、拡張子が `attachments.rs` の `OPENABLE` にある種類（文書、画像、音声、動画）だけです。開いたときに何が起きるかは拡張子で決まり、拡張子は送り主が決められるためです。プログラム、スクリプト、ショートカット、インストーラ、アーカイブ、HTML や SVG、マクロ付きの Office 文書は、アプリに「開く」を出さず、Rust 側も断ります。
- 保存: ダウンロード フォルダに書き出します。同じ名前のファイルがあれば `名前 (2).pdf` のように番号を付け、既存のファイルは上書きしません。

### 本文の文字コード

パートのヘッダーにある `charset` は、Gmail API が返すバイト列の文字コードと一致するとは限りません。

- Gmail API が本文を UTF-8 に変換して返したのに、ヘッダーは元の `ISO-2022-JP` のまま、ということがあります。実際のメールで確認しました。ヘッダーどおり ISO-2022-JP として読むと、日本語がすべて置換文字（�）になります。
- 送信者の側で、パートのヘッダーに文字コードがない、`us-ascii` と書いてある、ブラウザが知らない名前（`cp932` など）を使っている、といったメールもあります。

そのため `mime.rs` の `decode_text` は次の順に試し、エラーなく読めた最初のものを使います。

1. パートのヘッダーの `charset`（`us-ascii` は宣言なしとみなす）
2. HTML の `<meta>` が宣言する文字コード
3. エスケープシーケンスがあれば ISO-2022-JP
4. UTF-8
5. [chardetng](https://crates.io/crates/chardetng)（Firefox の文字コード推測）の結果

どれでもエラーなく読めなければ、1 の文字コード（なければ UTF-8）で置換文字を入れて読みます。

### HTML 本文の安全対策

HTML 本文は 2 段階で守ります。

1. Rust 側（`sanitize.rs`）で、実行や送信につながるもの（`script`、イベントハンドラ、`form`、`iframe`、`object`、`embed`、`meta`、`base`、`link`、`svg`、`javascript:` のリンク、相対 URL）を取り除きます。メールのレイアウトに要る `style` 要素と `style` 属性、表組みの属性は残します。外部画像を読み込むかどうかも、ここで判定します。埋め込み画像の `cid:` は `data:` URL に置き換え、取っていない画像の `cid:` は取り除きます。
2. アプリ側（`MessageFrame.swift`、`MessageWebView.swift`）で、残りを WKWebView に入れます。

WKWebView には次の決まりがあります。

- `WKWebpagePreferences.allowsContentJavaScript` を切るので、メールの中のスクリプトは動きません。高さを測るスクリプトはアプリが入れるもので、メールとは別のコンテンツワールドで動きます。
- 文書には専用の Content-Security-Policy（`default-src 'none'; style-src 'unsafe-inline'; img-src data:`）と `no-referrer` を付けます。それに加えて `WKContentRuleList` で、文書（`about:`）と `data:` 以外の読み込みをすべて止めます。「表示する」を押したときと、差出人が「画像を常に表示する差出人」にあるときは、`img-src` に `https:` と `http:` を加え、http と https の画像だけを通すルールに替えて読み込み直します。CSP とルールのどちらか一方だけでも外部の読み込みは止まります。ルールの正規表現には `|` が使えないので、許す URL は 1 つずつ別のルールにしています。
- WebView が読み込んでよいのは、アプリが `loadHTMLString` で渡した文書（`about:blank`）だけです。ほかの遷移はすべて止めます。クリックしたリンクのうち、`is_openable_link`（Rust 側）が通した http、https、mailto だけを既定のアプリで開きます。新しいウィンドウは作りません。
- Force Touch のリンクのプレビューは、リンク先を読み込むので切っています。データストアは保存しないもの（`nonPersistent()`）を使い、許可した画像が送った Cookie はアプリを終了すると消えます。
- WebView はスクロールしません。スクロールのイベントはすべてカードの本文に渡し、本文の高さに合わせて WebView を伸ばします。横方向には何もスクロールせず、カードより幅の広いメールは CSS の `zoom` で縮めて収めます。カードの上で横にスワイプしても、カードの中ではなくカードの列が動きます。高さは本文の最後に置いた目印の位置で測るので、メールが `html` や `body` に `height: 100%` を付けていても WebView ほど高くなりません。

### 画像を常に表示する差出人

「この差出人は常に表示」で覚えるのは、カードの差出人のメールアドレスです。Gmail の Web 版も画像を表示するかどうかを差出人のアドレスごとに覚えます。ドメインごとにしないのは、`gmail.com` のように大勢が使うドメインでは、1 人を許可すると同じドメインの全員の画像が読み込まれるためです。アドレスは小文字にそろえて比べ、アプリの設定ディレクトリの `image-senders.json` に保存します。一覧はアカウントごとに分けず、全アカウントで共有します。

`From` ヘッダーは送り主が自由に書けるので、許可したアドレスを名乗るメールでも画像が読み込まれます。画像を読み込むかどうかを決めるときは、SPF や DKIM の結果を見ていません。名乗ったメールで画像が読み込まれても、送り主に伝わるのは開封したことと IP アドレスで、スクリプトが動かないことは変わりません。

### 送信元の検証と BIMI

件名の下には 5 つの検証結果を並べます。SPF・DKIM・DMARC は Gmail の判定を、BIMI と VMC は Mail Crew が自分で確かめた結果を示します。

SPF・DKIM・DMARC は `get_message` で取ったメッセージの `Authentication-Results` ヘッダー（RFC 8601）から読みます。このヘッダーは送り主も書けるので、読むのはいちばん上の 1 つだけです。受け取ったサーバーはこのヘッダーを先頭に追加するので、いちばん上が Gmail の付けたものになります。そのうえで、認証サービスの識別子が `mx.google.com` のときだけ読みます。DKIM の署名が複数あるときは、1 つでも合格していれば合格とします。

BIMI は `get_brand` で確かめます。アプリは本文を取ったあと、Gmail の DMARC の判定が `pass` のときだけこれを呼びます。それ以外のときは、呼ばずに「DMARC の条件を満たさないため確認していません」と出します。Rust 側はメモリにある本文を使うので、Gmail API の使用量は増えません。手順は draft-brand-indicators-for-message-identification-15 の 7 章と、draft-fetch-validation-vmc-wchuang-12 に沿っています。

1. Gmail の DMARC の判定が `pass` で、`From` のアドレスが 1 つだけのメールに限ります。そのうえで差出人のドメインの DMARC レコードを DNS で引き、ポリシーが `reject` か、`pct=100` の `quarantine` であることを確かめます。
2. `default._bimi.<ドメイン>` の BIMI レコードを引き、なければ組織ドメイン（Public Suffix List で決める）のものを引きます。`lps=` があれば、ローカルパートから作ったセレクタのレコードも引きます。`BIMI-Selector` ヘッダーは読みません。仕様はこのヘッダーが DKIM の署名に含まれていることを求めていますが、Gmail の判定からは署名の対象が分からないためです。
3. レコードの `a=` にある証明書を HTTPS で取り、BIMI のルート証明書までの連鎖、有効期間、証明書と発行元の CA にある BIMI の拡張鍵用途（1.3.6.1.5.5.7.3.31）、証明書が示す CRL での失効、SAN にレコードのドメインがあることを確かめます。
4. ロゴは証明書の logotype 拡張に埋め込まれた SVG です。`l=` の SVG は取りに行きません。証明書のない BIMI レコードでは、ロゴを表示しません。

証明書の `subject:markType` が `Registered Mark` か `Government Mark` なら VMC、それ以外なら CMC と表示します。Certificate Transparency の証明は確かめていません。

ロゴは `data:` URL でアプリに渡します。アプリはそれを WKWebView の `<img>` で描きます。AppKit のドキュメントには、`NSImage(data:)` が読める形式として SVG が挙がっていないためです。`<img>` に読み込んだ SVG では、スクリプトも外部の読み込みも動きません。この WKWebView は本文と同じく JavaScript を切り、`WKContentRuleList` と CSP で、自分の文書と `data:` のほかは何も読み込ませません（`TrustRow.swift` の `LogoFrame`）。`data:image/svg+xml` で始まらない URL はロゴとして扱いません。設定で BIMI がオフなら、件名の下には SPF・DKIM・DMARC だけを出します。

結果は差出人のアドレスごとに、証明書の検証結果は URL ごとに、1 時間メモリに持ちます。DNS が答えなかったときと、証明書や CRL を取れなかったときは結果を持たず、次に同じ差出人のカードを表示したときに確かめ直します。設定で BIMI をオフにすると、Rust 側は DNS にも証明書のサーバーにも問い合わせません。

ルート証明書は、AuthIndicators Working Group が [Mark Certificate Issuer Information](https://bimigroup.org/vmc-issuers/) に載せている認証局のものを `core/bimi-roots/` に置いています。一覧にある DigiCert、Entrust、GlobalSign と、SSL.com の ECC と RSA の 2 つで、合わせて 5 つです。一覧が変わったら、このディレクトリの PEM と `bimi.rs` の `ROOTS` を合わせて変えます。

## Pull Request のビルド

`mailcrew/` を変更する Pull Request では、GitHub Actions が配布用のディスクイメージを作ります。成功すると、成果物へのダウンロードリンクを Pull Request にコメントします。push のたびに新しいコメントを投稿し、前回までのコメントは outdated として畳みます。

リンク先のダウンロードには GitHub へのログインが必要です。成果物には保持期限があり、過ぎるとリンクは無効になります（期限はコメントに書かれます）。

## テスト

手元と CI で走るものは次のとおりです。

| 対象 | 実行 | 場所 |
|---|---|---|
| Rust 側の単体テスト | `cargo test --features ffi`（`core/`） | 各ファイルの `mod tests` |
| Swift から Rust を呼ぶ部分 | `swift test`（`core/swift/`） | `core/swift/Tests/` |
| アプリ | `xcodebuild test` | `MailCrewTests/` |

`cargo test` が確かめるのは、ネットワークを使わない部分です。PKCE とリダイレクトの読み取り（`auth.rs`）、アカウントの一覧の更新と旧形式の読み取り（`credentials.rs`）、複数アカウントのカードを新しい順に並べて切る処理（`accounts.rs`）、ビューの保存と読み込み（`views.rs`）、画像を常に表示する差出人の保存と読み込み（`image_senders.rs`）、MIME の解析と文字コード（ISO-2022-JP、Shift_JIS を含む）、文字コードの宣言がないか誤っているときの推測、RFC 2047 のヘッダー（`mime.rs`）、サニタイズで消えるものと残るもの、埋め込み画像の置き換え（`sanitize.rs`）、添付ファイルの一覧と埋め込み画像の見分け（`mime.rs`、`card.rs`）、保存するファイル名、上書きしない書き出し、開いてよい種類（`attachments.rs`）、スレッドからカードへの変換、メッセージから本文への変換（`card.rs`）、上限で断られたときの判定と待ち時間（`gmail.rs`）、開いてよいリンク（`links.rs`）、Gmail の検証結果の読み取り（`authres.rs`）、DMARC のポリシーと BIMI レコードの判定、テスト用に作ったマーク証明書の検証（`bimi.rs`）、設定の保存と読み込み（`preferences.rs`）、アプリに渡すエラーと型の変換（`ffi.rs`）を見ます。

`swift test` は、生成したバインディングを通して Swift から Rust を呼べることを確かめます。ビューと画像を常に表示する差出人と設定の保存と読み込み、開いてよいリンクの判定、Rust のエラーが `MailCrewError` として届くことを見ます。Google とキーチェーンには触れません。

アプリのテストは、`AppModel` を Rust 側の代わり（`MailCrewTests/FakeCore.swift`）につないで確かめます。起動時にどの画面を出すか、全アカウントの受信トレイを取ること、答えなかったアカウントの知らせ、Google が受け付けなくなったアカウントのカードを外すこと、最後のアカウントが外れたらログイン画面に戻ること、ログインが始められなかったときと中止したときの表示を見ます。カードの日付の書き方（`CardDate`）も確かめます。本文については、カードが見えたときにそのカードと前後 2 枚の本文を取ること、取れなかったら抜粋のまま次に見えたときに取り直すこと、外部画像の「表示する」と「この差出人は常に表示」、開くリンクの種類、CSP とルールと遷移の判定を見ます。カードのボタンについては、完了、タグ、あとで読む、スターがカードのアカウントとメッセージ ID を Rust 側に渡すこと、押すとカードがすぐ列から外れ、失敗すると元の場所に戻ること、列が空になると次を読み込むこと、新しいタグは作ってから付けること、スターが失敗すると元に戻ること、タグの一覧の絞り込みを見ます。検証結果と添付ファイル（`TrustAndAttachmentsTests`）については、SPF・DKIM・DMARC・BIMI・VMC の表示の言葉と色の種類、DMARC が `pass` のときだけカードのアカウントで `get_brand` を呼ぶこと、BIMI がオフなら何も問い合わせないこと、ロゴは `data:image/svg+xml` の URL だけを出すこと、ロゴの文書の CSP、添付ファイルの「開く」が開いてよい種類のときだけ Rust 側を呼び、書き出したファイルを OS に渡すこと、「保存」がダウンロード フォルダに書き、保存した名前を知らせること、失敗したときの知らせ、サイズの書き方を見ます。サイドバーと設定（`SidebarAndSettingsTests`）については、タブの並びと、タブごとに Rust 側に渡す取得の条件を見ます。あとで読むでは、あとで読むのメールを取ります。ビューでは、ビューのアカウントのうちログインしているものと、検索条件で取ります。アカウントがすべて外れたビューでは、何も取りません。空の列の言葉と列の終わりの言葉、まとめて完了がアカウントごとに 1 回ずつアーカイブしてから次を読み込むこと、一部や全部が失敗したときの扱いも見ます。設定では、アカウントの削除と、最後のアカウントを消したときにログイン画面に戻ることを見ます。追加が始められなかったときと中止したときの表示も見ます。ビューは、保存するときに名前と検索条件の前後の空白を除くこと、名前がないときの知らせ、表示中のビューを直したら取り直すこと、消したら受信トレイに戻ること、保存できなかったときを見ます。画像を常に表示する差出人の削除と、BIMI のオン・オフの保存、そのときの検証結果の出方も見ます。引っ張って再読み込み（`PullGestureTests`）では、先頭のカードの位置で始めて右へ動かし、336 ポイント以上引っ張ってから離したときだけ再読み込みすることを見ます。届く前に離したとき、戻してから離したとき、ほかの位置から始めた操作、左や縦に動き始めた操作、取り消された操作では再読み込みしないことも見ます。引っ張りが 784 ポイントに近づくほど遅くなることと、先頭のカードの位置を越えて戻すと列がスクロールに戻ることも見ます。サイドバーのある画面で SwiftUI の横スクロールの中から `NSScrollView` が見つかることと、先頭のカードの位置を 0 と読むことも確かめます。カードの列のスクロール（`StripScrollTests`）では、カード 1 枚分の 15% を越えて進むと次のカードに移り、それより短いと元のカードに戻ることを見ます。長く進んだときに、15% を越えて入ったカードの枚数だけ移ることも見ます。`MessageWebViewTests` は本物の WKWebView に本文を入れ、この Mac の上に立てた HTTP サーバーに届いた読み込みを数えます。許可するまで何も届かないこと、許可すると画像だけが届くこと、スクリプトが動かないこと、高さと縮小を確かめます。

届かない範囲もあります。

- 本物の Google とのやりとり。ログイン画面（`SignIn` はブラウザと Google を待つ。設定からの追加も同じ）、トークンの発行と更新、Gmail API の応答とエラー
- キーチェーンからの移し替えとそのダイアログ、以前のバージョンからのログインの引き継ぎ（`accounts.json` の保存と読み出し、権限が 0600 になることは `cargo test` で見ます）
- 既定のブラウザやメールアプリでリンクが開くこと、添付ファイルが既定のアプリで開くこと、ダウンロード フォルダに保存されること
- 本物の DNS や、証明書と CRL を置いたサーバーとのやりとり。DigiCert や Entrust が発行した本物のマーク証明書の検証。WebKit が描いたロゴ
- 画面の見た目と、スクロールの動き（トラックパッドで引っ張ったときの動き、振動、再読み込みを含む）
- コード署名していない配布物を macOS が警告する挙動

これらは実機で確かめるしかありません。リリースの前には少なくとも次を手で確かめます。

1. 初回のログインから受信トレイが表示されるまで
2. 完了、タグ（既存と新規）、あとで読む、スターの結果が Gmail の Web 版に反映されること
3. まとめて完了で次の 50 件が表示されること
4. ニュースレターなど HTML のメールのレイアウトと、「表示する」で外部画像が出ること
5. 本文中のリンクが既定のブラウザで開くこと
6. アプリを終了して起動し直しても、ログイン、ビュー、画像を常に表示する差出人が続いていること
7. 前のバージョンから更新して起動しても、ログインが続いていること。0.8.0 からの更新ではキーチェーンの確認が 1 回だけ出て、`accounts.json` ができ、キーチェーンの `com.ysaeki.mailcrew` の項目が消えていること。その次の更新では確認が出ないこと
8. 設定から 2 つ目のアカウントを追加すると、両方のメールが新しい順に 1 つの列に並び、それぞれのボタンが正しいアカウントに反映されること
9. ビューのアカウントと検索条件で、並ぶメールが絞り込まれること
10. 0.1 のバージョンでログインしていた環境で、ログインし直さずに受信トレイが表示されること
11. Gmail の Web 版でロゴが出る差出人のメールで、件名の下の検証結果がすべて緑になり、名前の横に同じロゴが出ること。設定で「BIMI のロゴを表示する」をオフにすると、ロゴと BIMI・VMC の項目が消えること
12. PDF や画像の添付ファイルが「開く」で既定のアプリで開き、「保存」でダウンロード フォルダに入ること。同じものをもう一度保存すると、番号の付いた別のファイルになること
13. 署名の画像やロゴを本文に埋め込んだメール（Outlook や Apple Mail から送ったものなど）で、画像が本文の中に表示されること
14. 先頭のカードの位置で列を右へ大きく引っ張って離すと再読み込みし、古いメールから勢いよく戻ってきたときやマウスのホイールでは再読み込みしないこと
15. トラックパッドで列をカードの幅の 2 割ほど横にスワイプして離すと次のカードに移り、ごく小さな動きでは元のカードに戻ること。止まったカードの左右の余白が以前と同じであること

### CI

`mailcrew/` を変更する Pull Request では、2 系統のジョブが同時に走ります。`rust checks (macos-latest / windows-latest)`（`core/` の `cargo fmt`、`clippy`、`cargo test`）と、`app (macos-latest)`（`core/swift/` のバインディングの生成と `swift test`、アプリの `xcodebuild test`、Release のビルドとディスクイメージの作成とアップロード）です。Windows で `core/` を確かめるのは、Windows 向けのコードをビルドできる状態に保つためです。

バージョンの検査は `.github/workflows/mailcrew-version.yml` という別のワークフローです。ラベルの付け外しでも走る必要があり、`mailcrew.yml` に足すとビルドとテストまで走ってしまうためです。

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `project.yml` の `MARKETING_VERSION` です。`CFBundleShortVersionString` と `CFBundleVersion` はこれに従います。

`main` ブランチで `MARKETING_VERSION` が上がると、GitHub Actions が `mailcrew-v<version>` タグの Release を作り、ディスクイメージを添付します。上げずにマージした場合、タグが既に存在するため Release は作られません。

上げ忘れは `mailcrew-version.yml` が Pull Request を赤くして止めます。バージョンが base ブランチと同じか古いときに落ちます。リリースしない変更（ドキュメントだけ、CI だけなど）には `no-release` ラベルを付けてください。

依存ライブラリの新しいバージョンは Dependabot が週に 1 回確かめ、見つかると Pull Request を作ります（設定は `.github/dependabot.yml`。対象は `core/` の Cargo）。この Pull Request には `no-release` ラベルが付き、ビルドとテストがすべて通ると `.github/workflows/dependabot-merge.yml` がマージします。手で確かめる項目は次にこのアプリを変更する Pull Request でまとめて確認します。
