# draftpad の開発

draftpad をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

フロントエンドは CodeMirror 6、シェルは Tauri v2(Rust)です。配布対象は macOS(Apple Silicon)と
Windows(x64)の 2 つで、どちらも GitHub Actions でビルドします。

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
cd draftpad
pnpm install          # フロントエンドの依存を取得
pnpm tauri dev        # 開発モードで起動(フロントエンドを監視し、Rust 側を自動ビルド)
```

配布用ビルド:

```sh
pnpm tauri build
```

成果物は `src-tauri/target/release/bundle/` 以下に出力されます。

- macOS: `macos/draftpad.app`、`dmg/draftpad_<version>_aarch64.dmg`(Apple Silicon 向け)
- Windows: `nsis/draftpad_<version>_x64-setup.exe`

## 検証コマンド

```sh
pnpm typecheck                     # TypeScript の型検査(src/ と tests/)
pnpm lint:style                    # style.css がトークンだけで組まれているかの検査
pnpm build                         # フロントエンドのバンドル(dist/)
pnpm test:e2e                      # フロントエンドの E2E テスト
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

`pnpm test:e2e` は初回だけブラウザの取得が必要です。

```sh
pnpm exec playwright install chromium webkit
```

## Pull Request のビルド

`draftpad/` を変更する Pull Request では、GitHub Actions が macOS / Windows の両方をビルドします。両方が成功すると、
成果物へのダウンロードリンクを Pull Request にコメントします。push のたびに新しいコメントを投稿するので、再ビルドの
完了も通知で届きます。前回までのコメントは outdated として畳まれますが、消えはしないので、保持期限内なら過去の
ビルドもそこから辿れます。

リンク先のダウンロードには GitHub へのログインが必要です。成果物には保持期限があり、過ぎるとリンクは無効になります
(期限はコメントに書かれます)。

## テスト

手元と CI で走るものは 2 つです。

| 対象 | 実行 | 場所 |
|---|---|---|
| フロントエンド(`src/`)の E2E | `pnpm test:e2e` | `tests/e2e/` |
| `state.json` の読み書き | `cargo test` | `src-tauri/src/state.rs` の `mod tests` |

### フロントエンドの E2E テスト

[Playwright](https://playwright.dev/) が `src/` を**そのまま**ブラウザで動かします。Rust 側だけを
`tests/e2e/harness/backend.ts` が差し替える形で、`@tauri-apps/api/mocks` の `mockIPC` が
`window.__TAURI_INTERNALS__.invoke` を乗っ取ります。`load_state` が返す状態・`platform`・
`list_fonts` の一覧・保存の失敗はテストごとに指定でき、アプリが投げた `invoke` は順番どおりに
記録されるので、「終了前に保存したか」のような順序まで確認できます。

ブラウザは 2 つ動かします。配布している 2 つの webview に対応させたものです。

| Playwright の project | 実機での相手 |
|---|---|
| `chromium` | Windows の WebView2 |
| `webkit` | macOS の WKWebView |

`platform` は差し替えられるので、macOS 側の経路(メニューバーから届く `menu` イベント)と
Windows 側の経路(アプリ内のキー処理)を、どちらも Linux のランナー 1 台で確認できます。
`tests/e2e/menu.spec.ts` と `tests/e2e/shortcuts.spec.ts` がその 2 つです。

開発サーバは Content-Security-Policy を送りません。配布物はこれを持つため(`src-tauri/tauri.conf.json` の
`csp`)、ポリシーが禁じている読み込みはテストでは通り、インストールしたアプリでだけ失敗します。
`tests/e2e/csp.spec.ts` だけが同じポリシーの下でページを開くので、`data:` URI のような自分のファイル以外の
読み込みを足したときはここに追います。

届かない範囲もあります。webview の中に無いものは一切見えません。

- macOS のメニューバーそのもの、Windows のタスクバーメニュー(ジャンプリスト)
- 右クリックで開くコンテキストメニューそのもの。これもネイティブのメニューです。webview の
  既定のメニューを止めたことと、`show_context_menu` に何を渡したかまでを確認します
- IME の未確定文字列と変換候補の位置
- 常に手前に表示・フルスクリーン・ウィンドウサイズが実際にどうなるか
  (`invoke` が正しく呼ばれたところまでは確認します)
- 展開したドロップダウンの macOS での見た目。Playwright が同梱する WebKit は 26.0 で、
  `appearance: base-select` は WebKit 27 からなので、プラットフォームのメニューが開く側の経路しか
  通りません。該当のテストは `CSS.supports()` を見て自分をスキップするため、Playwright が 27 以降を
  同梱したら自動的に走り始めます(Chromium 側は `tests/e2e/editing.spec.ts` がリストの行をクリックして
  確認しています)
- コード署名していない配布物を各 OS が警告する挙動
- Windows インストーラの画面と挙動(`src-tauri/installer.nsi`)
- OS のクリップボードの中身。`plugin:clipboard-manager|read_text` はハーネスが返すので、
  読んだテキストがエディタのどこへ入るかまでを確認します

これらは実機で確認するしかありません。裏を返せば、実機で見るべきものはこの一覧に絞られます。

### CI

`draftpad/` を変更する Pull Request では、上の 2 つが両方走ります。E2E テストは `ubuntu-latest` で 1 回、
`cargo test` は macOS / Windows それぞれの rust checks ジョブの中です。E2E テストが落ちると、その run に
`playwright-report` が添付されます。

検証は 3 系統のジョブが同時に走ります。`frontend`(E2E・型検査・`lint:style`)、`rust checks (macos-latest / windows-latest)`
(`cargo fmt` / `clippy` / `cargo test`)、`build (macos-latest / windows-latest)`(`pnpm tauri build` と成果物の
アップロード)です。成果物リンクのコメントは `build` の後に付きます。rust checks と build は同じ依存クレートを
それぞれ別のプロファイル(dev と release)でコンパイルするため、順に走らせると所要時間が単純に足し算になります。
依存クレートは [`Swatinem/rust-cache`](https://github.com/Swatinem/rust-cache) でキャッシュします。Pull Request
への push でも書き込むので、2 回目以降の push はクレートのダウンロードとコンパイルを省けます。キャッシュは
リポジトリ全体で 10 GB を共有し、超えると古いものから捨てられます。Pull Request が書いたキャッシュはクローズ時に
消えます。

バージョンの検査だけは `.github/workflows/draftpad-version.yml` という別のワークフローです。ラベルの付け外しでも
走らせる必要があり、それを `draftpad.yml` に足すと macOS / Windows のビルドまで巻き添えで走ってしまうためです。
数秒で終わるので、ラベルを触るたびに走っても実害はありません。

こちらには `paths` を書いていません。スキップされたジョブはステータスを報告せず、required status check に
指定したものが報告されないと Pull Request はいつまでもマージできなくなるためです。代わりに全部の Pull Request で
走り、`draftpad/` 以下が 1 つも変わっていなければジョブ自身が何もせずに通します。required に指定するならこちらだけ
安全です(`draftpad.yml` 側のジョブは `paths` で絞っているので、指定すると同じ理由で詰まります)。

CI では失敗したテストを 1 回だけ再実行します。1 回目の trace が残るためですが、再実行で
通ったもの(flaky)は成功扱いにしません。`pnpm test:e2e` が `--fail-on-flaky-tests` を
渡しているので、ジョブは赤になります。緑のチェックの裏に不安定なテストが隠れない、という
のがここの意図です。

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や
挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `src-tauri/tauri.conf.json` の `version` の 1 箇所だけです。Release のタグ名、配布物のファイル名、
環境設定パネルの表示のすべてがここから決まります。`package.json` と `src-tauri/Cargo.toml` の `version` は実際には
参照されませんが(Tauri は `tauri.conf.json` に `version` があればそちらを使います)、紛らわしいので同じ値に揃えます。
`Cargo.toml` を書き換えたら `cargo update -p draftpad` で `Cargo.lock` も追随させてください。

`main` ブランチで `src-tauri/tauri.conf.json` の `version` が上がると、GitHub Actions が
`draftpad-v<version>` タグの Release を作り、上記の 2 つを添付します。上げずにマージした場合、タグが既に存在するため
Release は作られません。

### 上げ忘れを CI が止めます

上げ忘れても Release ジョブは黙ってスキップするだけなので、`draftpad-version.yml` が Pull Request を赤くします。
`draftpad/` を変更する Pull Request では、次のどれかに当たると落ちます。

- バージョンが base ブランチの先端と同じ、または古い
- 上の 4 つのファイルのバージョンが食い違っている

`draftpad/` 以下に変更が 1 つもない Pull Request は対象外です(その場合は何も確認せずに通ります)。

リリースするつもりがない変更(ドキュメントだけ、CI だけ、など)では、Pull Request に `no-release` ラベルを
付けてください。付いていればバージョンが据え置きでも通ります。上げるか、上げない理由をラベルで示すか、
どちらかを必ず選ぶことになります。

ビルド成果物のコメントにもバージョンが出ます。据え置きのまま `no-release` で通した Pull Request では、
Release が作られないことをそこで警告します。動作確認の前に必ず読む場所なので、macOS 側で作業していて
インストーラを開かない場合でも目に入ります。

配布物はコード署名・公証をしていません。そのため、ダウンロードした macOS 版は初回のみ `com.apple.quarantine`
属性の解除が必要で、Windows 版はインストーラ実行時に SmartScreen の警告が出ます。どちらも手順は README.md に
書いてあります。自分でビルドした `.app` には quarantine 属性が付かないため、開発中にこの操作は要りません。
macOS Sequoia (15.0) 以降では、以前あった Control クリック →「開く」による回避はできません。

## プラットフォーム固有の実装

キーボードショートカットは、macOS ではメニューバーのアクセラレータとして、Windows ではアプリ内のキー処理として
実装しています。Windows にはメニューバーがありません。

元に戻す / やり直すは例外で、どちらのプラットフォームでも CodeMirror のキーマップが処理します。CodeMirror の
`historyKeymap` は「やり直す」の `Ctrl+Shift+Z` を Linux 向けにしか割り当てていないため、`src/editor.ts` で明示的に
割り当てています。macOS の Edit メニューでは、この 2 つだけ定義済み項目を使わずコマンド表へ転送しています。定義済み
項目は WKWebView 自身の undo マネージャを動かすもので、エディタの履歴には触れないためです。

`⌘ ⇧ V` / `Ctrl + Shift + V`(プレーンテキストとして貼り付け)は、クリップボードのテキストを読んで
カーソル位置へ挿入します。draftpad はプレーンテキストしか扱わないので、`⌘ V` / `Ctrl + V` と結果は
同じです。macOS のメニューでは Edit に `Paste as Plain Text` を足しています。定義済みの貼り付け項目は
`⌘ V` に固定されていて、アクセラレータを差し替えられないためです。

読み取りは Tauri 公式の `tauri-plugin-clipboard-manager` 経由で、`capabilities/default.json` が許可するのは
`clipboard-manager:allow-read-text` の 1 つだけです。webview の `navigator.clipboard.readText()` は使いません。
WKWebView は別のオリジンが書いた内容を読むときに確認用の「ペースト」ボタンを出し、WebView2 は権限の確認を
挟みます。キーを押しただけで貼り付いてほしいこの操作には、どちらも合いません。

挿入先はエディタだけです。検索・置換や環境設定の入力欄がキーボードを持っている間は何もしません
(`src/main.ts` が `Editor.hasFocus` を見ています)。クリップボードにテキストが入っていない場合、
プラグインはエラーを返すので、何も起きません。

Windows のタスクバーメニュー(ジャンプリスト)の項目はショートカットなので、押すと draftpad が
もう一度起動されます。すでに起動していれば、その 2 つ目のプロセスは引数を既存のウィンドウへ渡して
自分は終了します。このため Windows 版は多重起動しません(同じ `state.json` を 2 つのプロセスが
奪い合わないという利点もあります)。macOS 版の挙動は変わりません。

ジャンプリストは起動のたびにアプリ自身が登録し直します。インストーラは関与しないので、
バージョンを上げたり別の場所へ移したりしても、次の起動で正しい実行ファイルを指し直します。

Windows は IME の未確定文字列と変換候補をキャレットの位置に合わせて表示します。キャレットを持つ要素が
ウィンドウの中にないまま操作対象になると、合わせる先がないため画面左上に出てしまいます。そのため
ウィンドウは `visible: false` のまま起動し、フロントエンドがエディタにフォーカスを移してから
`src/main.ts` が表示します(`src-tauri/src/lib.rs` には、フロントエンドが起動しなかったときのために
10 秒後に表示するフォールバックだけ残してあります)。ウィンドウが操作対象に戻ったときに中身の
どこにもフォーカスがなければ、エディタへ戻すようにもしています。

Windows の WebView2 はテキスト入力をフォームの一部とみなすため、検索・置換や環境設定の入力欄に
フォーカスすると「保存された情報」の候補が出ます。draftpad にフォームはないので、起動時に
`ICoreWebView2Settings4` の `IsGeneralAutofillEnabled` と `IsPasswordAutosaveEnabled` を false にして
止めています(`src-tauri/src/autofill.rs`)。macOS の WKWebView にこの挙動はありません。

右クリックすると webview は自分のメニューを出します。WKWebView のものは「調べる」「翻訳」「共有」「音声」
「フォント」「変換」「変形」「段落の方向」「サービス」「Writing Tools」まで並び、下書きの編集と関係のない項目が
ほとんどです。項目単位で削れないかを先に調べましたが、macOS では成立しませんでした。WebKit が `NSMenuItem` に
付ける `WKMenuItemIdentifier` は「コピー」「ペースト」「調べる」などにはあるものの、「切り取り」「すべてを選択」
「フォント」「サービス」には付きません。識別子で残す物を選ぶと切り取りが消え、識別子で消す物を選ぶとフォントや
サービスが残ります。

そのため webview のメニューはページ側で止め(`src/context-menu.ts` が `contextmenu` を `preventDefault` します)、
代わりに Tauri のネイティブメニューを `popup` で出しています(`src-tauri/src/menu.rs` の `show_context`)。並べる
項目はこちらが組み立てるので、OS が新しい項目を足しても影響を受けません。位置は渡していません。位置を省くと
muda がポインタの位置に出し、画面の端に収まるよう寄せます。

切り取り・コピー・貼り付け・すべてを選択は定義済み項目です。OS 自身の表記が付き、macOS では responder chain、
Windows では `Ctrl` との組み合わせの送出を通じて、キャレットを持っている要素——下書き・検索欄・環境設定の
入力欄——にそのまま届きます。定義済み項目は有効・無効を切り替えられないので、選択範囲がなくても灰色にはならず、
押しても何も起きないだけです。元に戻す・やり直す・検索・置換はコマンド表へ転送する項目なので、`show_context_menu`
が受け取る 3 つの真偽値で灰色にできます。

macOS では、選択範囲の外を右クリックするとカーソル下の単語が選択されます。WebKit が `contextmenu` イベントを
投げる前に行う macOS の編集挙動で、Windows (Chromium) にはありません。どちらもエンジンの標準どおりにしてあります。

### Windows インストーラ(NSIS)

自動アップデートに対応していないぶん、手動の入れ直しが軽く済むようにインストーラへ手を入れています。
Tauri の NSIS スクリプトはビルド時のテンプレートなので、`tauri.conf.json` の
`bundle.windows.nsis.template` で `src-tauri/installer.nsi` を指し、Tauri のものを置き換えています。

`src-tauri/installer.nsi` は Tauri の
[`installer.nsi`](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi)
(`@tauri-apps/cli` v2.11.4 のもの)をそのまま写し、次の 4 点だけを変えたものです。変更箇所には
`; draftpad:` で始まるコメントを付けてあります。

| 変更 | 内容 |
|---|---|
| 更新方法の既定 | 旧バージョンを検出したときのラジオボタンで、「アンインストールしてから入れる」ではなく「上書きする」を最初から選んでおきます |
| 起動中の draftpad | 確認ダイアログを出さずに終了させます。インストーラ・アンインストーラのどちらも |
| インストール後 | ログの画面で止まらず、完了画面まで自動で進みます |
| デスクトップショートカット | 完了画面のチェックボックスは、上書きインストールのときだけ外した状態で出します。新規インストールと、アンインストールを挟んだときは従来どおり入った状態です |

`utils.nsh` や言語ファイルは Tauri が出力先へ書き出すものをそのまま使うので、写しているのは
`installer.nsi` 1 ファイルだけです。`@tauri-apps/cli` を上げたときは上流の同ファイルと diff を取り、
上流の変更を取り込んでください。テンプレートが壊れていれば Windows の `build` ジョブが
`pnpm tauri build` で失敗します。

## UI の寸法と色

ステータスバー・検索パネル・環境設定パネルは、`src/style.css` の `:root` にあるカスタムプロパティだけで
組み立てます。UI ライブラリは入れていません。デザインガイドラインという別の文書も置いていません。
トークンの一覧そのものが仕様で、`pnpm lint:style` がそれを守らせます。値はここに書きません。CSS が唯一の
出どころで、文書に写すと片方が古くなるだけです。

| ファミリ | トークン | 役割 |
|---|---|---|
| 文字サイズ | `--font-size-title` / `-body` / `-caption` | 環境設定の見出し / パネル本文 / ステータスバーとバージョン表示 |
| 書体 | `--ui-font`、`--font-mono` | UI 全体 / バージョン表示。数の段階ではないので、予算の数え方も他と別です |
| 余白 | `--space-1` 〜 `--space-5` | 4px 刻みの 5 段。コントロール同士、バーとパネルの内側、グループ同士 |
| 角丸 | `--radius-sm` / `-md` / `-lg` | 部品の大きさに対応した 3 段(チェックボックスとアイコンボタン / 高さ `--control-height` のコントロール / パネル) |
| 寸法 | `--control-height`、`--checkbox-size`、`--toggle-width`、`--toggle-size-search`、`--glyph-size`、`--icon-size`、`--icon-button-size`、`--statusbar-height`、`--titlebar-height` | コントロールとバーの大きさ |
| レイアウト | `--field-width`、`--field-width-narrow`、`--field-width-search`、`--button-width-search`、`--label-width`、`--panel-width`、`--panel-inset`、`--language-width`、`--count-width`、`--count-width-narrow` | 入力欄・ラベル列・検索パネルと環境設定パネルの配置と、ステータスバーの文字数・行数セルの幅 |
| パレット | `--bg`、`--fg`、`--muted`、`--muted-strong`、`--placeholder`、`--border` など | 色、影 2 段(`--shadow-panel` / `--shadow-popup`)、描き込む印 7 枚(✓ / シェブロン上下 / × / 横棒 / Aa / .*) |

基準にしたのは Windows 11 のメモ帳のステータスバーです。macOS では `-apple-system`、Windows では Segoe UI が
当たるだけで、寸法は共通です。OS ごとに変えたくなったら `:root[data-platform="macos"]` でトークンを上書き
してください(`--titlebar-height` がすでにそうなっています)。

エディタ本文のフォントサイズは設定項目なので、このトークンには含めません。CodeMirror が自分で描く部分の色は
`src/dark-theme.ts` にあり、構文ハイライトの色だけは直値です(パレットとは別の体系なので意図的にそうしています)。

### アイコンと淡い文字

歯車とピン(常に手前に表示)は HTML に直接書いた SVG です。閉じるボタンの × は `--close-icon` をマスクして描きます。文字として
置くと、字形の中心と文字の送り幅の中心がずれる分だけボタンの中央から外れ、その量がフォントによって
変わります。CodeMirror は自前の閉じるボタンに × の文字を書き込むので、そちらは文字を `font-size: 0` で
畳んで同じマスクを被せています。検索パネルの「前へ」「次へ」も同じで、文字を畳んで `--chevron-up` /
`--chevron-down` を被せます(`--chevron-down` はセレクトの矢印と同じ画像です)。読み上げ用の名前は文字と
一緒に消えるので、`src/search-panel.ts` が `aria-label` に移し替えます。擬似要素にしか届かないもの
(✓・シェブロン・×・横棒・Aa・.*)が CSS の画像で、HTML から触れるもの(歯車・ピン)が SVG です。

検索パネルの「Aa」「.\*」トグルも同じ理由で画像です(`--case-mark` / `--regexp-mark`)。文字として置くと、
当たるフォントが macOS と Windows で違うぶんだけ字形も送り幅も変わり、20px の枠の中での位置が揃いません。

線の太さは 5 枚とも 1.4〜1.5 に揃えてあります。16×16 の viewBox に対しての値なので、`--glyph-size` で
描く印も `--icon-size` で描く印も、画面上ではほぼ同じ太さになります。ここを 1 枚だけ太くすると、その印だけ
別の出どころから持ってきたように見えます。

2 つのシェブロンと `--close-icon` はマスクなので `currentColor` で塗れますが、`--check-mark` と `--minus-mark`
は `input` の擬似要素の背景として敷くため色を焼き込んであり、テーマごとに 2 つずつ持っています。
`--check-mark` はチェックボックスの ✓ とトグルのオンの印を兼ねます。どちらも下地は `--accent` で、
必要な色が同じだからです。

`--muted-strong` は環境設定パネルのグループ見出しだけに使います。見出しは項目名より上位なので `--muted` では
沈みすぎ、`--fg` では項目名と同じ強さになります。文字サイズは項目名より小さいままにして、太さと字間で
上下関係を示しています(サイズを上げて示そうとすると、項目名との差が「大きさ」だけになります)。

プレースホルダは `--muted` ではなく `--placeholder` です。`--muted` はステータスバーやバージョン表示に
使う「読ませる二次テキスト」で、プレースホルダは「まだ何も入っていない」ことを示すものなので、同じ色だと
入力済みに見えます。ライト・ダークとも背景に対して約 3.3:1 に揃えてあります。

### 検索パネル

パネルを組み立てるのは CodeMirror で、中身の markup に口は出せません。丸ごと差し替える口だけはありますが
それは重すぎるので、`src/search-panel.ts` が出来上がったパネルに後から手を入れる形にしています。足すのは
件数を書き込む `span` 1 つだけで、あとは属性と `disabled` の付け外しです。

- **2 行 2 列のグリッド**。どこに何が来るかは `style.css` の `grid-area` だけで決めます。DOM の順序は
  CodeMirror が書いたまま、つまり Tab の順序もそのままです。ボタンは 4 つとも同じ幅で、対の右側を
  ちょうど 1 個ぶん右へ寄せてあるので、上下の 2 対は左端も右端も揃います。
- **件数**。CodeMirror は持っていないので、`SearchQuery` のカーソルで文書を走査して数えます。数え直すのは
  検索語かオプションが変わったときと、入力が 100ms 止まったときだけです。「2 / 3 件」の左の数字は、選択範囲が
  一致のどれかとぴったり重なっているときだけ出ます(まだどれにも移動していない状態では総数だけ)。
- **一致なし**。正規表現として書きかけの文字列も、単に見つからない場合と同じ扱いです。下書きの途中で
  赤くしても手が止まるだけなので、色も変えません。
- **無効化**。一致が 0 件のあいだは「前へ / 次へ / 置換 / すべて」を `disabled` にします。
- **トグルの中身**。「Aa」「.\*」は文字ではなくマスクした画像です。読み上げ用の名前は CodeMirror が
  `label` に書いた文字がそのまま担うので(`font-size: 0` で畳んでも消えません)、ここは属性を足しません。

### ドロップダウン

`<select>` を展開したリストは、既定ではプラットフォームが描く別ウィンドウで、CSS が一切届きません。
`appearance: base-select` を指定すると、リストがページの中の普通の要素(`::picker(select)`)になり、
このスタイルシートで組めるようになります。対応は Chromium 135 / WebKit 27 以降です
(MDN の browser-compat-data で確認できます)。

**指定しているのは Windows だけです。**

| 環境 | 展開時の見た目 |
|---|---|
| Windows(WebView2) | こちらで組んだもの。WebView2 は自動更新されるため 135 以降になります |
| macOS(WKWebView) | macOS 自身のメニュー。`:root[data-platform="windows"]` で囲っているので、Safari が 27 以降になっても変わりません |

macOS のメニューはその OS の他の部分と揃っているので、そちらに任せるほうがよいという判断です。
スコープで明示しているのは、これを「インストールされている Safari が古いから結果的にそう見えている」
状態にしないためです。囲まなければ、OS を更新した人の手元で見た目が勝手に入れ替わります。

行の間隔は `--space-1` です。ピッカーを開くと選択中の行にフォーカスが残るので、その隣をホバーすると
同じハイライトが 2 行に乗ります。詰めて並べているとこれが 1 つの縦長のブロックに見えてしまうため、
行を離してあります。間隔は `option` の `margin-block` で、`gap` は使えません。閉じたピッカーは
ブラウザが `display: none` で隠しており、`::picker(select)` に `display` を書くとそれに勝ってしまって、
起動した瞬間から全部のセレクトのリストが開いたまま画面に乗ります(`dialog:not([open])` と同じ罠です)。
`tests/e2e/editing.spec.ts` がこれを見ています。

エンジン側が対応していない場合も、宣言が丸ごと無視されてプラットフォームのメニューが開きます。
`tests/e2e/editing.spec.ts` がこの判断を見ています。判定には `CSS.supports()` ではなく、セレクトに
実際に base appearance が乗ったかどうかを使っています。前者はエンジンの対応だけを見るので、
macOS でも true になってしまいます。

Windows 側では、指定しないと元の見た目が変わるところが 2 点あります。

- **矢印**。プラットフォームが描いていたものが `::picker-icon` に移ります。`--chevron-down` を
  `currentColor` でマスクして塗るので、テーマごとに画像を用意する必要はありません(チェックボックスの
  ✓ だけは `input` の擬似要素に背景色が届かないため、いまも色別に 2 つ持っています)
- **ステータスバーのセレクトの幅**。ネイティブの `<select>` は一番長い選択肢に合わせた幅を持つため、
  言語を変えてもバーは動きませんでした。base-select は選択中のラベルに合わせて縮むので、
  `--language-width` を下限として与えて同じ挙動に戻しています。この下限も Windows だけに掛けて
  あります(macOS はネイティブの幅のまま)

### `pnpm lint:style` が見ているもの

`lint-style.mjs` が `src/style.css` を読んで、次に当たると落ちます。依存はありません。

- `:root` の外に 3px 以上の長さが書かれている(罫線とフォーカスリングの 1〜2px だけは許します)
- `:root` の外に色が直接書かれている
- `:root` の外でトークンを宣言している(在庫が 2 つに割れるため)
- 宣言したのに使われていないトークンがある
- `var()` で参照しているのに宣言がないトークンがある。CSS は未定義のカスタムプロパティを黙って無視するので、
  綴り間違いは画面を見ても気付けません
- ファミリの値の規則から外れている。余白は 4 の倍数、文字サイズは 11〜18px の整数、角丸は 8px 以下の偶数、
  それ以外の長さは 4 の倍数
- ファミリの個数が予算を超えている

`--ui-font` と `--font-mono` はどの予算にも数えません。長さでも色でもなく、段階を持たないためです。

見ているのは px / em / rem だけです。`50vh` のようなビューポート基準の値は、段階を持つ寸法ではなく
その場のレイアウトなので、トークンにせず直接書きます。

検査が壊れて黙って通るようになるのを防ぐため、実行のたびに、まず `lint-style.mjs` 末尾のフィクスチャに対して
ルールを走らせます。上の各項目に 1 つずつ、報告されるはずの最小の CSS が並んでいて、どれかが報告されなく
なったら「この検査は主張どおりのことを見ていない」と言って落ちます。フィクスチャはスクリプトの中にあるので、
ルールを変えたときに追随させ忘れる別ファイルにはなりません。

### トークンを足すとき

個数の予算は `lint-style.mjs` の先頭にあります。上限は現在の個数そのままなので、1 つ足すには予算も上げる
必要があります。上げるのは構いません。ただ 1 行の差分として残るので、レビューで「本当に必要か」を必ず一度
通ることになります。気まぐれで 5px の文字サイズを足すのは、値の規則の側で止まります。

機械で決められないことが 3 つ残ります。

- **既存で足りないか。** 1〜2px の違いで新しい値が欲しくなったときは、たいてい既存に寄せたほうが揃います。
  足す前にこれを試してください
- **どのファミリか。** 段階のあるもの(文字サイズ・余白・角丸)は序数か大小で名付け、役割が 1 つに決まるもの
  (`--label-width` など)は用途で名付けます
- **1 箇所しか使わない値をトークンにするか。** します。在庫を 1 箇所にまとめるのが目的なので、使用箇所が
  1 つでも `:root` に置きます。ただし予算を食うので、既存で足りるならそちらが先です

## 環境設定パネル

環境設定は `<dialog>` で、`showModal()` で開きます。モーダルダイアログはトップレイヤーに置かれるため、
スタッキングコンテキストの外側に出ます。背面が操作不能になるのもブラウザ側の保証です。

これは見た目の好みではなく、CodeMirror が自前のスタッキング順を持っていることへの対策です。検索バーと
Vim のステータス行(`.cm-panels`)は `z-index: 300`、入力候補などのツールチップ(`.cm-tooltip`)は
`z-index: 500` で、どちらも `.cm-editor` が自身のスタッキングコンテキストを作らないままページのルートに
積まれます。パネルを普通の要素で重ねると、この数字を上回る `z-index` を書き続けることになります。
トップレイヤーならその競争に参加しません。`src/style.css` に `z-index` が 1 つもないのはこのためです。

2 点だけ注意があります。

- `dialog:not([open]) { display: none }` はブラウザ標準のスタイルなので、作成者スタイルの `display` に
  負けます。`#preferences` の表示切り替えを明示的に書いているのはこのためです。
- 暗幕は `::backdrop` ではなくダイアログ要素自身の背景で描いています。`::backdrop` が元の要素から
  カスタムプロパティを継承するようになったのは Safari 17.4 からで、それ以前では `var(--overlay)` が
  解決できません。

`<dialog>` の `showModal()` は WebKit では Safari 15.4 以降です。README が対応を謳う macOS 11 でも
Safari 16.6 まで更新できるので通常は問題になりませんが、Safari を一度も更新していない macOS 11 では
環境設定が開けません。

### 項目のまとまり

7 項目は「編集」「表示」の 2 グループに分けてあります。`<fieldset>` + `<legend>` で、既定の枠線と余白は
消して、2 群の間の区切り線 1 本だけを描きます。DOM の順序がそのままタブ順なので、並べ替えは
`tests/e2e/preferences.spec.ts` のタブ順のケースが押さえています。

### 真偽値のコントロール

環境設定の「入力候補を表示」はトグルとして描きます。パネルの設定は切り替えた瞬間に効くもので、
チェックボックスは「これから送信するもの」に見えるためです。ステータスバーの「常に手前に表示」は
チェックボックスのままです(バーに 28px のトグルは入りません)。スコープが `#preferences` の中だけに
なっているのはこのためで、共有の `input[type="checkbox"]` の規則はそちらが使い続けます。

中身は `<input type="checkbox">` のままです。`appearance: none` で枠(トラック)を描き、`::before` を
つまみ(ノブ)として動かしているだけなので、`<label for>` の結び付き・`change` イベント・スクリーン
リーダーへの伝わり方はどれも素のチェックボックスと同じです。`src/preferences.ts` に手は要りません。

状態は 3 つで示します。ノブの位置(左 / 右)、色(`--toggle-knob` / `--accent`)、印(横棒 / ✓)です。
「オン」「オフ」の文字は使いません。項目によっては文言が合わないためで、位置だけ・色だけに頼らない
のは、片方が見えない環境でも残りが読めるようにするためです。オフでノブが枠から浮いて見えることが
条件で、その担保はテーマごとに違います。ライトはノブの白に `--border` の輪郭、ダークは輪郭ではなく
`--control-active` と `--control-bg` の明度差です(ダークでノブを暗くして差を作ろうとすると、今度は
ノブが穴に見えます)。

ノブは `translate` で動かします。仕様上は `margin-inline-start: auto` で右端に寄せる書き方もできますが、
`auto` は補間できないのでノブが瞬間移動します。`prefers-reduced-motion: reduce` ではこの遷移を切ります。

## 状態の保存

設定と本文は 1 つの JSON にまとめて保存します。書き込みは一時ファイルに書いてから置き換える方式なので、
途中でプロセスが落ちても壊れたファイルは残りません。壊れていた場合は `state.json.broken` として退避します。

- macOS: `~/Library/Application Support/com.ysaeki.draftpad/state.json`
- Windows: `%APPDATA%\com.ysaeki.draftpad\state.json`

ウィンドウのサイズは保存しますが、位置は保存しません(常に画面中央に開きます)。

## 依存関係の方針

Tauri 公式・CodeMirror 公式・Microsoft 公式以外の依存は次の 2 つだけです。

- `@replit/codemirror-vim`(Vim モード)。外す場合は `src/vim.ts` と `Editor` の `vim` Compartment を削除
- `font-kit`(フォント一覧の取得)。外す場合は `src-tauri/src/fonts.rs` と `list_fonts` コマンドを削除

上の方針は配布物に入る依存の話です。テストのためだけの開発依存が 2 つあり、どちらも配布物には
入りません。

- `@playwright/test`(Microsoft 公式)。フロントエンドの E2E テスト
- `@types/node`(DefinitelyTyped)。`tests/` と `playwright.config.ts` の型付けだけに使います。
  `src/` には `tsconfig.test.json` で分けてあるので Node の型は入りません

`tests/e2e/harness/backend.ts` のバンドルは、バンドルに使っている `esbuild` をそのまま使います。
Rust 側のテストは追加の crate を使いません(一時ディレクトリは `mod tests` の中で自作しています)。

両方のプラットフォームで使う Tauri 公式のプラグインが 1 つあります。

- `tauri-plugin-clipboard-manager` と `@tauri-apps/plugin-clipboard-manager`。「プレーンテキストとして
  貼り付け」がクリップボードを読むのに使います。外す場合は `src/commands.ts` の `paste_plain` と、
  `src-tauri/src/menu.rs` の同じ id の項目をまとめて削除してください

Windows 向けのビルドだけが使う依存が 3 つあります。

- `windows`(Microsoft 公式の Win32 バインディング)。ジャンプリストを作る Shell COM API に使います
- `tauri-plugin-single-instance`(Tauri 公式)。ジャンプリストから起動された 2 つ目のプロセスの
  引数を、動作中のインスタンスへ渡します
- `webview2-com`(WebView2 COM API のバインディング)。Tauri 自身が使っているものと同じクレートで、
  オートフィルを切るために WebView2 の設定へ触ります

前の 2 つはタスクバーメニューのためのもので、外す場合は `src-tauri/src/jumplist.rs` と `lib.rs` の
該当箇所をまとめて削除してください。`webview2-com` は `src-tauri/src/autofill.rs` だけで使います。

### 更新の運用(サプライチェーン対策)

- npm: `pnpm-workspace.yaml` の `minimumReleaseAge: 10080` により、公開から 7 日未満のバージョンは解決しません。
  `pnpm-lock.yaml` をコミットし、更新は `pnpm update` を明示的に実行したときだけ行います
- npm(インストールスクリプト): 依存のインストールスクリプトは `pnpm-workspace.yaml` の `allowBuilds` で
  明示的に許可したものだけが走ります。挙動を決めていない依存が現れると install が失敗するので、新しい依存を
  足したときは `allowBuilds` に `true` / `false` を書いてください。現在は `esbuild: false` の 1 件だけで、
  これはプラットフォーム別のバイナリを選ぶだけのスクリプトであり、そのバイナリは pnpm が optional な依存として
  すでに入れているため不要です
- Cargo: 同等の設定は Cargo 本体に未実装です(RFC 3923 `registry.global-min-publish-age` が承認済みで、クライアント側の実装待ち)。
  `Cargo.lock` をコミットし、`cargo update` を自動では実行しません。クライアント側が安定したら `.cargo/config.toml` に設定を追加してください

## ディレクトリ構成

```
draftpad/
  build.mjs             esbuild によるバンドル(dist/)。--serve で開発サーバー
  lint-style.mjs        style.css がトークンだけで組まれているかの検査(pnpm lint:style)
  playwright.config.ts  E2E テストの設定(chromium / webkit の 2 project)
  tsconfig.test.json    tests/ 用。Node の型を足すためだけに分けてある
  tests/e2e/
    fixtures.ts         アプリを起動する launch フィクスチャと共通のロケータ
    global-setup.ts     harness/backend.ts を harness/dist/ へバンドル
    harness/backend.ts  偽の Rust 側(mockIPC)。invoke を記録し、状態を返す
    *.spec.ts           起動 / 編集 / 環境設定 / ショートカット / メニュー
  src/
    main.ts             起動処理と各部品の配線
    editor.ts           CodeMirror の構成(Compartment で動的切替)
    languages.ts        言語一覧と遅延ロード
    modes/              Batch / Solidity / PHP の自作ハイライト(簡易的なパーサ)
    state.ts            永続化する状態と保存のデバウンス
    commands.ts         コマンド表(メニュー・ショートカット共用)
    preferences.ts      環境設定パネル
    statusbar.ts        ステータスバー
    theme.ts            ライト / ダークの解決
    style.css           ステータスバーとパネルのスタイル。寸法と色のトークンもここ
  src-tauri/
    src/lib.rs          Tauri Builder。起動時のウィンドウサイズ復元
    src/state.rs        state.json の読み書き(原子的書き込み)
    src/commands.rs     load_state / save_state / list_fonts / quit_app
    src/menu.rs         macOS のメニュー
    src/jumplist.rs     Windows のタスクバーメニュー(ジャンプリスト)
    src/autofill.rs     Windows の WebView2 オートフィル抑止
    src/fonts.rs        フォント列挙
    installer.nsi       Windows インストーラ(NSIS)のテンプレート
    tauri.conf.json     ウィンドウ・バンドル設定
    capabilities/       webview に許可する API
```
