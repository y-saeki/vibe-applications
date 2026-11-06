# Mail Crew の開発

Mail Crew をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

フロントエンドはフレームワークを使わない TypeScript で、シェルは Tauri v2（Rust）です。Gmail との通信、ログイン、メール本文の解析とサニタイズは Rust 側が受け持ち、ページは受け取ったカードを並べるだけです。

Mail Crew は画面を SwiftUI で作るネイティブ版（macOS）に作り直している途中です。Rust 側の中身は `core/` のクレートにまとめてあり、今の Tauri 版（`src-tauri/`）とネイティブ版（`MailCrew/`）の両方が使います。詳しくは「Rust の中身と Swift からの呼び出し」と「ネイティブ版」を参照してください。
配布対象は macOS（Apple Silicon）と Windows（x64）の 2 つで、どちらも GitHub Actions でビルドします。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)（stable） | `rustup` でインストール |
| Node.js 22 以降 | |
| [pnpm](https://pnpm.io/) 12 以降 | CI もこのバージョンを使います |
| macOS: Xcode Command Line Tools | `xcode-select --install` |
| macOS: Xcode | ネイティブ版（`MailCrew/`）と、Swift から Rust を呼ぶ部分（`core/swift/`）に使います |
| macOS: [XcodeGen](https://github.com/yonaskolb/XcodeGen) | ネイティブ版の Xcode プロジェクトを `project.yml` から作ります。`brew install xcodegen` |
| Windows: Visual Studio Build Tools | 「C++ によるデスクトップ開発」を選択。Rust は MSVC ツールチェーンを既定にする |

詳細は [Tauri の Prerequisites](https://v2.tauri.app/start/prerequisites/) を参照してください。動かして確かめるには、README の「初回の準備」で作る OAuth クライアントも要ります。

## セットアップと実行

```sh
cd mailcrew
pnpm install          # フロントエンドの依存を取得
pnpm tauri dev        # 開発モードで起動(フロントエンドを監視し、Rust 側を自動ビルド)
```

配布用のビルドは次のとおりです。

```sh
pnpm tauri build
```

成果物は `src-tauri/target/release/bundle/` 以下に出力されます。

- macOS: `macos/Mail Crew.app`、`dmg/Mail Crew_<version>_aarch64.dmg`（Apple Silicon 向け）
- Windows: `nsis/Mail Crew_<version>_x64-setup.exe`

アプリの名前は「Mail Crew」で、`tauri.conf.json` の `productName` がこれを決めます。ディレクトリ名、Cargo と npm のパッケージ名、`identifier`（`com.ysaeki.mailcrew`）、Release のタグ（`mailcrew-v<version>`）は `mailcrew` のままです。Release に添付すると、ファイル名の空白は GitHub が `.` に置き換えます。

## 検証コマンド

```sh
pnpm typecheck                     # TypeScript の型検査(src/ と tests/)
pnpm build                         # フロントエンドのバンドル(dist/)
pnpm test:e2e                      # フロントエンドの E2E テスト
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
cd core && cargo fmt --check && cargo clippy --all-targets --features ffi -- -D warnings && cargo test --features ffi
```

Swift から Rust を呼ぶ部分は、Mac でだけ確かめられます。

```sh
cd core/swift
./build.sh                                        # Rust の静的ライブラリと Swift のバインディングを作る
swift test -Xlinker -L"$(./build.sh --lib-dir)"   # Swift から呼んで確かめる
```

ネイティブ版は、Xcode のプロジェクト（`MailCrew.xcodeproj`）を作ってから、ビルドとテストをします。プロジェクトはコミットしないので、最初に一度作ります。作り直すのは、`project.yml` が変わったときだけです。

```sh
xcodegen generate         # project.yml から MailCrew.xcodeproj を作る
open MailCrew.xcodeproj   # Xcode で開き、⌘R で実行、⌘U でテスト
xcodebuild test -project MailCrew.xcodeproj -scheme MailCrew -destination 'platform=macOS,arch=arm64'
```

ふだんは、pull したら Xcode で ⌘R を押すだけです。

- スキームのビルド前の処理が `core/swift/build.sh` を呼び、Rust のライブラリとバインディングを作ります。Rust 側が変わっていなければ、cargo も `build.sh` も何もしないので、数秒で終わります。
- Xcode から呼ばれるときは PATH に cargo がないので、`build.sh` は `~/.cargo/bin` を足し、それでもなければ `rustup which cargo` で探します。Homebrew の rustup は cargo を PATH に置かないためです。
- スキームのビルド前の処理は、失敗してもビルドを止めません。そこで、`build.sh` が失敗したときは出力を `core/target/xcode-build.failed` に残します。続くアプリのターゲットの最初のビルド フェーズ（Check the Rust build）が、それをビルドのエラーとして出します。
- `build.sh` は、生成したバインディングのうち中身が変わったファイルだけを書き換えます。同じ中身で書き直すと、毎回パッケージがビルドし直されるためです。
- `MailCrew/` と `MailCrewTests/` は同期フォルダ（Xcode 16 の synchronized folders）です。Swift のファイルを足したり消したりしても、`xcodegen generate` は要りません。

`pnpm test:e2e` は初回だけブラウザの取得が必要です。

```sh
pnpm exec playwright install chromium webkit
```

## 構成

| 場所 | 役割 |
|---|---|
| `core/src/auth.rs` | Google へのログイン（OAuth 2.0、ループバックのリダイレクトと PKCE）、トークンの取得と更新 |
| `core/src/credentials.rs` | アカウント情報の保存先（macOS のキーチェーン、Windows の資格情報マネージャー）と、以前のバージョンの保存の形からの移し替え |
| `core/src/accounts.rs` | ログイン中のアカウントの一覧と、全アカウントの受信トレイを 1 つの列にまとめる処理 |
| `core/src/gmail.rs` | 1 アカウント分の Gmail API の呼び出し。受信トレイのスレッド、ラベル、ラベルの付け外し |
| `core/src/views.rs` | カスタムビューの保存先（アプリの設定ディレクトリの `views.json`） |
| `core/src/image_senders.rs` | 外部画像を常に表示する差出人の保存先（アプリの設定ディレクトリの `image-senders.json`） |
| `core/src/card.rs` | スレッドから 1 枚のカードに載せる内容を組み立てる |
| `core/src/mime.rs` | Gmail が返す MIME 構造の読み取り。本文の選択、文字コード（宣言がないか誤っているときは推測する）、RFC 2047 のヘッダー |
| `core/src/sanitize.rs` | HTML 本文のサニタイズ（ammonia）と、埋め込み画像の `cid:` を `data:` URL に置き換える処理 |
| `core/src/attachments.rs` | 添付ファイルの書き出し。保存するファイル名、上書きしない書き出し先、開いてよい種類 |
| `core/src/service.rs` | アプリができることの一覧（`Core`）。Tauri のコマンドとネイティブ版のバインディングは、どちらもここを呼ぶ |
| `core/src/links.rs` | アプリの外で開いてよいリンク（http、https、mailto） |
| `core/src/ffi.rs` | ネイティブ版が Swift から呼ぶためのバインディング（UniFFI） |
| `core/swift/` | `core/` を Swift から使うためのパッケージ（`MailCrewCore`）と、そのテスト |
| `project.yml` | ネイティブ版の Xcode プロジェクトの定義（XcodeGen） |
| `MailCrew/AppModel.swift` | ネイティブ版の状態。どの画面を出すか、ログイン、受信トレイの取得と失敗の扱い、ビュー、設定。`MailCrew` を呼ぶのはここだけ |
| `MailCrew/SignInView.swift`、`InboxView.swift`、`CardView.swift` | ネイティブ版の画面。ログイン、サイドバーとカードの列と列の終わり、カード |
| `MailCrew/PullToRefresh.swift` | ネイティブ版の、引っ張って再読み込み。指の状態と引っ張った量の判定と、すき間に出す表示 |
| `MailCrew/SettingsView.swift` | ネイティブ版の設定のウィンドウ。アカウント、ビュー、画像を常に表示する差出人、BIMI |
| `MailCrew/CardToolbar.swift` | ネイティブ版のカードの下のボタンの行。タグの一覧と、添付ファイルの一覧 |
| `MailCrew/TrustRow.swift` | ネイティブ版の件名の下の検証結果と、BIMI のロゴを描く WKWebView |
| `MailCrew/MessageFrame.swift` | ネイティブ版で HTML 本文を表示するときの決まり。文書と CSP、`WKContentRuleList` のルール、遷移の判定、テキスト本文のリンク |
| `MailCrew/MessageWebView.swift` | ネイティブ版の HTML 本文の WKWebView。高さと縮小の計算、リンクを外で開く処理 |
| `MailCrewTests/` | ネイティブ版の単体テスト |
| `src-tauri/src/commands.rs` | ページから呼べるコマンドの一覧。中身は `Core` に任せ、ブラウザやファイルを開く処理だけを受け持つ |
| `src-tauri/src/links.rs` | webview がアプリ以外のページへ移動しないようにする。メール中のリンクは既定のアプリで開く |
| `core/src/authres.rs` | Gmail が付けた `Authentication-Results` ヘッダーから、SPF・DKIM・DMARC の結果を読み取る |
| `core/src/bimi.rs` | BIMI のレコードとマーク証明書（VMC・CMC）の検証。ロゴは証明書から取り出す |
| `core/bimi-roots/` | マーク証明書の検証に使うルート証明書 |
| `core/src/preferences.rs` | 設定の保存先（アプリの設定ディレクトリの `preferences.json`）。いまは BIMI のロゴを表示するかどうかだけ |
| `src/inbox.ts` | カードの列、ボタンの動作、まとめて完了 |
| `src/views.ts` | サイドバーに並ぶビューと、ビューから取得条件を作る処理 |
| `src/sidebar.ts` | サイドバーの開閉。閉じたかどうかは `localStorage` に保存する |
| `src/settings.ts` | 設定画面。アカウントの追加と削除、ビューの作成・編集・削除、画像を常に表示する差出人の削除 |
| `src/image-senders.ts` | 画像を常に表示する差出人の一覧。受信トレイと設定画面が同じ一覧を使う |
| `src/message-frame.ts` | 本文の表示。HTML は iframe に入れる |
| `src/overlay-scrollbar.ts` | カードの本文に重ねて描くスクロールバー |
| `src/tag-picker.ts` | タグを選ぶポップアップ |
| `src/attachment-menu.ts` | 添付ファイルの一覧のポップアップ。開くと保存のボタン |
| `src/trust.ts` | 件名の下に並ぶ検証結果の表示 |
| `src/preferences.ts` | 設定の値。受信トレイと設定画面が同じ値を使う |

### Rust の中身と Swift からの呼び出し

`core/` は、画面に関わらない処理をまとめたライブラリのクレート（`mailcrew-core`）です。Tauri には依存しません。入口は `Core`（`core/src/service.rs`）で、Tauri 版が `State` として持っていたものと、各コマンドの処理をここに移しました。ブラウザを開く、ファイルを OS に渡す、ダウンロード フォルダを探すといった OS の機能だけは、使う側が受け持ちます。たとえばログインは、`start_sign_in` が待ち受けのポートと同意画面の URL を用意し、使う側がその URL をブラウザで開いてから `finish_sign_in` を待ちます。

設定ファイル（`views.json`、`image-senders.json`、`preferences.json`）を置くディレクトリは `Core::new` に渡します。Tauri 版はアプリの設定ディレクトリ（macOS では `~/Library/Application Support/com.ysaeki.mailcrew`）を渡しています。ネイティブ版も同じディレクトリを渡せば、設定がそのまま引き継がれます。アカウント情報は、これまでどおりキーチェーンの同じ項目に入っています。

ネイティブ版は `core/src/ffi.rs` を通して `Core` を呼びます。これは `ffi` フィーチャーを付けたときだけビルドされ、[UniFFI](https://mozilla.github.io/uniffi-rs/) が Swift のバインディングを生成します。

- `MailCrew` が `Core` にあたるオブジェクトです。メソッドは Swift の `async throws` になり、処理は Rust 側で持つ Tokio のランタイムで動きます。失敗は `MailCrewError` で返り、ケースの種類は Tauri 版の `kind` と同じです。
- ログインは `startSignIn` が返す `SignIn` の `url()` をブラウザで開き、`finish()` を待ちます。
- 受け渡す型（カード、本文、添付ファイルなど）は `ffi.rs` の中で別に定義し、Rust の型から変換します。Tauri 版のページが受け取る JSON の形を変えないためです。添付ファイルの取得元やメールの差出人のように、Rust の中でだけ使う値は Swift に渡しません。

`core/swift/build.sh` は、`core/` を Apple Silicon 向けの静的ライブラリ（`libmailcrew_core.a`）としてビルドし、そこから C のヘッダーと Swift のソースを生成します。生成したファイルはコミットしません。`core/swift/Package.swift` はそれらを `MailCrewCore` というライブラリにまとめます。

### ネイティブ版

ネイティブ版は SwiftUI の macOS アプリで、`project.yml` から XcodeGen で Xcode のプロジェクトを作ります。プロジェクトの形式は、同期フォルダを使うために `xcode16_0` にしています。`MailCrewCore`（`core/swift/`）をローカルのパッケージとして使い、`build.sh` が作った `libmailcrew_core.a` をリンクします。Apple Silicon 向けだけで、対応する macOS は 14 以降です。`build.sh` は Rust のライブラリも同じ macOS 14 向けにビルドします。

バンドル識別子は Tauri 版と同じ `com.ysaeki.mailcrew` です。キーチェーンの項目と設定のディレクトリが、そのまま引き継がれます。今は証明書で署名せず、アドホック署名でビルドします。

`MailCrewCore` の `View` と `Label` は、SwiftUI の同じ名前の型と衝突します。そのため画面のファイルは `MailCrewCore` を読み込まず、Rust 側を呼ぶのは `AppModel` だけにしています。画面が使う型（`Card` など）には、`AppModel.swift` で別名を付けています。

作り直しは #298 の「進め方の案」の順に進めています。今あるのは、ログイン、受信トレイのカードの列、カードの見出しと本文、カードのボタンとタグ、検証結果と BIMI のロゴ、添付ファイル、サイドバーとビュー、設定、まとめて完了、引っ張って再読み込みまでです。引っ張って再読み込みは、ネイティブ版にしかありません。設定は、macOS のほかのアプリと同じく、別のウィンドウ（⌘,）で開きます。Tauri 版では、受信トレイと同じウィンドウの中に出しています。配布しているのも、まだ Tauri 版です。

列を先頭のカードより右へ引っ張り、指を離すと再読み込みします（`PullToRefresh.swift`）。引っ張りとして扱うのは、列が先頭のカードの位置にあるときに始まり、最初に右へ動いた操作だけです。指の状態はスクロールのイベントの `phase`（`began`、`changed`、`ended`、`cancelled`）で分かります。`phase` のないマウスのホイールは数えません。古いメールから勢いよく戻ってきて先頭を越えても、その操作はほかの位置で始まっているので、跳ね返るだけです。

`NSScrollView` の端のバウンスは、指をどれだけ動かしても数十ポイントで止まり、引っ張るには短すぎます。そのため、引っ張っている間は、イベントのモニター（`NSEvent.addLocalMonitorForEvents`）がスクロールのイベントを `NSScrollView` に渡さず、カードを自分でずらします。ずらす量は指の動いた量から決め、最初は指にほぼついていき、560 ポイントに近づくほど遅くなります。`began` と `ended` のイベントは `NSScrollView` にも渡します。始まりを見たまま終わりを見ないと、`NSScrollView` が操作の途中だと思い続けるためです。引っ張ったあとの慣性のイベント（`momentumPhase` があるもの）も渡しません。指を左へ戻して先頭のカードの位置を越えると、その先は列が普通にスクロールします。

カードの位置を決めるときは、クリップ ビューの `bounds.origin.x` に `contentInsets.left` を足します。サイドバーは列の上に浮いていて、`NSScrollView` はサイドバーの下まで広がります。そのため、内容にはサイドバーの幅の余白が付き、先頭のカードの位置でも `bounds.origin.x` はその幅だけマイナスになります。

240 ポイント以上引っ張ったところで指を離すと再読み込みし、届く前に戻したときは何もしません。引っ張っている間は、開いたすき間に矢印と「引っ張って再読み込み」を出します。言葉は、すき間が言葉の幅より広くなってから出します。240 ポイントに届くと、塗りつぶした矢印と「離すと再読み込み」をアクセントの色で出し、トラックパッドを振動させます（`NSHapticFeedbackManager`）。戻して届かなくなったときも、弱く振動させます。離して再読み込みが始まると、すき間を 120 ポイント開けたまま「読み込み中」を出し、終わったら閉じます。

`core/` のバージョンは `0.0.0` のままです。単独では配布しないためで、リリースのバージョンは今までどおり `src-tauri/tauri.conf.json` が決めます。依存ライブラリは `src-tauri/Cargo.lock` と `core/Cargo.lock` の両方に載ります。Dependabot はそれぞれのディレクトリを別に確かめます。

### ログインとアカウント情報

OAuth クライアントは利用者が自分の Google Cloud プロジェクトで作ったものを使い、配布物には何も埋め込みません。ログインは「インストール型アプリの OAuth 2.0」の流れです。`127.0.0.1` の空いているポートで待ち受け、既定のブラウザで同意画面を開き、戻ってきたリダイレクトから認可コードを受け取ります。PKCE（S256）と `state` を使い、5 分で待ち受けをやめます。スコープは `gmail.modify` の 1 つです。

クライアント ID、クライアント シークレット、リフレッシュトークン、メールアドレスは、アカウントごとに 1 つの JSON にまとめて OS の資格情報ストアに入れます（サービス名 `com.ysaeki.mailcrew`）。項目の分け方は OS で異なります。

- macOS: 全アカウントの JSON を配列にして、`all-accounts` という 1 つの項目に入れます。キーチェーンは、更新したアプリが項目を初めて読むときに、項目ごとに確認を出します。配布物は Apple が発行した証明書で署名していないので、キーチェーンはビルドごとに別のアプリとみなします。自分で作った証明書で署名しても、この扱いは変わりません。Apple が公開している securityd のソースでは、Apple が発行していない証明書の署名は cdhash で区別されます。項目を 1 つにすれば、アカウントがいくつあっても確認は更新ごとに 1 回で済みます。
- Windows: 資格情報マネージャーは 1 項目を 2,560 バイトまでに制限しています。全アカウントをまとめると数アカウントで超えるので、アカウントごとに `google-account:<メールアドレス>` という項目に入れます。資格情報ストアには保存した項目の一覧を問い合わせられないため、メールアドレスを追加した順に並べた一覧を `accounts` という別の項目に入れます。

古い形からの移し替えは起動時に行います。0.3 までの macOS は Windows と同じ形でした。`all-accounts` がなければ古い形から読み、`all-accounts` に入れてから古い項目を消します。キーチェーンが削除を断った項目は残しますが、`all-accounts` ができたあとは読みません。0.1 までのバージョンは 1 アカウントを `google-account` に入れていました。`accounts` がなく `google-account` があれば、同じように新しい形へ移して古い項目を消します。

設定画面からアカウントを追加するとき、クライアント ID とシークレットを空欄のままにすると、Rust 側は最初のアカウントのクライアントを使います。クライアントはページに渡しません。

アクセストークンはアカウントごとにメモリにだけ持ち、期限の 1 分前か、API が 401 を返したときに取り直します。Google があるアカウントのリフレッシュトークンを受け付けなくなる（`invalid_grant`）と、そのアカウントの保存した情報を消し、ページには `signedOut` を返します。ページはアカウントの一覧を取り直し、ほかのアカウントが残っていれば受信トレイのまま知らせを出し、残っていなければログイン画面に戻ります。

### 受信トレイとボタン

`list_inbox` は、対象のアカウント（ビューで選んだもの。「受信トレイ」と「あとで読む」のタブなら全アカウント）ごとに `threads.list`（50 件。ビューに検索条件があれば `q` に渡す）で並びを取り、各スレッドを `threads.get`（`format=metadata`、`From` と `Subject` のヘッダーだけ）で 8 件ずつ並行して取ります。アカウントどうしは並行して取り、全アカウントのカードを日時の新しい順に並べて 50 件に切ります。各アカウントの新しい 50 件をそろえてから切るので、全体の新しい 50 件が漏れません。`threads.list` の `labelIds` は、「あとで読む」のタブなら `SetAside` ラベル、それ以外のタブなら `INBOX` です。`SetAside` ラベルがまだないアカウントは、ラベルを作らずに空の一覧を返します。カードには下書きを除いた最新の 1 通の送信者、件名、日時と、スレッドの既読やスターの状態を載せ、どのアカウントのスレッドかを `account` に持たせます。

本文と添付ファイルの一覧は、カードが画面に近づいたときにページが `get_message` で取ります。Rust 側は `messages.get`（`format=full`）で 1 通を取り、本文を選んでサニタイズして返します。取るまでのカードにはスニペットが出ます。取った本文はアカウントごとに 200 通までメモリに持ちます。

Gmail API の上限は、1 ユーザーあたり 1 分に 6,000 単位です（[Usage limits](https://developers.google.com/workspace/gmail/api/reference/quota)）。`threads.get` は `format=full` だと 120 単位を使うので、50 件のスレッドを本文ごと取ると、それだけで上限を超えます。`format=metadata` なら 40 単位なので、一覧の読み込みは `threads.list` の 10 単位と合わせて 2,010 単位です。本文の `messages.get`（`format=full`）は 1 通 60 単位で、画面に近づいたカードの分だけかかります。

さらにアカウントごとに、取得したスレッドのカードを `threads.list` が返す `historyId` と一緒にメモリに持ちます。再読み込みやビューの切り替えで `threads.get` を呼ぶのは、`historyId` が変わったスレッドだけです。持つのは 1 アカウントあたり 1,000 件までで、超えたら直近の一覧にないものを捨てます。

Google が上限を理由に断ったとき（429、または理由が `rateLimitExceeded` や `userRateLimitExceeded` の 403）は、上の Usage limits のページが勧めるとおり、1、2、4 秒と倍に延ばした間（それぞれ最大 1 秒を足し、32 秒で頭打ち）を置いて、同じリクエストを 6 回まで送り直します。待つ時間は合わせて 1 分を少し超えます。それでも断られたら、少し待ってから再読み込みするよう知らせます。

一部のアカウントだけが失敗したときは、ほかのアカウントのカードと、失敗したアカウントとその理由（`failures`）を一緒に返します。ページは理由を知らせに出します。

ボタンの操作はカードの `account` のアカウントに送ります。まとめて完了はアカウントごとに `archive` を呼びます。一部のアカウントだけが失敗したときは、受信トレイを読み込み直してから理由を知らせます。タグの一覧には、カードと同じアカウントのラベルが並びます。

ボタンを押すと、カードを読み込んだときのメッセージ ID を対象に `messages.batchModify` を呼びます。スレッド単位の `threads.modify` を使わないのは、カードを表示している間に届いた返信まで受信トレイから外さないためです。

| ボタン | 付けるラベル | 外すラベル |
|---|---|---|
| 完了 / まとめて完了 | なし | `INBOX`、`UNREAD`（「あとで読む」のタブでは `SetAside` も） |
| タグ | 選んだラベル | `INBOX`（「あとで読む」のタブでは `SetAside` も） |
| あとで読む | `SetAside` | `INBOX` |
| スター（付ける） | 表示中の 1 通に `STARRED` | なし |
| スター（外す） | なし | スレッドの全メッセージから `STARRED` |

「あとで読む」のタブでは、カードに「あとで読む」のボタンを出しません。ページは `archive` と `apply_label` に、カードを読み込んだタブの `mailbox`（`inbox` か `setAside`）を渡します。

ページはボタンを押した時点でカードを消し、Rust 側が失敗を返したら元の位置に戻します。

### 添付ファイルと埋め込み画像

添付ファイルの一覧に載るのは、ファイル名があるパートのうち、HTML 本文が `cid:` で表示している画像を除いたものです。以前は `Content-Disposition: inline` で `Content-ID` があるパートを一律に除いていましたが、本文が表示しない画像まで見えなくなるので、本文が実際に参照しているかで分けています。一覧の各項目はメッセージの中のパート ID（`partId`）で区別します。

埋め込み画像は `get_message` の中で取ります。HTML 本文が `cid:` で参照する `image/*` のパートを、`messages.attachments.get`（1 回 20 単位）で取り、`data:` URL にしてサニタイズのときに `src` と `background` を置き換えます。Gmail が本文と一緒に中身を返したパートは、追加の呼び出しなしで使います。取るのは 1 通につき 20 枚、合わせて 5 MB までで、それを超える画像と、取れなかった画像は表示しません。取れなかった画像があるときは、本文をメモリに残さず、次にカードを表示したときに取り直します。iframe の CSP は前から `img-src data:` を許しているので、ネットワークへの読み込みは増えず、開いたことは送り主に伝わりません。

添付ファイルを開くとき（`open_attachment`）と保存するとき（`save_attachment`）は、Rust 側がメモリにある本文から添付ファイルを探し、`messages.attachments.get`（20 単位）で中身を取ります。ファイル名は `attachments.rs` の `safe_file_name` で、パスの区切りや OS が受け付けない文字を `_` にし、先頭と末尾の `.` と空白を取り除いてから使います。末尾の `.` や空白は Windows がエラーを出さずに取り除くので、残すと拡張子が変わってしまいます。

- 開く: 一時ディレクトリの `mailcrew-attachments/<乱数>/` に書き出し、OS が選ぶアプリで開きます（opener プラグインの `open_path` を Rust 側から呼びます）。このディレクトリは起動のたびに消します。開けるのは、拡張子が `attachments.rs` の `OPENABLE` にある種類（文書、画像、音声、動画）だけです。開いたときに何が起きるかは拡張子で決まり、拡張子は送り主が決められるためです。プログラム、スクリプト、ショートカット、インストーラ、アーカイブ、HTML や SVG、マクロ付きの Office 文書は、ページに「開く」を出さず、Rust 側も断ります。
- 保存: ダウンロード フォルダに書き出します。同じ名前のファイルがあれば `名前 (2).pdf` のように番号を付け、既存のファイルは上書きしません。

### ウィンドウの見た目

macOS では `src-tauri/tauri.macos.conf.json` で次のように設定しています。

- タイトルバーをページに重ねる（`titleBarStyle: Overlay`、`hiddenTitle`）
- 信号ボタンをツールバーの行の高さに合わせる（`trafficLightPosition`）
- ウィンドウを透明にして、その下に標準素材を敷く（`windowEffects` の `underWindowBackground`）

Tauri の OS 別の設定ファイルは配列を丸ごと置き換えるので、`app.windows` の要素は `tauri.conf.json` の値も含めて書き直してあります。ウィンドウの大きさなどを変えるときは、両方のファイルを変えます。

`trafficLightPosition` の `y` は、信号ボタンの中心がツールバーの行の中央にそろうように、実機のスクリーンショットで測って決めた値です。ボタンの大きさや置き方は macOS のバージョンで変わることがあるので、macOS のメジャーアップデートのあとは信号ボタンの位置を見て、ずれていれば `y` を直します。Tauri には実行中に信号ボタンを動かす公開 API がないため、位置はこの設定の値だけで決まります。

ページは `get_session` が返す `platform` を `<html data-platform>` に入れます。macOS のときだけ背景を透明にし、ツールバーの先頭を信号ボタンの右へずらします。タイトルバーがないので、ツールバー全体と、設定画面とログイン画面の余白を `data-tauri-drag-region` でドラッグ領域にしています。そのために `src-tauri/capabilities/default.json` で `core:window:allow-start-dragging` と `core:window:allow-internal-toggle-maximize` を許可しています。

カードの上に浮く操作部品（サイドバー、ツールバーのボタン、タグを選ぶポップアップ、通知）は、`.glass`（`backdrop-filter` のぼかし）で半透明にします。カードは中身なので不透明のままです。Apple の Human Interface Guidelines が Liquid Glass を操作とナビゲーションの層に限っているのに合わせています。Windows ではウィンドウを透明にせず、Mica などの素材も使っていません。

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

### カードの本文

本文の iframe は自分ではスクロールせず、メールの高さに合わせて伸ばします。スクロールするのはそれを包むカードの本文の領域だけです。

- 横方向には何もスクロールしません。カードの本文の領域と iframe の文書は横のはみ出しを隠し、カードより幅の広いメールは CSS の `zoom` で縮めて収めます。カードの上で横にスワイプしても、カードの中ではなくカードの列が動きます。
- OS のスクロールバーは隠し、`overlay-scrollbar.ts` が細いバーを本文の上に重ねて描きます。バーはスクロールしている間だけ表示されます。
- 高さは iframe の `load` と、ウィンドウの大きさが変わったときに測り直します。

### HTML 本文の安全対策

HTML 本文は 2 段階で守ります。

1. Rust 側（`sanitize.rs`）で、実行や送信につながるもの（`script`、イベントハンドラ、`form`、`iframe`、`object`、`embed`、`meta`、`base`、`link`、`svg`、`javascript:` のリンク、相対 URL）を取り除きます。メールのレイアウトに要る `style` 要素と `style` 属性、表組みの属性は残します。外部画像を読み込むかどうかも、ここで判定します。埋め込み画像の `cid:` は `data:` URL に置き換え、取っていない画像の `cid:` は取り除きます。
2. ページ側（`message-frame.ts`）で、`sandbox="allow-same-origin"` だけを付けた iframe に `srcdoc` で入れます。`allow-scripts` がないので、サニタイズをすり抜けたものがあってもスクリプトは動きません。iframe の文書には専用の Content-Security-Policy（`default-src 'none'; style-src 'unsafe-inline'; img-src data:`）を付け、利用者が「表示する」を押したときと、差出人が「画像を常に表示する差出人」にあるときだけ、`img-src` に `https:` と `http:` を加えます。`referrer` は送りません。

`allow-same-origin` は、ページが iframe の中のリンクのクリックを拾い、幅の広いメールを縮小するために付けています。`allow-scripts` と同時に付けると sandbox の意味がなくなるため、この 2 つは決して並べないでください。

リンクは iframe の中で遷移させず、Rust 側が `http`、`https`、`mailto` だけを既定のアプリで開きます。経路はエンジンによって 2 つあります。

- WebView2（Windows）: ページが iframe の中のクリックを拾い、`open_link` コマンドに渡します。
- WKWebView（macOS）: WebKit はスクリプトを禁じた iframe の中でページのイベントリスナーを呼ばないので、クリックは iframe の遷移として始まります。WKWebView はどのフレームの遷移も Rust 側に問い合わせるので、`links.rs` がそこで遷移を止め、リンクを既定のアプリで開きます。ただし WebKit は問い合わせる前にアプリ本体の CSP の `frame-src` を確かめ、許されない遷移は何も表示せずに取りやめます。そのため macOS だけ `tauri.macos.conf.json` で `frame-src` に `http:`、`https:`、`mailto:` を加えています。Tauri は macOS でこのファイルを `tauri.conf.json` に重ねます。iframe の遷移は `links.rs` がすべて止めるので、外部のページが iframe に読み込まれることはありません。Windows では iframe が遷移しないので、`frame-src` は `'self'` のままです。

WebView2 はトップレベルの遷移しか問い合わせないため、ページの側でクリックを拾う処理も必要です。opener プラグインの権限はページに与えていません（`capabilities/default.json`）。

ネイティブ版では、2 段階目を iframe ではなく WKWebView で作ります（`MessageFrame.swift`、`MessageWebView.swift`）。守る決まりは Tauri 版と同じです。

- `WKWebpagePreferences.allowsContentJavaScript` を切るので、メールの中のスクリプトは動きません。高さを測るスクリプトはアプリが入れるもので、メールとは別のコンテンツワールドで動きます。
- 文書には Tauri 版と同じ CSP と `no-referrer` を付けます。それに加えて `WKContentRuleList` で、文書（`about:`）と `data:` 以外の読み込みをすべて止めます。「表示する」を押したときと、差出人が「画像を常に表示する差出人」にあるときは、http と https の画像だけを通すルールに替えて読み込み直します。CSP とルールのどちらか一方だけでも外部の読み込みは止まります。ルールの正規表現には `|` が使えないので、許す URL は 1 つずつ別のルールにしています。
- WebView が読み込んでよいのは、アプリが `loadHTMLString` で渡した文書（`about:blank`）だけです。ほかの遷移はすべて止めます。クリックしたリンクのうち、`is_openable_link`（Rust 側）が通した http、https、mailto だけを既定のアプリで開きます。新しいウィンドウは作りません。
- Force Touch のリンクのプレビューは、リンク先を読み込むので切っています。データストアは保存しないもの（`nonPersistent()`）を使い、許可した画像が送った Cookie はアプリを終了すると消えます。
- WebView はスクロールしません。スクロールのイベントはすべてカードの本文に渡し、本文の高さに合わせて WebView を伸ばします。幅の広いメールは Tauri 版と同じく CSS の `zoom` で縮めます。高さは本文の最後に置いた目印の位置で測るので、メールが `html` や `body` に `height: 100%` を付けていても WebView ほど高くなりません。

`srcdoc` の文書はアプリ本体の CSP も引き継ぎます。Tauri は既定で `style-src` に nonce を足しますが、nonce があると `'unsafe-inline'` が無視され、メールのインラインスタイルがすべて効かなくなります。そのため `tauri.conf.json` の `dangerousDisableAssetCspModification` で `style-src` だけを対象から外しています。アプリ本体の `img-src` に `https:` と `http:` があるのも、許可した外部画像を通すためです。

### 画像を常に表示する差出人

「この差出人は常に表示」で覚えるのは、カードの差出人のメールアドレスです。Gmail の Web 版も画像を表示するかどうかを差出人のアドレスごとに覚えます。ドメインごとにしないのは、`gmail.com` のように大勢が使うドメインでは、1 人を許可すると同じドメインの全員の画像が読み込まれるためです。アドレスは小文字にそろえて比べ、アプリの設定ディレクトリの `image-senders.json` に保存します。一覧はアカウントごとに分けず、全アカウントで共有します。

`From` ヘッダーは送り主が自由に書けるので、許可したアドレスを名乗るメールでも画像が読み込まれます。画像を読み込むかどうかを決めるときは、SPF や DKIM の結果を見ていません。名乗ったメールで画像が読み込まれても、送り主に伝わるのは開封したことと IP アドレスで、スクリプトが動かないことは変わりません。

### 送信元の検証と BIMI

件名の下には 5 つの検証結果を並べます。SPF・DKIM・DMARC は Gmail の判定を、BIMI と VMC は Mail Crew が自分で確かめた結果を示します。

SPF・DKIM・DMARC は `get_message` で取ったメッセージの `Authentication-Results` ヘッダー（RFC 8601）から読みます。このヘッダーは送り主も書けるので、読むのはいちばん上の 1 つだけです。受け取ったサーバーはこのヘッダーを先頭に追加するので、いちばん上が Gmail の付けたものになります。そのうえで、認証サービスの識別子が `mx.google.com` のときだけ読みます。DKIM の署名が複数あるときは、1 つでも合格していれば合格とします。

BIMI は `get_brand` で確かめます。ページは本文を取ったあとにこのコマンドを呼び、Rust 側はメモリにある本文を使うので、Gmail API の使用量は増えません。手順は draft-brand-indicators-for-message-identification-15 の 7 章と、draft-fetch-validation-vmc-wchuang-12 に沿っています。

1. Gmail の DMARC の判定が `pass` で、`From` のアドレスが 1 つだけのメールに限ります。そのうえで差出人のドメインの DMARC レコードを DNS で引き、ポリシーが `reject` か、`pct=100` の `quarantine` であることを確かめます。
2. `default._bimi.<ドメイン>` の BIMI レコードを引き、なければ組織ドメイン（Public Suffix List で決める）のものを引きます。`lps=` があれば、ローカルパートから作ったセレクタのレコードも引きます。`BIMI-Selector` ヘッダーは読みません。仕様はこのヘッダーが DKIM の署名に含まれていることを求めていますが、Gmail の判定からは署名の対象が分からないためです。
3. レコードの `a=` にある証明書を HTTPS で取り、BIMI のルート証明書までの連鎖、有効期間、証明書と発行元の CA にある BIMI の拡張鍵用途（1.3.6.1.5.5.7.3.31）、証明書が示す CRL での失効、SAN にレコードのドメインがあることを確かめます。
4. ロゴは証明書の logotype 拡張に埋め込まれた SVG です。`l=` の SVG は取りに行きません。証明書のない BIMI レコードでは、ロゴを表示しません。

証明書の `subject:markType` が `Registered Mark` か `Government Mark` なら VMC、それ以外なら CMC と表示します。Certificate Transparency の証明は確かめていません。

ロゴはページに `data:` URL で渡し、`<img>` で表示します。`<img>` に読み込んだ SVG では、スクリプトも外部の読み込みも動きません。

ネイティブ版もロゴを WKWebView の `<img>` で描きます。AppKit のドキュメントには、`NSImage(data:)` が読める形式として SVG が挙がっていないためです。この WKWebView は本文と同じく JavaScript を切り、`WKContentRuleList` と CSP で、自分の文書と `data:` のほかは何も読み込ませません（`TrustRow.swift` の `LogoFrame`）。`data:image/svg+xml` で始まらない URL はロゴとして扱いません。ネイティブ版は DMARC が `pass` のときだけ `get_brand` を呼び、それ以外のときは、呼ばずに「DMARC の条件を満たさないため確認していません」と出します。設定で BIMI がオフなら、件名の下には SPF・DKIM・DMARC だけを出します。

結果は差出人のアドレスごとに、証明書の検証結果は URL ごとに、1 時間メモリに持ちます。DNS が答えなかったときと、証明書や CRL を取れなかったときは結果を持たず、次に同じ差出人のカードを表示したときに確かめ直します。設定で BIMI をオフにすると、Rust 側は DNS にも証明書のサーバーにも問い合わせません。

ルート証明書は、AuthIndicators Working Group が [Mark Certificate Issuer Information](https://bimigroup.org/vmc-issuers/) に載せている認証局のものを `core/bimi-roots/` に置いています。一覧にある DigiCert、Entrust、GlobalSign と、SSL.com の ECC と RSA の 2 つで、合わせて 5 つです。一覧が変わったら、このディレクトリの PEM と `bimi.rs` の `ROOTS` を合わせて変えます。

## Pull Request のビルド

`mailcrew/` を変更する Pull Request では、GitHub Actions が macOS / Windows の両方をビルドします。両方が成功すると、成果物へのダウンロードリンクを Pull Request にコメントします。push のたびに新しいコメントを投稿し、前回までのコメントは outdated として畳みます。

リンク先のダウンロードには GitHub へのログインが必要です。成果物には保持期限があり、過ぎるとリンクは無効になります（期限はコメントに書かれます）。

## テスト

手元と CI で走るものは次のとおりです。

| 対象 | 実行 | 場所 |
|---|---|---|
| フロントエンド（`src/`）の E2E | `pnpm test:e2e` | `tests/e2e/` |
| Rust 側の単体テスト | `cargo test`（`core/` と `src-tauri/`） | 各ファイルの `mod tests` |
| Swift から Rust を呼ぶ部分 | `swift test`（`core/swift/`） | `core/swift/Tests/` |
| ネイティブ版 | `xcodebuild test` | `MailCrewTests/` |

`cargo test` が確かめるのは、ネットワークを使わない部分です。PKCE とリダイレクトの読み取り（`auth.rs`）、アカウントの一覧の更新と旧形式の読み取り（`credentials.rs`）、複数アカウントのカードを新しい順に並べて切る処理（`accounts.rs`）、ビューの保存と読み込み（`views.rs`）、画像を常に表示する差出人の保存と読み込み（`image_senders.rs`）、MIME の解析と文字コード（ISO-2022-JP、Shift_JIS を含む）、文字コードの宣言がないか誤っているときの推測、RFC 2047 のヘッダー（`mime.rs`）、サニタイズで消えるものと残るもの、埋め込み画像の置き換え（`sanitize.rs`）、添付ファイルの一覧と埋め込み画像の見分け（`mime.rs`、`card.rs`）、保存するファイル名、上書きしない書き出し、開いてよい種類（`attachments.rs`）、スレッドからカードへの変換、メッセージから本文への変換と JSON の形（`card.rs`）、上限で断られたときの判定と待ち時間（`gmail.rs`）、開いてよいリンク（`core/src/links.rs`）、webview に許す遷移（`links.rs`）、Gmail の検証結果の読み取り（`authres.rs`）、DMARC のポリシーと BIMI レコードの判定、テスト用に作ったマーク証明書の検証（`bimi.rs`）、設定の保存と読み込み（`preferences.rs`）、ネイティブ版に渡すエラーと型の変換（`ffi.rs`）を見ます。

`swift test` は、生成したバインディングを通して Swift から Rust を呼べることを確かめます。ビューと画像を常に表示する差出人と設定の保存と読み込み、開いてよいリンクの判定、Rust のエラーが `MailCrewError` として届くことを見ます。Google とキーチェーンには触れません。

ネイティブ版のテストは、`AppModel` を Rust 側の代わり（`MailCrewTests/FakeCore.swift`）につないで確かめます。起動時にどの画面を出すか、全アカウントの受信トレイを取ること、答えなかったアカウントの知らせ、Google が受け付けなくなったアカウントのカードを外すこと、最後のアカウントが外れたらログイン画面に戻ること、ログインが始められなかったときと中止したときの表示を見ます。カードの日付の書き方（`CardDate`）も、Tauri 版の `formatDate` と同じ例で確かめます。本文については、カードが見えたときにそのカードと前後 2 枚の本文を取ること、取れなかったら抜粋のまま次に見えたときに取り直すこと、外部画像の「表示する」と「この差出人は常に表示」、開くリンクの種類、CSP とルールと遷移の判定を見ます。カードのボタンについては、完了、タグ、あとで読む、スターがカードのアカウントとメッセージ ID を Rust 側に渡すこと、押すとカードがすぐ列から外れ、失敗すると元の場所に戻ること、列が空になると次を読み込むこと、新しいタグは作ってから付けること、スターが失敗すると元に戻ること、タグの一覧の絞り込みを見ます。検証結果と添付ファイル（`TrustAndAttachmentsTests`）については、SPF・DKIM・DMARC・BIMI・VMC の表示の言葉と色の種類が Tauri 版と同じこと、DMARC が `pass` のときだけカードのアカウントで `get_brand` を呼ぶこと、BIMI がオフなら何も問い合わせないこと、ロゴは `data:image/svg+xml` の URL だけを出すこと、ロゴの文書の CSP、添付ファイルの「開く」が開いてよい種類のときだけ Rust 側を呼び、書き出したファイルを OS に渡すこと、「保存」がダウンロード フォルダに書き、保存した名前を知らせること、失敗したときの知らせ、サイズの書き方を見ます。サイドバーと設定（`SidebarAndSettingsTests`）については、タブの並びと、タブごとに Rust 側に渡す取得の条件を見ます。あとで読むでは、あとで読むのメールを取ります。ビューでは、ビューのアカウントのうちログインしているものと、検索条件で取ります。アカウントがすべて外れたビューでは、何も取りません。空の列の言葉と列の終わりの言葉、まとめて完了がアカウントごとに 1 回ずつアーカイブしてから次を読み込むこと、一部や全部が失敗したときの扱いも見ます。設定では、アカウントの削除と、最後のアカウントを消したときにログイン画面に戻ることを見ます。追加が始められなかったときと中止したときの表示も見ます。ビューは、保存するときに名前と検索条件の前後の空白を除くこと、名前がないときの知らせ、表示中のビューを直したら取り直すこと、消したら受信トレイに戻ること、保存できなかったときを見ます。画像を常に表示する差出人の削除と、BIMI のオン・オフの保存、そのときの検証結果の出方も見ます。引っ張って再読み込み（`PullGestureTests`）では、先頭のカードの位置で始めて右へ動かし、240 ポイント以上引っ張ってから離したときだけ再読み込みすることを見ます。届く前に離したとき、戻してから離したとき、ほかの位置から始めた操作、左や縦に動き始めた操作、取り消された操作では再読み込みしないことも見ます。引っ張りが 560 ポイントに近づくほど遅くなることと、先頭のカードの位置を越えて戻すと列がスクロールに戻ることも見ます。サイドバーのある画面で SwiftUI の横スクロールの中から `NSScrollView` が見つかることと、先頭のカードの位置を 0 と読むことも確かめます。`MessageWebViewTests` は本物の WKWebView に本文を入れ、この Mac の上に立てた HTTP サーバーに届いた読み込みを数えます。許可するまで何も届かないこと、許可すると画像だけが届くこと、スクリプトが動かないこと、高さと縮小を確かめます。本物のログイン（`SignIn` はブラウザと Google を待つ。設定からの追加も同じ）、画面の見た目、スクロールの動き（トラックパッドで引っ張ったときの動き、振動、再読み込みを含む）、リンクを開いたあとのブラウザ、WebKit が描いたロゴ、DNS とマーク証明書、添付ファイルを開いたアプリは確かめられません。

### フロントエンドの E2E テスト

[Playwright](https://playwright.dev/) が `src/` をそのままブラウザで動かします。Rust 側だけを `tests/e2e/harness/backend.ts` が差し替え、`@tauri-apps/api/mocks` の `mockIPC` が `invoke` を受けます。ログイン中のアカウント、`list_inbox` が順に返すカードと失敗させるアカウント、ラベルの一覧、保存済みのビュー、失敗させるコマンドはテストごとに指定でき、ページが呼んだコマンドは順に記録されます。ハーネスは `sign_in` と `sign_out` に合わせてアカウントの一覧を変え、`signedOut` で失敗させたアカウントは Rust 側と同じく一覧から外します。

ブラウザは 2 つ動かします。配布している 2 つの webview に対応させたものです。

| Playwright の project | 実機での相手 |
|---|---|
| `chromium` | Windows の WebView2 |
| `webkit` | macOS の WKWebView |

ハーネスが返す本文は Rust のサニタイズを通っていません。`tests/e2e/message.spec.ts` はこれを利用して、`script` やイベントハンドラを含む HTML でもスクリプトが動かないこと、「表示する」を押すまで外部画像へのリクエストが 1 件も出ないことを、iframe だけで確かめています。本文中のリンクは、Chromium ではクリックが `open_link` に渡ること、WebKit ではリンク先への iframe の遷移になることを確かめます。このテストはアプリ本体の CSP の下で動かすので、`frame-src` が遷移を止めていないことも確かめられます。その遷移を `links.rs` が止めて外で開くところは webview の外なので、テストでは確かめられません。

開発サーバは Content-Security-Policy を送りません。`tests/e2e/csp.spec.ts` と本文中のリンクのテストだけが、`tauri.conf.json` と同じポリシーの下でページを開きます。WebKit では macOS と同じく `tauri.macos.conf.json` を重ねたポリシーを使います。`csp.spec.ts` はメールのスタイルが効くこと、許可した画像が通ることを確かめます。

届かない範囲もあります。

- 本物の Google とのやりとり。ログイン画面、トークンの発行と更新、Gmail API の応答とエラー
- OS の資格情報ストアへの保存と読み出し、macOS のキーチェーンのダイアログ、以前のバージョンからのログインの引き継ぎ
- 既定のブラウザやメールアプリでリンクが開くこと、添付ファイルが既定のアプリで開くこと、ダウンロード フォルダに保存されること
- 本物の DNS や、証明書と CRL を置いたサーバーとのやりとり。DigiCert や Entrust が発行した本物のマーク証明書の検証
- Tauri が実際に付ける CSP（nonce を含む）。テストは `tauri.conf.json`（WebKit では `tauri.macos.conf.json` を重ねたもの）の値を送るだけです
- コード署名していない配布物を各 OS が警告する挙動と、Windows インストーラの画面
- ウィンドウの素材、信号ボタンの位置、ツールバーでのウィンドウのドラッグ。どれも Tauri のウィンドウの機能で、ブラウザでは動きません

これらは実機で確かめるしかありません。リリースの前には少なくとも次を手で確かめます。

1. 初回のログインから受信トレイが表示されるまで
2. 完了、タグ（既存と新規）、あとで読む、スターの結果が Gmail の Web 版に反映されること
3. まとめて完了で次の 50 件が表示されること
4. ニュースレターなど HTML のメールのレイアウトと、「表示する」で外部画像が出ること
5. 本文中のリンクが既定のブラウザで開くこと
6. アプリを終了して起動し直しても、ログイン、ビュー、画像を常に表示する差出人が続いていること
7. macOS で前のバージョンから更新して起動しても、ログインが続いていること。2 回目以降の更新では、キーチェーンの確認が 1 回だけ出ること
7. 設定から 2 つ目のアカウントを追加すると、両方のメールが新しい順に 1 つの列に並び、それぞれのボタンが正しいアカウントに反映されること
8. ビューのアカウントと検索条件で、並ぶメールが絞り込まれること
9. 0.1 のバージョンでログインしていた環境で、ログインし直さずに受信トレイが表示されること
10. macOS で、信号ボタンがツールバーの行の中央にそろい、ウィンドウの背景に素材が出ること。ツールバーをドラッグするとウィンドウが動き、ダブルクリックするとウィンドウが拡大すること
11. Gmail の Web 版でロゴが出る差出人のメールで、件名の下の検証結果がすべて緑になり、名前の横に同じロゴが出ること。設定で「BIMI のロゴを表示する」をオフにすると、ロゴと BIMI・VMC の項目が消えること
12. PDF や画像の添付ファイルが「開く」で既定のアプリで開き、「保存」でダウンロード フォルダに入ること。同じものをもう一度保存すると、番号の付いた別のファイルになること
13. 署名の画像やロゴを本文に埋め込んだメール（Outlook や Apple Mail から送ったものなど）で、画像が本文の中に表示されること

### CI

`mailcrew/` を変更する Pull Request では、4 系統のジョブが同時に走ります。`frontend`（E2E と型検査）、`rust checks (macos-latest / windows-latest)`（`src-tauri/` と `core/` の `cargo fmt`、`clippy`、`cargo test`）、`native app (macos-latest)`（`core/swift/` のバインディングの生成と `swift test`、ネイティブ版のビルドと `xcodebuild test`）、`build (macos-latest / windows-latest)`（`pnpm tauri build` と成果物のアップロード）です。E2E テストが落ちると、その run に `playwright-report` が添付されます。CI では失敗したテストを 1 回だけ再実行しますが、再実行で通ったもの（flaky）も失敗として扱います。

バージョンの検査は `.github/workflows/mailcrew-version.yml` という別のワークフローです。ラベルの付け外しでも走る必要があり、`mailcrew.yml` に足すと macOS / Windows のビルドまで走ってしまうためです。

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `src-tauri/tauri.conf.json` の `version` です。`package.json` と `src-tauri/Cargo.toml` の `version` も同じ値にそろえ、`Cargo.toml` を書き換えたら `cargo update -p mailcrew` で `Cargo.lock` も追随させてください。

`main` ブランチで `src-tauri/tauri.conf.json` の `version` が上がると、GitHub Actions が `mailcrew-v<version>` タグの Release を作り、上記の 2 つを添付します。上げずにマージした場合、タグが既に存在するため Release は作られません。

上げ忘れは `mailcrew-version.yml` が Pull Request を赤くして止めます。バージョンが base ブランチと同じか古いとき、4 つのファイルのバージョンが食い違っているときに落ちます。リリースしない変更（ドキュメントだけ、CI だけなど）には `no-release` ラベルを付けてください。

依存ライブラリの新しいバージョンは Dependabot が週に 1 回確かめ、見つかると Pull Request を作ります（設定は `.github/dependabot.yml`）。この Pull Request には `no-release` ラベルが付き、ビルドとテストがすべて通ると `.github/workflows/dependabot-merge.yml` がマージします。手で確かめる項目は次にこのアプリを変更する Pull Request でまとめて確認します。

Tauri は、Rust のクレートと npm のパッケージで major と minor がそろっていることを求めます。そのため Dependabot の設定では、Rust 側の `tauri`・`tauri-*` と npm 側の `@tauri-apps/*` を、それぞれ `tauri` という別のグループにしています。minor が変わる更新では片側だけが上がるので、このグループの Pull Request は落ちます。そのときは両側を同じ minor にそろえる Pull Request を手で作ります。
