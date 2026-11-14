# pecha-player の開発

pecha-player をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

フロントエンドは TypeScript で、PDF の描画に [PDF.js](https://mozilla.github.io/pdf.js/)(`pdfjs-dist`)を使います。
シェルは Tauri v2(Rust)です。配布対象は macOS(Apple Silicon)と Windows(x64)の 2 つで、どちらも GitHub Actions でビルドします。
Intel Mac 向けのビルドとユニバーサルバイナリは作りません。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)(stable) | `rustup` でインストール |
| Node.js 22 以降 | |
| [pnpm](https://pnpm.io/) 12 以降 | CI もこのバージョンを使います |
| macOS: Xcode Command Line Tools | `xcode-select --install` |
| Windows: Visual Studio Build Tools | 「C++ によるデスクトップ開発」を選択。Rust は MSVC ツールチェーンを既定にする |

詳細は [Tauri の Prerequisites](https://v2.tauri.app/start/prerequisites/) を参照してください。

## セットアップと実行

```sh
cd pecha-player
pnpm install          # フロントエンドの依存を取得
pnpm tauri dev        # 開発モードで起動
```

配布用のビルドは次のコマンドです。

```sh
pnpm tauri build
```

成果物は `src-tauri/target/release/bundle/` 以下に出力されます。

- macOS: `macos/pecha-player.app`、`dmg/pecha-player_<version>_aarch64.dmg`(Apple Silicon 向け)
- Windows: `nsis/pecha-player_<version>_x64-setup.exe`

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

## 仕組み

### 再生のタイミング

`src/timeline.ts` が、スタートを押した時刻と現在時刻だけから「何枚目か・経過時間・シャッターを下ろすか」を計算します。
タイマーは次に表示が変わる時刻(次の 1 秒か次のめくり)に 1 回ずつ仕掛けるだけで、カウンタを積み上げません。
タイマーが遅れても表示が追いつくだけで、ずれは溜まりません。進行バーはめくるたびに残り時間ぶんの CSS トランジションを
仕掛け直します。

### PDF の描画

PDF はどこから来たものでも開くことになるので、PDF.js は描画以外の機能を切って使います(`src/deck.ts`)。

- PDF 内の JavaScript は実行しません。PDF.js でスクリプトを動かすのはビューア部品だけで、それを使っていません。
  スクリプト用のエンジン(QuickJS)は `dist/` にも入れていません(`build.mjs`)
- XFA フォームは無効、注釈レイヤーは作りません。リンクも押せません
- DOM に入るのは canvas に描いた絵だけです。解析は PDF.js のワーカーの中で行います
- 300 MB を超えるもの、先頭 1 KB に `%PDF-` がないものは PDF.js に渡す前に断ります

古い WKWebView(macOS 11)でも動くよう、PDF.js は legacy ビルドを使います。ワーカーと、日本語フォントを埋め込んでいない
PDF に要る CMap・標準フォント・画像デコーダの wasm は、`build.mjs` が `dist/pdfjs/` にコピーします。

次のページは裏の canvas に先に描いておき、めくるときは表と裏を入れ替えるだけにしています。

### Google スライドの読み込み

webview からは別オリジンの応答を読めないため、ダウンロードは Rust 側で行います(`src-tauri/src/google.rs`)。

1. 貼り付けた URL からはスライドの ID だけを取り出し、`https://docs.google.com/presentation/d/<ID>/export/pdf`
   を組み立て直します。URL のほかの部分はリクエストに使いません
2. リダイレクトは自分で 1 つずつたどります。行き先は https の `docs.google.com` と `*.googleusercontent.com` だけを許し、
   `accounts.google.com` へ飛ばされたらログインが要ると判断します
3. 各リクエストには、webview の Cookie ストアがその URL に対して持っている Cookie を付けます
   (`Webview::cookies_for_url`)。ブラウザが送るのと同じものです

ログインは Google のログインページを開いた別ウィンドウで行います(`open_google_sign_in`)。ログイン後の行き先は
スライド自体で、そこまで読み込まれたらメインウィンドウにイベントを送ってウィンドウを閉じます。フロントエンドは
それを受けて読み込みをやり直します。すべてのウィンドウが Cookie ストアを共有しているので、ログインで入った Cookie を
そのまま 3 で使えます。

ログインのウィンドウにはどの capability も与えていません(`src-tauri/capabilities/default.json` は `main` だけを
指定しています)。Google のページからアプリのコマンドは呼べません。Google Workspace の SSO で会社の IdP を経由する
こともあるので、行き先のホストは絞らず、https 以外への遷移だけを止めています。

Cookie の読み出しは Windows では同期コマンドから呼ぶとデッドロックします。そのため `load_google_slides` は async で、
ダウンロード自体は blocking スレッドで行います。ウィンドウの作成も同じ理由で async コマンドから行います。

### フルスクリーン

フルスクリーンでは、スライドが画面全体を使います。Windows では通常のフルスクリーンです。

macOS では、Rust 側の `set_slide_fullscreen`(`src-tauri/src/fullscreen.rs`)が simple fullscreen に切り替えます。
ウィンドウをその場で画面いっぱいに広げ、タイトルバーをなくす方式です。新しい Space は作りません。そのうえで、
メニューバーと Dock を隠します(`NSApplication` の presentation options)。フルスクリーンを抜けると、もとに戻ります。

macOS 標準のフルスクリーン(`setFullscreen`)を使わないのは、メニューバーを隠せないためです。システム設定で
「フルスクリーンでもメニューバーを表示する」ようにしていると、アプリが presentation options を指定しても無視され、
メニューバーとタイトルバーがスライドの上に出ます。実機で確かめました。

simple fullscreen に切り替えると、tao がウィンドウのスタイルを変え、自分のビューを first responder にします。
そのままでは、クリックするまで web view にポインタの動きもキー入力も届きません。そこで、切り替えのたびに
web view を first responder に戻しています。

simple fullscreen に入っているかどうかはウィンドウに問い合わせられないので、`src/main.ts` が自分で覚えています。
緑のボタンから入る macOS 標準のフルスクリーンは OS の動きのままで、`isFullscreen` で検知して表示を合わせます。
フルスクリーンに入るとウィンドウの大きさが変わり、その通知でも状態を確かめます。その答えはウィンドウとの
往復のあとで受け取るので、自分で覚えている値もそのあとで読みます。先に読むと、切り替えたばかりの状態を
古い値で上書きしてしまいます。

操作バーはスライドの下端に重ねて置き、ポインタが 2.5 秒動かないとポインタと一緒に隠します。ポインタを動かすと
ポインタだけが戻り、画面の下端から 80 px 以内に来たときにバーも戻ります。隠す時点でポインタがバーの上にあるときと、
読み込みの画面を開いているときは隠しません。WebKit は、ポインタが動かなくても下のページが変わるたびに
`pointermove` を送ってきます(練習中は時計が毎秒変わります)。位置が変わっていないものは数えません。
バーが出入りしても、スライドの大きさは変わりません。

### Content-Security-Policy

`src-tauri/tauri.conf.json` の `csp` は自分のファイルだけを許します。PDF.js の wasm のために `script-src` に
`'wasm-unsafe-eval'` を、ワーカーのために `worker-src 'self'` を足しています。

## テスト

| 対象 | 実行 | 場所 |
|---|---|---|
| フロントエンド(`src/`)の E2E | `pnpm test:e2e` | `tests/e2e/` |
| URL からの ID の取り出し、リダイレクト先の判定、PDF の判定 | `cargo test` | `src-tauri/src/google.rs` の `mod tests` |

### フロントエンドの E2E テスト

[Playwright](https://playwright.dev/) が `src/` を**そのまま**ブラウザで動かします。Rust 側だけを
`tests/e2e/harness/backend.ts` が差し替えます。`@tauri-apps/api/mocks` の `mockIPC` が
`window.__TAURI_INTERNALS__.invoke` を乗っ取り、`load_google_slides` が返すもの(ページ数か、エラーの種類)は
テストごとに指定できます。ログインウィンドウからのイベントもテストから送れます。テスト用の PDF は
`tests/e2e/harness/pdf.ts` がその場で作ります。

時間は Playwright の偽の時計(`page.clock`)で進めます。「20 秒後にめくれる」「400 秒でシャッターが下りる」を、
実時間を待たずにちょうどの時刻で確かめられます。PDF.js は描画の一部を `setTimeout` で進めるため、ページの表示を
待つ `expectPage` は見つからない間だけ時計を 50 ms ずつ進めます。

ブラウザは 2 つ動かします。配布している 2 つの webview に対応させたものです。

| Playwright の project | 実機での相手 |
|---|---|
| `chromium` | Windows の WebView2 |
| `webkit` | macOS の WKWebView |

開発サーバは Content-Security-Policy を送りません。`tests/e2e/csp.spec.ts` だけが配布物と同じポリシーの下で
ページを開き、PDF.js のワーカーや wasm がポリシーに止められないことを確かめます。

届かない範囲もあります。webview の外にあるものと、本物の Google は見えません。

- Google との実際の通信。エクスポートの応答、リダイレクト、Cookie の受け渡し(判定のロジックは `cargo test` で確かめます)
- ログインウィンドウそのもの。開く・閉じる・スライドに着いたら閉じる、という Rust 側の挙動
- OS のファイル選択ダイアログと、Finder / エクスプローラーからのドロップ
- フルスクリーンが実際にどうなるか(`set_slide_fullscreen` が呼ばれたところと、バーとポインタを隠す判断までは確認します)
- ウィンドウのタイトル(`set_title` が呼ばれたところまでは確認します)
- シャッターのアニメーションの見た目
- コード署名していない配布物を各 OS が警告する挙動

これらは実機で確認します。

### 実機で確認すること

- PDF を選んで読み込み、表紙が出ること。ドロップでも読み込めること
- スタートから 20 秒ごとにめくれ、20 枚目のあとにシャッターが下りること。ストップと「もう一度」
- 公開の Google スライドが読み込めること
- 非公開の Google スライドで「Google にログイン…」が出て、ログイン後に自動で読み込まれること
- フルスクリーンにすると、タイトルバー・メニューバー・Dock が出ずにスライドが画面全体に表示されること。Esc で戻ること
- フルスクリーンで操作バーがしばらくすると隠れ、画面の下端にポインタを近づけると戻ること。バーの上にポインタがある間は隠れないこと
- 読み込んだファイル名がタイトルバーに出ること

### CI

`pecha-player/` を変更する Pull Request では、上の 2 つが両方走ります。E2E テストは `ubuntu-latest` で 1 回、
`cargo test` は macOS / Windows それぞれの rust checks ジョブの中です。E2E テストが落ちると、その run に
`playwright-report` が添付されます。

ジョブの構成、成果物リンクのコメント、失敗したテストの再実行の扱いは draftpad と同じです
(`.github/workflows/pecha-player.yml`)。

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や
挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `src-tauri/tauri.conf.json` の `version` の 1 箇所だけです。`package.json`・`src-tauri/Cargo.toml`・
`src-tauri/Cargo.lock` も同じ値に揃えます。`Cargo.toml` を書き換えたら `cargo update -p pecha-player` で
`Cargo.lock` を追随させてください。

`main` ブランチで `version` が上がると、GitHub Actions が `pecha-player-v<version>` タグの Release を作り、
`.dmg` と `-setup.exe` を添付します。上げずにマージした場合、タグが既に存在するため Release は作られません。

上げ忘れは `.github/workflows/pecha-player-version.yml` が Pull Request を赤くして止めます。4 つのファイルの
バージョンが食い違っているときも落ちます。リリースするつもりがない変更では、Pull Request に `no-release`
ラベルを付けてください。

依存ライブラリの新しいバージョンは Dependabot が週に 1 回確かめます(`.github/dependabot.yml`)。この Pull Request
には `no-release` ラベルが付き、ビルドとテストがすべて通ると `.github/workflows/dependabot-merge.yml` がマージします。
上の「実機で確認すること」は、次にこのアプリを変更する Pull Request でまとめて確認します。

Tauri は、Rust のクレートと npm のパッケージで major と minor がそろっていることを求めます。そのため Rust 側の
`tauri`・`tauri-*` と npm 側の `@tauri-apps/*` は、それぞれ `tauri` という別のグループにしています。minor が変わる
更新では片側だけが上がるので、このグループの Pull Request は落ちます。そのときは両側を同じ minor にそろえる
Pull Request を手で作ってください。

## コード署名

pecha-player はアクセシビリティなどの権限を使わないため、署名していません。各 OS の警告への対処は README に
書いてあります。
