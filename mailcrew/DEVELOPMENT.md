# Mail Crew の開発

Mail Crew をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

フロントエンドはフレームワークを使わない TypeScript で、シェルは Tauri v2（Rust）です。Gmail との通信、ログイン、メール本文の解析とサニタイズは Rust 側が受け持ち、ページは受け取ったカードを並べるだけです。
配布対象は macOS（Apple Silicon）と Windows（x64）の 2 つで、どちらも GitHub Actions でビルドします。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)（stable） | `rustup` でインストール |
| Node.js 22 以降 | |
| [pnpm](https://pnpm.io/) 12 以降 | CI もこのバージョンを使います |
| macOS: Xcode Command Line Tools | `xcode-select --install` |
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
```

`pnpm test:e2e` は初回だけブラウザの取得が必要です。

```sh
pnpm exec playwright install chromium webkit
```

## 構成

| 場所 | 役割 |
|---|---|
| `src-tauri/src/auth.rs` | Google へのログイン（OAuth 2.0、ループバックのリダイレクトと PKCE）、トークンの取得と更新 |
| `src-tauri/src/credentials.rs` | アカウント情報の保存先（macOS のキーチェーン、Windows の資格情報マネージャー） |
| `src-tauri/src/gmail.rs` | Gmail API の呼び出し。受信トレイのスレッド、ラベル、ラベルの付け外し |
| `src-tauri/src/card.rs` | スレッドから 1 枚のカードに載せる内容を組み立てる |
| `src-tauri/src/mime.rs` | Gmail が返す MIME 構造の読み取り。本文の選択、文字コード（宣言がないか誤っているときは推測する）、RFC 2047 のヘッダー |
| `src-tauri/src/sanitize.rs` | HTML 本文のサニタイズ（ammonia） |
| `src-tauri/src/commands.rs` | ページから呼べるコマンドの一覧 |
| `src-tauri/src/links.rs` | webview がアプリ以外のページへ移動しないようにする。メール中のリンクは既定のアプリで開く |
| `src/inbox.ts` | カードの列、ボタンの動作、まとめて完了 |
| `src/message-frame.ts` | 本文の表示。HTML は iframe に入れる |
| `src/overlay-scrollbar.ts` | カードの本文に重ねて描くスクロールバー |
| `src/tag-picker.ts` | タグを選ぶポップアップ |

### ログインとアカウント情報

OAuth クライアントは利用者が自分の Google Cloud プロジェクトで作ったものを使い、配布物には何も埋め込みません。ログインは「インストール型アプリの OAuth 2.0」の流れです。`127.0.0.1` の空いているポートで待ち受け、既定のブラウザで同意画面を開き、戻ってきたリダイレクトから認可コードを受け取ります。PKCE（S256）と `state` を使い、5 分で待ち受けをやめます。スコープは `gmail.modify` の 1 つです。

クライアント ID、クライアント シークレット、リフレッシュトークン、メールアドレスは、1 つの JSON にまとめて OS の資格情報ストアに入れます（サービス名 `com.ysaeki.mailcrew`）。アクセストークンはメモリにだけ持ち、期限の 1 分前か、API が 401 を返したときに取り直します。Google がリフレッシュトークンを受け付けなくなる（`invalid_grant`）と、保存した情報を消してログイン画面に戻します。

### 受信トレイとボタン

`threads.list`（`labelIds=INBOX`、50 件）で並びを取り、各スレッドを `threads.get`（`format=full`）で 8 件ずつ並行して取ります。カードには下書きを除いた最新の 1 通を載せます。

ボタンを押すと、カードを読み込んだときのメッセージ ID を対象に `messages.batchModify` を呼びます。スレッド単位の `threads.modify` を使わないのは、カードを表示している間に届いた返信まで受信トレイから外さないためです。

| ボタン | 付けるラベル | 外すラベル |
|---|---|---|
| 完了 / まとめて完了 | なし | `INBOX`、`UNREAD` |
| タグ | 選んだラベル | `INBOX` |
| あとで読む | `SetAside` | `INBOX` |
| スター（付ける） | 表示中の 1 通に `STARRED` | なし |
| スター（外す） | なし | スレッドの全メッセージから `STARRED` |

ページはボタンを押した時点でカードを消し、Rust 側が失敗を返したら元の位置に戻します。

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

1. Rust 側（`sanitize.rs`）で、実行や送信につながるもの（`script`、イベントハンドラ、`form`、`iframe`、`object`、`embed`、`meta`、`base`、`link`、`svg`、`javascript:` のリンク、相対 URL）を取り除きます。メールのレイアウトに要る `style` 要素と `style` 属性、表組みの属性は残します。外部画像を読み込むかどうかも、ここで判定します。
2. ページ側（`message-frame.ts`）で、`sandbox="allow-same-origin"` だけを付けた iframe に `srcdoc` で入れます。`allow-scripts` がないので、サニタイズをすり抜けたものがあってもスクリプトは動きません。iframe の文書には専用の Content-Security-Policy（`default-src 'none'; style-src 'unsafe-inline'; img-src data:`）を付け、利用者が「表示する」を押したときだけ `img-src` に `https:` と `http:` を加えます。`referrer` は送りません。

`allow-same-origin` は、ページが iframe の中のリンクのクリックを拾い、幅の広いメールを縮小するために付けています。`allow-scripts` と同時に付けると sandbox の意味がなくなるため、この 2 つは決して並べないでください。

リンクは iframe の中で遷移させず、Rust 側が `http`、`https`、`mailto` だけを既定のアプリで開きます。経路はエンジンによって 2 つあります。

- WebView2（Windows）: ページが iframe の中のクリックを拾い、`open_link` コマンドに渡します。
- WKWebView（macOS）: WebKit はスクリプトを禁じた iframe の中でページのイベントリスナーを呼ばないので、クリックは iframe の遷移として始まります。WKWebView はどのフレームの遷移も Rust 側に問い合わせるので、`links.rs` がそこで遷移を止め、リンクを既定のアプリで開きます。ただし WebKit は問い合わせる前にアプリ本体の CSP の `frame-src` を確かめ、許されない遷移は何も表示せずに取りやめます。そのため macOS だけ `tauri.macos.conf.json` で `frame-src` に `http:`、`https:`、`mailto:` を加えています。Tauri は macOS でこのファイルを `tauri.conf.json` に重ねます。iframe の遷移は `links.rs` がすべて止めるので、外部のページが iframe に読み込まれることはありません。Windows では iframe が遷移しないので、`frame-src` は `'self'` のままです。

WebView2 はトップレベルの遷移しか問い合わせないため、ページの側でクリックを拾う処理も必要です。opener プラグインの権限はページに与えていません（`capabilities/default.json`）。

`srcdoc` の文書はアプリ本体の CSP も引き継ぎます。Tauri は既定で `style-src` に nonce を足しますが、nonce があると `'unsafe-inline'` が無視され、メールのインラインスタイルがすべて効かなくなります。そのため `tauri.conf.json` の `dangerousDisableAssetCspModification` で `style-src` だけを対象から外しています。アプリ本体の `img-src` に `https:` と `http:` があるのも、許可した外部画像を通すためです。

## Pull Request のビルド

`mailcrew/` を変更する Pull Request では、GitHub Actions が macOS / Windows の両方をビルドします。両方が成功すると、成果物へのダウンロードリンクを Pull Request にコメントします。push のたびに新しいコメントを投稿し、前回までのコメントは outdated として畳みます。

リンク先のダウンロードには GitHub へのログインが必要です。成果物には保持期限があり、過ぎるとリンクは無効になります（期限はコメントに書かれます）。

## テスト

手元と CI で走るものは 2 つです。

| 対象 | 実行 | 場所 |
|---|---|---|
| フロントエンド（`src/`）の E2E | `pnpm test:e2e` | `tests/e2e/` |
| Rust 側の単体テスト | `cargo test` | 各ファイルの `mod tests` |

`cargo test` が確かめるのは、ネットワークを使わない部分です。PKCE とリダイレクトの読み取り（`auth.rs`）、MIME の解析と文字コード（ISO-2022-JP、Shift_JIS を含む）、文字コードの宣言がないか誤っているときの推測、RFC 2047 のヘッダー（`mime.rs`）、サニタイズで消えるものと残るもの（`sanitize.rs`）、スレッドからカードへの変換と JSON の形（`card.rs`）、開いてよいリンク（`commands.rs`）、webview に許す遷移（`links.rs`）を見ます。

### フロントエンドの E2E テスト

[Playwright](https://playwright.dev/) が `src/` をそのままブラウザで動かします。Rust 側だけを `tests/e2e/harness/backend.ts` が差し替え、`@tauri-apps/api/mocks` の `mockIPC` が `invoke` を受けます。ログインの有無、`list_inbox` が順に返すカード、ラベルの一覧、失敗させるコマンドはテストごとに指定でき、ページが呼んだコマンドは順に記録されます。

ブラウザは 2 つ動かします。配布している 2 つの webview に対応させたものです。

| Playwright の project | 実機での相手 |
|---|---|
| `chromium` | Windows の WebView2 |
| `webkit` | macOS の WKWebView |

ハーネスが返す本文は Rust のサニタイズを通っていません。`tests/e2e/message.spec.ts` はこれを利用して、`script` やイベントハンドラを含む HTML でもスクリプトが動かないこと、「表示する」を押すまで外部画像へのリクエストが 1 件も出ないことを、iframe だけで確かめています。本文中のリンクは、Chromium ではクリックが `open_link` に渡ること、WebKit ではリンク先への iframe の遷移になることを確かめます。このテストはアプリ本体の CSP の下で動かすので、`frame-src` が遷移を止めていないことも確かめられます。その遷移を `links.rs` が止めて外で開くところは webview の外なので、テストでは確かめられません。

開発サーバは Content-Security-Policy を送りません。`tests/e2e/csp.spec.ts` と本文中のリンクのテストだけが、`tauri.conf.json` と同じポリシーの下でページを開きます。WebKit では macOS と同じく `tauri.macos.conf.json` を重ねたポリシーを使います。`csp.spec.ts` はメールのスタイルが効くこと、許可した画像が通ることを確かめます。

届かない範囲もあります。

- 本物の Google とのやりとり。ログイン画面、トークンの発行と更新、Gmail API の応答とエラー
- OS の資格情報ストアへの保存と読み出し、macOS のキーチェーンのダイアログ
- 既定のブラウザやメールアプリでリンクが開くこと
- Tauri が実際に付ける CSP（nonce を含む）。テストは `tauri.conf.json`（WebKit では `tauri.macos.conf.json` を重ねたもの）の値を送るだけです
- コード署名していない配布物を各 OS が警告する挙動と、Windows インストーラの画面

これらは実機で確かめるしかありません。リリースの前には少なくとも次を手で確かめます。

1. 初回のログインから受信トレイが表示されるまで
2. 完了、タグ（既存と新規）、あとで読む、スターの結果が Gmail の Web 版に反映されること
3. まとめて完了で次の 50 件が表示されること
4. ニュースレターなど HTML のメールのレイアウトと、「表示する」で外部画像が出ること
5. 本文中のリンクが既定のブラウザで開くこと
6. アプリを終了して起動し直しても、ログインが続いていること

### CI

`mailcrew/` を変更する Pull Request では、3 系統のジョブが同時に走ります。`frontend`（E2E と型検査）、`rust checks (macos-latest / windows-latest)`（`cargo fmt`、`clippy`、`cargo test`）、`build (macos-latest / windows-latest)`（`pnpm tauri build` と成果物のアップロード）です。E2E テストが落ちると、その run に `playwright-report` が添付されます。CI では失敗したテストを 1 回だけ再実行しますが、再実行で通ったもの（flaky）も失敗として扱います。

バージョンの検査は `.github/workflows/mailcrew-version.yml` という別のワークフローです。ラベルの付け外しでも走る必要があり、`mailcrew.yml` に足すと macOS / Windows のビルドまで走ってしまうためです。

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `src-tauri/tauri.conf.json` の `version` です。`package.json` と `src-tauri/Cargo.toml` の `version` も同じ値にそろえ、`Cargo.toml` を書き換えたら `cargo update -p mailcrew` で `Cargo.lock` も追随させてください。

`main` ブランチで `src-tauri/tauri.conf.json` の `version` が上がると、GitHub Actions が `mailcrew-v<version>` タグの Release を作り、上記の 2 つを添付します。上げずにマージした場合、タグが既に存在するため Release は作られません。

上げ忘れは `mailcrew-version.yml` が Pull Request を赤くして止めます。バージョンが base ブランチと同じか古いとき、4 つのファイルのバージョンが食い違っているときに落ちます。リリースしない変更（ドキュメントだけ、CI だけなど）には `no-release` ラベルを付けてください。

依存ライブラリの新しいバージョンは Dependabot が週に 1 回確かめ、見つかると Pull Request を作ります（設定は `.github/dependabot.yml`）。この Pull Request には `no-release` ラベルが付き、ビルドとテストがすべて通ると `.github/workflows/dependabot-merge.yml` がマージします。手で確かめる項目は次にこのアプリを変更する Pull Request でまとめて確認します。

Tauri は、Rust のクレートと npm のパッケージで major と minor がそろっていることを求めます。そのため Dependabot の設定では、Rust 側の `tauri`・`tauri-*` と npm 側の `@tauri-apps/*` を、それぞれ `tauri` という別のグループにしています。minor が変わる更新では片側だけが上がるので、このグループの Pull Request は落ちます。そのときは両側を同じ minor にそろえる Pull Request を手で作ります。
