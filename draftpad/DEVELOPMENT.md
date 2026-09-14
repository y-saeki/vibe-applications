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
| 余白 | `--space-1` 〜 `--space-5` | 4px 刻みの 5 段。コントロール同士、バーとパネルの内側、グループ同士 |
| 角丸 | `--radius-sm` / `-md` / `-lg` | 部品の大きさに対応した 3 段(チェックボックスとアイコンボタン / 高さ `--control-height` のコントロール / パネル) |
| 寸法 | `--control-height`、`--checkbox-size`、`--icon-size`、`--icon-button-size`、`--statusbar-height`、`--titlebar-height` | コントロールとバーの大きさ |
| レイアウト | `--field-width`、`--field-width-narrow`、`--label-width`、`--panel-width`、`--panel-inset`、`--language-width` | 入力欄・ラベル列・環境設定パネルの配置 |
| パレット | `--bg`、`--fg`、`--muted`、`--placeholder`、`--border` など | 色、影 2 段(`--shadow-panel` / `--shadow-popup`)、マスク用の画像 3 枚(✓ / シェブロン / ×) |

基準にしたのは Windows 11 のメモ帳のステータスバーです。macOS では `-apple-system`、Windows では Segoe UI が
当たるだけで、寸法は共通です。OS ごとに変えたくなったら `:root[data-platform="macos"]` でトークンを上書き
してください(`--titlebar-height` がすでにそうなっています)。

エディタ本文のフォントサイズは設定項目なので、このトークンには含めません。CodeMirror が自分で描く部分の色は
`src/dark-theme.ts` にあり、構文ハイライトの色だけは直値です(パレットとは別の体系なので意図的にそうしています)。

### アイコンと淡い文字

歯車は HTML に直接書いた SVG です。閉じるボタンの × は `--close-icon` をマスクして描きます。文字として
置くと、字形の中心と文字の送り幅の中心がずれる分だけボタンの中央から外れ、その量がフォントによって
変わります。CodeMirror は自前の閉じるボタンに × の文字を書き込むので、そちらは文字を `font-size: 0` で
畳んで同じマスクを被せています。擬似要素にしか届かないもの(✓・シェブロン・×)が CSS の画像で、
HTML から触れるもの(歯車)が SVG です。

プレースホルダは `--muted` ではなく `--placeholder` です。`--muted` はステータスバーやバージョン表示に
使う「読ませる二次テキスト」で、プレースホルダは「まだ何も入っていない」ことを示すものなので、同じ色だと
入力済みに見えます。ライト・ダークとも背景に対して約 3.3:1 に揃えてあります。

### ドロップダウン

`<select>` を展開したリストは、既定ではプラットフォームが描く別ウィンドウで、CSS が一切届きません。
`appearance: base-select` を指定すると、リストがページの中の普通の要素(`::picker(select)`)になり、
このスタイルシートで組めるようになります。対応は Chromium 135 / WebKit 27 以降です
(MDN の browser-compat-data で確認できます)。

| 環境 | 展開時の見た目 |
|---|---|
| Windows(WebView2) | 常にこちらで組んだもの。WebView2 は自動更新されるため 135 以降になります |
| macOS(WKWebView) | Safari 27 以降ならこちらで組んだもの。それより古い macOS ではプラットフォームのメニュー |

行の間隔は `--space-1` です。ピッカーを開くと選択中の行にフォーカスが残るので、その隣をホバーすると
同じハイライトが 2 行に乗ります。詰めて並べているとこれが 1 つの縦長のブロックに見えてしまうため、
行を離してあります。

対応していないエンジンでは宣言が丸ごと無視され、従来どおりプラットフォームのメニューが開きます。
どちらでも同じコントロールで、新しい OS では周囲のクロームに揃う、という形です。そのため
base-select 用の記述に条件分岐は要りません。ネイティブの `<select>` はフレックスコンテナではないので
ボタン側の `gap` や `align-items` は効かず、OS のメニューは `option` の指定を読みません。

2 点だけ、指定しないと元の見た目が変わるところがあります。

- **矢印**。プラットフォームが描いていたものが `::picker-icon` に移ります。`--chevron` を
  `currentColor` でマスクして塗るので、テーマごとに画像を用意する必要はありません(チェックボックスの
  ✓ だけは `input` の擬似要素に背景色が届かないため、いまも色別に 2 つ持っています)
- **ステータスバーのセレクトの幅**。ネイティブの `<select>` は一番長い選択肢に合わせた幅を持つため、
  言語を変えてもバーは動きませんでした。base-select は選択中のラベルに合わせて縮むので、
  `--language-width` を下限として与えて同じ挙動に戻しています

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
