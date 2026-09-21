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
- 右クリックで開くコンテキストメニューそのもの。どちらのプラットフォームでもネイティブのメニューです。
  webview の既定のメニューを止めたのがどちらのプラットフォームか、`show_context_menu` に何を渡したかまでを
  確認します。Windows 側の項目の絞り込みは webview の外なので届きません
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

## 空白文字の表示

環境設定の「空白文字を表示」を入れると、本文の空白文字に印が付きます(`src/whitespace.ts`)。設定は
`state.json` の `showWhitespace` に残り、エディタ側では `src/editor.ts` の Compartment 1 つで、他の設定と同じく
エディタを作り直さずに差し替えます。

印を付けるのは 3 種類です。

| 文字 | 付くクラス | 出どころ |
|---|---|---|
| タブ | `.cm-highlightTab` | `@codemirror/view` の `highlightWhitespace` |
| スペース (U+0020) | `.cm-highlightSpace` | 同上 |
| 全角スペース (U+3000) | `.cm-ideographicSpace` | `src/whitespace.ts` の `MatchDecorator` |

`highlightWhitespace` が見ているのは `/\t| /` だけで、全角スペースは通りません。日本語を書く道具でこれを
落とすと、「印が付いていない = 空白がない」と読めてしまい、この設定を入れた当人がいちばん見つけたい文字が
見えないままになります。そのため同じ形の印を付ける規則を 1 つ足しています。

改行と文末には印を付けません。空白文字に印が付けば行の終わりは自明で、全行に印が並ぶ分だけうるさくなります
([#79](https://github.com/y-saeki/vibe-applications/issues/79))。

印の描き方は `src/style.css` です。スペースの点は放射グラデーション、タブの矢印は `--whitespace-tab` を
マスクして `--whitespace` で塗ります。どちらも長さを 1 つも持ちません。文字が占める箱そのものを基準に、
点の半径は行の高さに対する割合、矢印はその高さに合わせた `contain` で決めています。本文のフォントサイズは
設定項目なので、印だけ取り残されないのがこのためで、半角と全角のスペースが同じ大きさの点になるのも、幅では
なく高さから取っているためです。`--whitespace` は半透明で、下にあるもの(地の色、検索の一致、選択範囲)の
色をわずかに受けます。

タブの矢印はスパンをその形に切り抜いて描くので、スパンの中に何かが描かれていれば一緒に切り抜かれます。
検索の一致や選択の一致と入れ子になるときは必ず空白文字の側が内側なので実際には起きませんが、それが前提だと
コードからは読めないため、`tests/e2e/preferences.spec.ts` が子要素を持たないことを見ています。

## インデントガイド

環境設定の「インデントガイドを表示」を入れると、インデントの段ごとに縦線が引かれます(`src/indent-guides.ts`)。
設定は `state.json` の `showIndentGuides` に残り、空白文字の印と同じく `src/editor.ts` の Compartment 1 つで
差し替えます。

段の幅は「タブ幅」の設定です。行頭の空白を桁数で数え、タブ幅で割った商をその行の段数とします。CodeMirror が
タブを並べるのに使う `tab-size` と同じ数字なので、線はタブが刻む位置に立ちます。段を埋めない端数は段を
開きません。行頭として数えるのはスペースとタブだけです。全角スペースは文章そのものの文字で、幅もタブ幅の
倍数になりませんから、そこから線を引くと桁と桁の間に立ちます。

### 線は行の背景で、文字に付ける飾りではない

縦線は行の高さいっぱいに伸びて、上下の行の線とつながっていなければ縦線に見えません。行頭の空白文字の側に
飾りを付ける方法(マークデコレーションの `border-left` など)だと、その箱の高さは行ではなく文字の高さなので、
縦に並べたときに破線になります。ブロックの途中にある空の行には、掛ける文字すらありません。

そこで、段数を持たせた行デコレーションを行に付け、`src/style.css` が行の箱に繰り返しグラデーションでその数
だけ線を描きます。行の箱には上下の隙間がないので、線は行をまたいでつながります。渡す値は
`--indent-guide-levels`(その行の段数)と `--indent-guide-step`(1 段の桁数)の 2 つで、どちらも長さでは
なく数です。長さにするのはスタイルシートの側で、`ch` を掛けます。本文のフォントサイズは設定項目なので、
線だけ取り残されないのがこのためです。等幅フォント以外を指定すると、この `ch`(「0」の送り幅)と、タブと
スペースの送り幅とが揃わないぶんだけ線が文字からずれます。

組み立て直すのは表示中の行だけです(`ViewPlugin` と `view.visibleRanges`)。

### 空の行

ブロックの途中にある空の行は、前後にある「空白以外を含む行」のうち浅いほうの段数を取ります。同じ深さの
行に挟まれていれば線はそのまま通り、ブロックが終わったあとの空行では線が伸びません。その行自身が持って
いる空白は数えません。消し忘れの空白であることが多く、そこから段数を決めると線の本数が行ごとに揺れます。

### 折り返した行

長い行は折り返されます(`EditorView.lineWrapping`)。線は行の箱に描くので、折り返した先の行にも同じ数
だけ続きます。折り返した 2 行目以降が字下げされないのは CodeMirror の既定なので、そこでは線が文字の後ろを
通ります。

## 行番号

環境設定の「行番号を表示」を入れると、本文の左に行番号の列が付きます。設定は `state.json` の
`showLineNumbers` に残り、他の表示設定と同じく `src/editor.ts` の Compartment 1 つで差し替えます。

描くのは `@codemirror/view` の `lineNumbers()` です。インデントガイドと違って自前では持ちません。
必要なものはガターの側にすべてあります。表示中の行だけを組み立てること、折り返した行に番号を 1 つ
だけ付けること、下書きの行数が届く桁の幅を先に確保して、スクロールしても本文が横に動かないことです。

### 余計な面を外す

CodeMirror の既定のガターはパネルです。自分の地色を持ち、本文との境に罫線を引きます。`src/style.css`
はその 2 つを外します。列にあるのは数字だけで、列の終わりは数字の終わりです。本文の脇にもう 1 つ面が
増えるのではなく、本文が数えられている余白になります。

字の色は `--line-number` です。空白文字の印(`--whitespace`)とインデントガイド(`--indent-guide`)と
同じ色を土台に、透明度だけが違う 3 段になっています。濃い順に 行番号 > 空白文字の印 > インデント
ガイドで、そのさらに上が本文です。印とガイドは読み飛ばすもの、番号は読むものなので、この順です
([#88](https://github.com/y-saeki/vibe-applications/issues/88))。

左右の余白は左が `--space-3`、右が `--space-1` です。列は右端(一の位)から読むので、狭いほうを本文側
に置きます。本文との隙間は、行自身が持っている `--space-3` がここに足された分です。

### 番号は本文のフォントで組む

ガターはスクローラの中にあるので、`.cm-scroller` に当てている書体と太さ、`&` のフォントサイズを
そのまま継ぎます。本文のフォントサイズは設定項目なので、番号だけ取り残されません。

折り返した行に付く番号は 1 つで、その行が始まる段に立ちます(`EditorView.lineWrapping`)。数え方は
ステータスバーの「n 行」と同じです。

### ステータスバーからは切り替えません

頻繁に変えるものではないので、環境設定だけに置きます。ステータスバーにあるトグルは「常に手前に表示」
だけで、そちらは書いている最中に切り替えるものです。

## スクロールバー

縦のスクロールバーは draftpad が自分で描きます(`src/overlay-scrollbar.ts`)。本文の脇ではなく上に重ねるので、
下書きがウィンドウに収まらなくなってもエディタの幅は変わりません。プラットフォームのスクロールバーは
レイアウトの一部で、出た瞬間にその幅だけ本文が狭くなります。ウィンドウの高さを変えている最中にこれが
出たり消えたりすると、折り返し位置がそのたびに動いて表示がガタつきます
([#85](https://github.com/y-saeki/vibe-applications/issues/85))。

重ねる指定はどのエンジンにもありません。`overflow: overlay` は Chromium 114 以降 `auto` の別名で、
`::-webkit-scrollbar` に幅を与えると重ねる側から場所を取る側に変わります。macOS が重ねて描くかどうかは
OS の設定次第です。そのため `src/style.css` がプラットフォームのバーを消して、代わりにこちらで描きます。
消すのに 2 つ書いているのは、標準の `scrollbar-width` が Safari 18.2 以降のもので、そこまで更新していない
macOS には `::-webkit-scrollbar` しか届かないためです。

対象は draftpad 自身がスクロールさせる 2 箇所、下書き(`.cm-scroller`)と環境設定パネル(`.panel`)です。
Windows で `<select>` を展開したリストは中に要素を足せないので、そこは webview のバーのままです。

### 横のバーは描きません

プラットフォームのバーは片方の軸だけ消すことができません。`scrollbar-width` も `::-webkit-scrollbar`
も縦横まとめてです。つまり横のバーも消えているので、横に溢れる中身があれば手が届かなくなります。

描かずに済ませられるのは、そもそも横に溢れる場所がないからです。下書きは全部の行が折り返します
(`EditorView.lineWrapping`)。環境設定パネルはラベル列が固定で、入力欄の列が残りを取ります。その列は
`minmax(0, 1fr)` で下限を持ちません。`1fr` だけだと下限が入力欄自身の min-content 幅になり、最小の
ウィンドウ幅では行がパネルの外へ出ます。

これは「いまのところそうなっている」ことなので、E2E テストで固定してあります。下書き側
(`tests/e2e/editing.spec.ts`)は `scrollWidth === clientWidth` を、パネル側
(`tests/e2e/preferences.spec.ts`)は「どのコントロールもパネルの content edge を越えないこと」を
求めます。パネル側が使うウィンドウ幅は `src-tauri/tauri.conf.json` の `minWidth` を読むので、
そこを下げればテストも新しい幅で走ります。

パネルだけ求めるものが違うのは、WebKit がテーマ行のネイティブな `<select>` のところで、パネルを
実際より 11px ほど広く測るためです。そこには要素の箱が 1 つもなく、隠れている中身もありません。
`scrollWidth` で見るとこれに引っかかるので、横のバーが要るかどうかを決めている条件——
content edge を越える中身があるか——をそのまま見ます。`.panel` は `overflow-x: hidden` なので、
この 11px を横スクロールで覗くこともできません。

どちらかが横に溢れるようになったら CI が落ちます。そのときは、溢れないように直すか、横のバーを
足すかのどちらかです。

### 長さを決めるのはスタイルシート

`src/overlay-scrollbar.ts` がバーに渡すのは 2 つの数だけです。`--scrollbar-cover`(内容のうち画面に
出ている割合)と `--scrollbar-progress`(どこまで送ったか)で、どちらも長さではありません。つまみの
長さと位置はこの 2 つから `src/style.css` が組み立てます。インデントガイドと同じ形です。

`--scrollbar-thumb-min` より短いつまみにはしません。長い下書きではつまみが点になり、見えなくなるのと
同時に掴めなくもなります。この長さを `style.css` が 2 回書いているのは、`:root` の 1 つのトークンに
まとめられないためです。カスタムプロパティの中の `var()` はそれを宣言した要素で解決されるので、
`:root` に置くと割合が常に初期値の 0 になります。

バーの位置はビューポート基準(`position: fixed`)で、`src/overlay-scrollbar.ts` が毎回置き直します。
どちらのバーも包含ブロックを用意しなくて済み、検索パネルが開いて下書きが下へずれるような場合にも
そのまま追随します。環境設定パネルのバーだけはダイアログの中に入れます。ダイアログはトップレイヤーに
あり、その上に描けるものが他にないためです。

### 出ている条件

バーを出すのはスクロールだけです。止めてから `IDLE_MS` 後にフェードアウトします。出ている間に
つまみへポインタが乗れば、離れるまで出したままになります。掴もうとしている最中に消えないためです。

ポインタが対象の上にあること自体は条件にしていません。書いている間ずっとそこにあるので、条件に
入れるとバーが消えなくなり、重ねて描いた意味がなくなります。

ポインタを受け取るのは出ている間のつまみだけです。消えているバーの下——行末の余白——をクリック
したときに、下書きではなくバーに当たってしまうのを避けるためです。
`prefers-reduced-motion: reduce` では出入りのフェードを切ります。

つまみはドラッグでスクロールできます。溝の余白には何も割り当てていません。ポインタを受け取らないので、
押すと下書きの側に当たります。重ねて描くバーは普段消えていて、狙って押す場所ではないという判断です。

## プラットフォーム固有の実装

キーボードショートカットは、macOS ではメニューバーのアクセラレータとして、Windows ではアプリ内のキー処理として
実装しています。Windows にはメニューバーがありません。

元に戻す / やり直すは例外で、どちらのプラットフォームでも CodeMirror のキーマップが処理します。CodeMirror の
`historyKeymap` は「やり直す」の `Ctrl+Shift+Z` を Linux 向けにしか割り当てていないため、`src/editor.ts` で明示的に
割り当てています。macOS の「編集」メニューでは、この 2 つだけ定義済み項目を使わずコマンド表へ転送しています。定義済み
項目は WKWebView 自身の undo マネージャを動かすもので、エディタの履歴には触れないためです。

`⌘ ⇧ V` / `Ctrl + Shift + V`(プレーンテキストとして貼り付け)は、クリップボードのテキストを読んで
カーソル位置へ挿入します。draftpad はプレーンテキストしか扱わないので、`⌘ V` / `Ctrl + V` と結果は
同じです。macOS のメニューでは「編集」に「プレーンテキストとして貼り付け」を足しています。定義済みの貼り付け項目は
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

macOS では、AppKit が「編集」メニューに勝手に 4 つの項目を足します。Writing Tools、AutoFill、
「音声入力を開始…」「絵文字と記号」です。どう組み立てたメニューでも足されるので、こちらで外しています
(`src-tauri/src/edit_menu.rs`)。後ろ 2 つには `NSDisabledDictationMenuItem` と
`NSDisabledCharacterPaletteMenuItem` というデフォルト値のスイッチがあるので、アプリの起動中に
`registerDefaults` で登録します(書き込みではないので、draftpad の設定ファイルには何も残りません)。
前の 2 つにスイッチはないため、起動が終わってから——AppKit が足し終わるのがそこなので——メニューから
直接取り除きます。draftpad 自身の最後の項目(「検索・置換」)より後ろを全部外す、という形にしてあるので、
取り除く側が項目の名前を知っている必要はありません。`RunEvent::Ready` がそのタイミングです。

右クリックメニューが出るのは、テキストを編集できる場所——下書きと、検索・置換パネル / 環境設定パネルの
テキスト入力欄——だけです。ステータスバーやボタン、チェックボックスの上では、切り取りもコピーも貼り付けも
対象を持たないため、メニューごと出しません。Windows は `ICoreWebView2ContextMenuTarget` の `IsEditable`、
macOS はページ側で `contextmenu` の `target` を見て判定します。

消したい項目を消せる手段はプラットフォームごとに違うため、経路は 2 つあります。

Windows は WebView2 が `ContextMenuRequested` を上げてくれるので、WebView2 自身にメニューを描かせたまま、出る
項目だけを差し替えています(`src-tauri/src/context_menu.rs`)。見た目も文言も WebView2 のもの、つまり OS の
言語設定どおりのままで、編集項目の配線もそのままです。残すのは `undo` `redo` `cut` `copy` `paste` `selectAll`
の 6 つで、それ以外は落とします。消す物ではなく残す物を並べているのは、後の Edge が項目を足しても——絵文字や
Writing Tools がそうだったように——勝手には出てこないようにするためです。デバッグビルドでだけ `inspectElement`
も残します。Windows にはメニューバーがなく、ここが開発者ツールへの唯一の入り口だからです。「検索・置換」は
対応する既定の項目がないので、`CreateContextMenuItem` で足して `CustomItemSelected` をコマンド表へ転送します。

macOS には同じ仕組みがありません。WKWebView のメニューは「調べる」「翻訳」「共有」「音声」「フォント」「変換」
「変形」「段落の方向」「サービス」「Writing Tools」まで並びますが、項目単位では削れません。WebKit が
`NSMenuItem` に付ける `WKMenuItemIdentifier` は「コピー」「ペースト」「調べる」などにはあるものの、「切り取り」
「すべてを選択」「フォント」「サービス」には付かないためです。識別子で残す物を選ぶと切り取りが消え、識別子で
消す物を選ぶとフォントやサービスが残ります。

そこで macOS ではページ側でメニューごと止め(`src/context-menu.ts` が `contextmenu` を `preventDefault` します)、
代わりに Tauri のネイティブメニューを `popup` で出しています(`src-tauri/src/menu.rs` の `show_context`)。位置は
渡していません。省くと muda がポインタの位置に出し、画面の端に収まるよう寄せます。

こちらの切り取り・コピー・貼り付け・すべてを選択は定義済み項目です。responder chain を通じて、キャレットを
持っている要素——下書き・検索欄・環境設定の入力欄——にそのまま届きます。ただしラベルは自分で書きます。muda の
定義済み項目が持っているのは `Cu&t` のような英語の文字列で、OS のローカライズ済みの表記ではありません。
Windows 側は WebView2 の表記なので日本語で出ますが、macOS 側は指定しないと英語のままです。

定義済み項目は有効・無効を切り替えられないので、選択範囲がなくても灰色にはならず、押しても何も起きないだけ
です。元に戻す・やり直す・検索・置換はコマンド表へ転送する項目なので、`show_context_menu` が受け取る 3 つの
真偽値で灰色にできます。Windows 側にこの区別はなく、「検索・置換」は常に押せます。環境設定を開いている間に
押されても困らないよう、コマンド表の `openSearch` 自体がパネルの開閉を見ています。

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
| 寸法 | `--control-height`、`--checkbox-size`、`--toggle-width`、`--toggle-size-search`、`--glyph-size`、`--icon-size`、`--icon-button-size`、`--statusbar-height`、`--titlebar-height`、`--scrollbar-size`、`--scrollbar-thumb-size`、`--scrollbar-thumb-min` | コントロールとバーの大きさ、スクロールバーの溝とつまみ |
| レイアウト | `--field-width`、`--field-width-narrow`、`--field-width-search`、`--button-width-search`、`--label-width`、`--panel-width`、`--panel-inset`、`--language-width`、`--count-width`、`--count-width-narrow` | 入力欄・ラベル列・検索パネルと環境設定パネルの配置と、ステータスバーの文字数・行数セルの幅 |
| パレット | `--bg`、`--fg`、`--muted`、`--muted-strong`、`--placeholder`、`--border` など | 色、影 2 段(`--shadow-panel` / `--shadow-popup`)、描き込む印 8 枚(✓ / シェブロン上下 / × / 横棒 / Aa / .* / タブの矢印)、空白文字の印(`--whitespace`)、インデントガイドの色(`--indent-guide`)、行番号の色(`--line-number`)、スクロールバーのつまみの色 2 段(`--scrollbar-thumb` / `--scrollbar-thumb-hover`)と、`src/indent-guides.ts` と `src/overlay-scrollbar.ts` が入れる 2 つずつの数(`--indent-guide-levels` / `--indent-guide-step`、`--scrollbar-cover` / `--scrollbar-progress`。色でも長さでもありませんが、ファミリにも入らないのでここで数えます) |

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

線の太さは 6 枚とも 1.4〜1.5 に揃えてあります。16×16 の viewBox に対しての値なので、`--glyph-size` で
描く印も `--icon-size` で描く印も、画面上ではほぼ同じ太さになります。タブの矢印は行の高さに合わせて
拡大しますが、これもその範囲に収まります。ここを 1 枚だけ太くすると、その印だけ別の出どころから
持ってきたように見えます。

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

10 項目は「編集」「表示」の 2 グループに分けてあります。`<fieldset>` + `<legend>` で、既定の枠線と余白は
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
  build.mjs               esbuild によるバンドル(dist/)。--serve で開発サーバー
  lint-style.mjs          style.css がトークンだけで組まれているかの検査(pnpm lint:style)
  playwright.config.ts    E2E テストの設定(chromium / webkit の 2 project)
  tsconfig.test.json      tests/ 用。Node の型を足すためだけに分けてある
  tests/e2e/
    fixtures.ts           アプリを起動する launch フィクスチャと共通のロケータ
    global-setup.ts       harness/backend.ts を harness/dist/ へバンドル
    harness/backend.ts    偽の Rust 側(mockIPC)。invoke を記録し、状態を返す
    *.spec.ts             起動 / 編集 / 環境設定 / ショートカット / メニュー
  src/
    main.ts               起動処理と各部品の配線
    editor.ts             CodeMirror の構成(Compartment で動的切替)
    languages.ts          言語一覧と遅延ロード
    modes/                Batch / Solidity / PHP の自作ハイライト(簡易的なパーサ)
    state.ts              永続化する状態と保存のデバウンス
    commands.ts           コマンド表(メニュー・ショートカット共用)
    preferences.ts        環境設定パネル
    statusbar.ts          ステータスバー
    theme.ts              ライト / ダークの解決
    overlay-scrollbar.ts  本文に重ねて描く縦スクロールバー
    style.css             ステータスバーとパネルのスタイル。寸法と色のトークンもここ
  src-tauri/
    src/lib.rs            Tauri Builder。起動時のウィンドウサイズ復元
    src/state.rs          state.json の読み書き(原子的書き込み)
    src/commands.rs       load_state / save_state / list_fonts / quit_app
    src/menu.rs           macOS のメニュー
    src/jumplist.rs       Windows のタスクバーメニュー(ジャンプリスト)
    src/autofill.rs       Windows の WebView2 オートフィル抑止
    src/fonts.rs          フォント列挙
    installer.nsi         Windows インストーラ(NSIS)のテンプレート
    tauri.conf.json       ウィンドウ・バンドル設定
    capabilities/         webview に許可する API
```
