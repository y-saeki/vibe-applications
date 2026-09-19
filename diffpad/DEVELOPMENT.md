# diffpad の開発

diffpad をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

フロントエンドは CodeMirror 6 で、2 ペインの差分表示は [`@codemirror/merge`](https://github.com/codemirror/merge) の
`MergeView` です。シェルは Tauri v2(Rust)です。配布対象は macOS(Apple Silicon)と Windows(x64)の 2 つで、
どちらも GitHub Actions でビルドします。

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
cd diffpad
pnpm install          # フロントエンドの依存を取得
pnpm tauri dev        # 開発モードで起動(フロントエンドを監視し、Rust 側を自動ビルド)
```

開発サーバのポートは 1421 です(draftpad の 1420 と並行して動かせるように、1 つずらしてあります)。

配布用ビルド:

```sh
pnpm tauri build
```

成果物は `src-tauri/target/release/bundle/` 以下に出力されます。

- macOS: `macos/diffpad.app`、`dmg/diffpad_<version>_aarch64.dmg`(Apple Silicon 向け)
- Windows: `nsis/diffpad_<version>_x64-setup.exe`

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

`diffpad/` を変更する Pull Request では、GitHub Actions が macOS / Windows の両方をビルドします。両方が成功すると、
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

差分そのもの(どの行・どの文字に印が付くか、行アキ、件数、移動)は `tests/e2e/diff.spec.ts` にあります。印は
`MergeView` が付ける `.cm-changedLine` / `.cm-changedText` / `.cm-changedLineGutter` を数えて確認し、行アキは
`.cm-mergeSpacer` の数と両ペインの同じ行の縦位置で、移動はキャレットのある行番号で確認します(`fixtures.ts` の
`gaps` / `caretLine`)。

開発サーバは Content-Security-Policy を送りません。配布物はこれを持つため(`src-tauri/tauri.conf.json` の
`csp`)、ポリシーが禁じている読み込みはテストでは通り、インストールしたアプリでだけ失敗します。
`tests/e2e/csp.spec.ts` だけが同じポリシーの下でページを開くので、`data:` URI のような自分のファイル以外の
読み込みを足したときはここに追います。

届かない範囲もあります。webview の中に無いものは一切見えません。

- macOS のメニューバーそのもの
- IME の未確定文字列と変換候補の位置
- 常に手前に表示・フルスクリーン・ウィンドウサイズが実際にどうなるか
  (`invoke` が正しく呼ばれたところまでは確認します)
- 展開したドロップダウンの macOS での見た目。Playwright が同梱する WebKit は 26.0 で、
  `appearance: base-select` は WebKit 27 からなので、プラットフォームのメニューが開く側の経路しか
  通りません。該当のテストは `CSS.supports()` ではなく実際に base appearance が乗ったかを見て自分を
  スキップするため、Playwright が 27 以降を同梱したら自動的に走り始めます
- コード署名していない配布物を各 OS が警告する挙動
- Windows インストーラの画面と挙動

これらは実機で確認するしかありません。裏を返せば、実機で見るべきものはこの一覧に絞られます。

### CI

`diffpad/` を変更する Pull Request では、上の 2 つが両方走ります。E2E テストは `ubuntu-latest` で 1 回、
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

バージョンの検査だけは `.github/workflows/diffpad-version.yml` という別のワークフローです。ラベルの付け外しでも
走らせる必要があり、それを `diffpad.yml` に足すと macOS / Windows のビルドまで巻き添えで走ってしまうためです。
数秒で終わるので、ラベルを触るたびに走っても実害はありません。

こちらには `paths` を書いていません。スキップされたジョブはステータスを報告せず、required status check に
指定したものが報告されないと Pull Request はいつまでもマージできなくなるためです。代わりに全部の Pull Request で
走り、`diffpad/` 以下が 1 つも変わっていなければジョブ自身が何もせずに通します。required に指定するならこちらだけ
安全です(`diffpad.yml` 側のジョブは `paths` で絞っているので、指定すると同じ理由で詰まります)。

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
`Cargo.toml` を書き換えたら `cargo update -p diffpad` で `Cargo.lock` も追随させてください。

`main` ブランチで `src-tauri/tauri.conf.json` の `version` が上がると、GitHub Actions が
`diffpad-v<version>` タグの Release を作り、上記の 2 つを添付します。上げずにマージした場合、タグが既に存在するため
Release は作られません。

### 上げ忘れを CI が止めます

上げ忘れても Release ジョブは黙ってスキップするだけなので、`diffpad-version.yml` が Pull Request を赤くします。
`diffpad/` を変更する Pull Request では、次のどれかに当たると落ちます。

- バージョンが base ブランチの先端と同じ、または古い
- 上の 4 つのファイルのバージョンが食い違っている

`diffpad/` 以下に変更が 1 つもない Pull Request は対象外です(その場合は何も確認せずに通ります)。base ブランチに
まだ `diffpad/` が無い Pull Request(diffpad を追加するもの)も、比べる相手が無いので通ります。

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

## 差分の計算と表示

2 ペインは `@codemirror/merge` の `MergeView` 1 つです(`src/editor.ts`)。左が `a`、右が `b` で、`MergeView` が
2 つの `EditorView` を作り、どちらかの文書が変わるたびに差分(chunk の列)を計算し直して両方に配ります。差分は
編集のたびに同期的に、全文を対象に計算します(下記「行の対応付け」)。

### 行の対応付け

`@codemirror/merge` の差分は文字単位で、変更された文字の並びを行の境界で区切って chunk にします。これを
そのまま使うと、左右で行を対応付ける表示としては困ることが 2 つありました。

- 末尾に行を足すと、文字としては直前の行の末尾に改行を足したことになるので、直前の行が両側で変更扱いになり、
  足した行の向かいに空きが出ない
- 削除と挿入の間にある共通行が 3 文字未満だと 1 つの変更に併合され、共通行ごと塗られる

VS Code や difff は先に行同士を対応付け、その中で初めて文字を比べます。diffpad も同じ順にしています
(`src/linediff.ts`)。まず両方の文書の行を「同じ内容の行は同じ 1 文字」に置き換えた文字列を作り、それを
パッケージの `diff` にかけると、行単位の差分がそのまま得られます(文字は UTF-16 の 1 単位で、diff が途中で
割ってしまうサロゲート領域は避けているので、区別できる行の種類は 63,488 までです。超えたときだけパッケージ
本来の文字単位の計算に戻します)。得られた各ブロックを `Chunk` にし、ブロックの中で改めて `presentableDiff` を
かけて文字単位の印にします。

`MergeView` に chunk を差し込む口はありませんが、chunk は `Chunk.build` / `Chunk.updateA` / `Chunk.updateB`
という exported なクラスの静的メソッドから、必要になった時点で取り出されます。`installLineDiff` はこの 3 つを
差し替えます。パッケージの更新でこの呼び方が変わると黙って元の挙動に戻るので、`tests/e2e/diff.spec.ts` の
行アキのケースが番人です。

パッケージ本来の `updateA` / `updateB` は編集箇所の前後 1000 文字だけを計算し直しますが、差し替え後は
編集のたびに全文を計算します。行単位の pass は行数ぶんの文字列を比べるだけで、文字単位の pass は変更ブロックの
中だけなので、似た 2 つのテキストなら大きくても数 ms から数十 ms です。重くなるのは、大きな変更ブロックの中を
編集し続けるときで、そのブロックの文字単位の pass が毎回走ります。増分更新は、必要になったら足します。

### 行単位と文字単位

`MergeView` は chunk に含まれる行を両側で `.cm-changedLine` として塗り、`highlightChanges` が有効なら行の中で
異なる範囲を `.cm-changedText` として重ねて塗ります。README の「行単位」「文字単位」はこのオプションの
オフ / オンそのものです(`Editor.setDiffMode`)。差分の計算自体は両方のモードで同じなので、切り替えに計算コストは
ありません。設定は `state.json` の `diffMode` に `"line"` / `"char"` で残ります。

文字単位の印は、`@codemirror/merge` が差分を表示向けに整える(`presentableDiff`)ときに、異なる文字の前後を
単語の境界まで広げます。`foo bar` と `foo baz` なら `r` と `z` ではなく `bar` と `baz` に印が付きます。広げる幅は
前後 8 文字までなので、句読点や空白の無い長い日本語の並びでも、文全体が塗られることはありません。これは
パッケージの挙動で、設定はありません。

### 計算の打ち切り

`MergeView` の既定は `diffConfig: { scanLimit: 500 }` で、差分の深さ(変更文字数のおおよその上限)で精密な計算を
打ち切ります。これだと、200 行のうち 1 行おきに変わっている程度の 2 つのテキストでも 1 つの chunk に潰れました
(手元の計測。2000 行に 400 箇所の小さな編集がある散文でも同じです)。文書の 2 つの版を比べるのが diffpad の
用途なので、これでは役に立ちません。

そこで `scanLimit` は外し、時間で打ち切ります(`src/editor.ts` の `DIFF_TIMEOUT_MS`、500ms)。この 1 つの予算を
行単位の pass と各ブロックの文字単位の pass で分け合います(`linediff.ts` の `budget`)。似た 2 つのテキスト
なら精密な計算は数十 ms で終わるので、普通の使い方でこの上限に当たることはありません。当たるのは、内容が
まるで違う大きなテキスト同士を比べたときで、そのときは残りを粗い計算で埋めます(README の制限事項に書いて
ある「大まかな色付け」がこれです)。

### 件数と移動

ステータスバーの「差異 N 箇所」は chunk の数です。`MergeView` には差分が更新されたことを知らせる口が
無いので、左ペインの `updateListener` で `getChunks(state)` の配列が入れ替わったかを見ています(両ペインに
同じ配列が配られるので、片方で十分です)。

前後の差異への移動は `@codemirror/merge` の `goToNextChunk` / `goToPreviousChunk` で、末尾の次は先頭に戻ります。
動かすのは「最後にフォーカスを持っていたペイン」で、`Editor` が `focusChanged` を見て覚えています。元に戻す /
やり直すも同じペインに対して働きます。

### レイアウト

`MergeView` は 2 つのエディタのスクロールを自前では同期しません。代わりに各エディタの高さを内容に合わせて
伸ばし(`.cm-scroller` の高さを `auto` に固定)、chunk の高さの差を空白のウィジェット(spacer、`.cm-mergeSpacer`)で
埋めて行を揃え、外側の `.cm-mergeView` 1 つをスクロールさせます。`style.css` はこれに従い、`.cm-mergeView` に
ウィンドウの高さを与えています。spacer は片方のペインにしかない行の向かいに入る空きなので、VS Code と同じく
`--border` の斜線で塗り、空行と見分けが付くようにしています。

そのままだと短いテキストのペインは内容の高さしか無く、その下の余白をクリックしてもキャレットが入りません。
`.cm-mergeViewEditor` を縦の flex にしてエディタと `.cm-scroller` を下端まで伸ばし、`.cm-content` は自身の
`min-height: 100%` でそれに追随します(`tests/e2e/diff.spec.ts` の「下の余白をクリック」のケースが見ています)。

## プラットフォーム固有の実装

キーボードショートカットは、macOS ではメニューバーのアクセラレータとして、Windows ではアプリ内のキー処理として
実装しています。Windows にはメニューバーがありません。`src/commands.ts` が唯一のコマンド表で、
`src-tauri/src/menu.rs` の項目 id と一致しています。

元に戻す / やり直すは例外で、どちらのプラットフォームでも CodeMirror のキーマップが処理します。CodeMirror の
`historyKeymap` は「やり直す」の `Ctrl+Shift+Z` を Linux 向けにしか割り当てていないため、`src/editor.ts` で明示的に
割り当てています。macOS の Edit メニューでは、この 2 つだけ定義済み項目を使わずコマンド表へ転送しています。定義済み
項目は WKWebView 自身の undo マネージャを動かすもので、エディタの履歴には触れないためです。

次の差異 / 前の差異は `F7` / `Shift+F7` です。VS Code の差分エディタと同じで、CodeMirror の既定のキーマップは
ファンクションキーを使わないので、エディタ内のキーと競合しません(WinMerge の `Alt+↓` / `Alt+↑` は CodeMirror では
行の移動に割り当てられているため採りませんでした)。

Windows は IME の未確定文字列と変換候補をキャレットの位置に合わせて表示します。キャレットを持つ要素が
ウィンドウの中にないまま操作対象になると、合わせる先がないため画面左上に出てしまいます。そのため
ウィンドウは `visible: false` のまま起動し、フロントエンドが左ペインにフォーカスを移してから
`src/main.ts` が表示します(`src-tauri/src/lib.rs` には、フロントエンドが起動しなかったときのために
10 秒後に表示するフォールバックだけ残してあります)。ウィンドウが操作対象に戻ったときに中身の
どこにもフォーカスがなければ、最後に使っていたペインへ戻すようにもしています。

Windows の WebView2 はテキスト入力をフォームの一部とみなすため、環境設定のフォント欄に
フォーカスすると「保存された情報」の候補が出ます。diffpad にフォームはないので、起動時に
`ICoreWebView2Settings4` の `IsGeneralAutofillEnabled` と `IsPasswordAutosaveEnabled` を false にして
止めています(`src-tauri/src/autofill.rs`)。macOS の WKWebView にこの挙動はありません。

Windows のインストーラは Tauri の NSIS スクリプトをそのまま使っています(draftpad が入れている上書き
インストール向けの手直しは、まだ持ち込んでいません)。

## UI の寸法と色

ステータスバー・環境設定パネル・ペインのガター・差分の印は、`src/style.css` の `:root` にあるカスタムプロパティ
だけで組み立てます。UI ライブラリは入れていません。デザインガイドラインという別の文書も置いていません。
トークンの一覧そのものが仕様で、`pnpm lint:style` がそれを守らせます。値はここに書きません。CSS が唯一の
出どころで、文書に写すと片方が古くなるだけです。

| ファミリ | トークン | 役割 |
|---|---|---|
| 文字サイズ | `--font-size-title` / `-body` / `-caption` | 環境設定の見出し / パネル本文 / ステータスバーとバージョン表示 |
| 書体 | `--ui-font`、`--font-mono` | UI 全体 / バージョン表示。数の段階ではないので、予算の数え方も他と別です |
| 余白 | `--space-1` 〜 `--space-5` | 4px 刻みの 5 段。コントロール同士、バーとパネルの内側、グループ同士 |
| 角丸 | `--radius-sm` / `-md` / `-lg` | 部品の大きさに対応した 3 段(アイコンボタン / 高さ `--control-height` のコントロール / パネル) |
| 寸法 | `--control-height`、`--icon-size`、`--icon-button-size`、`--statusbar-height`、`--titlebar-height` | コントロールとバーの大きさ |
| レイアウト | `--field-width-narrow`、`--label-width`、`--panel-width`、`--panel-inset`、`--mode-width`、`--count-width` | 環境設定パネルの配置と、ステータスバーの件数セル・表示単位セレクタの幅 |
| パレット | `--bg`、`--fg`、`--muted`、`--muted-strong`、`--placeholder`、`--border` など | 色、影 2 段(`--shadow-panel` / `--shadow-popup`)、描き込む印 2 枚(シェブロン / ×)、差分の色 6 つ(`--diff-a-line` / `-text` / `-mark` と `b` 側) |

基準にしたのは Windows 11 のメモ帳のステータスバーで、draftpad と同じ寸法・同じパレットです。macOS では
`-apple-system`、Windows では Segoe UI が当たるだけで、寸法は共通です。OS ごとに変えたくなったら
`:root[data-platform="macos"]` でトークンを上書きしてください(`--titlebar-height` がすでにそうなっています)。

差分の色は `a`(左)が赤系、`b`(右)が緑系で、行の地色(`-line`)・文字の印(`-text`)・ガターの縞(`-mark`)の
3 つずつです。`@codemirror/merge` の既定は文字の印を下線で描きますが、diffpad は行より一段濃い地色で塗り
替えています。ライトとダークで別の値を持ちます。

エディタ本文のフォントサイズは設定項目なので、このトークンには含めません。CodeMirror が自分で描く部分のうち、
ペインの背景・キャレット・選択範囲だけは `src/dark-theme.ts` にあります(ダークのときだけ当てる必要があるため)。
ガターと差分の印はテーマに関係なく同じセレクタで済むので、`style.css` 側にあります。

### アイコンと淡い文字

歯車・ピン(常に手前に表示)・前後の差異のシェブロンは HTML に直接書いた SVG です。閉じるボタンの × は
`--close-icon` をマスクして描きます。文字として置くと、字形の中心と文字の送り幅の中心がずれる分だけボタンの
中央から外れ、その量がフォントによって変わります。擬似要素にしか届かないもの(× と、Windows のセレクトの矢印
`--chevron-down`)が CSS の画像で、HTML から触れるものが SVG です。

線の太さは 1.5 に揃えてあります。16×16 の viewBox に対しての値なので、SVG で描く印もマスクで描く印も、画面上では
ほぼ同じ太さになります。

`--muted-strong` は環境設定パネルのグループ見出しだけに使います。見出しは項目名より上位なので `--muted` では
沈みすぎ、`--fg` では項目名と同じ強さになります。

プレースホルダ(空のペインの「比較元のテキスト」「比較先のテキスト」、フォント欄の既定値)は `--muted` ではなく
`--placeholder` です。`--muted` はステータスバーやバージョン表示に使う「読ませる二次テキスト」で、プレースホルダは
「まだ何も入っていない」ことを示すものなので、同じ色だと入力済みに見えます。

### ドロップダウン

`<select>` を展開したリストは、既定ではプラットフォームが描く別ウィンドウで、CSS が一切届きません。
`appearance: base-select` を指定すると、リストがページの中の普通の要素(`::picker(select)`)になり、
このスタイルシートで組めるようになります。対応は Chromium 135 / WebKit 27 以降です。

**指定しているのは Windows だけです。**

| 環境 | 展開時の見た目 |
|---|---|
| Windows(WebView2) | こちらで組んだもの。WebView2 は自動更新されるため 135 以降になります |
| macOS(WKWebView) | macOS 自身のメニュー。`:root[data-platform="windows"]` で囲っているので、Safari が 27 以降になっても変わりません |

macOS のメニューはその OS の他の部分と揃っているので、そちらに任せるほうがよいという判断です。
閉じたピッカーはブラウザが `display: none` で隠しており、`::picker(select)` に `display` を書くとそれに勝ってしまって、
起動した瞬間から全部のセレクトのリストが開いたまま画面に乗ります。`tests/e2e/diff.spec.ts` がこれを見ています。

Windows 側では、矢印がプラットフォームのものから `::picker-icon` に移るので `--chevron-down` を `currentColor` で
マスクして塗り、base-select は選択中のラベルに合わせて縮むので、ステータスバーのセレクトには `--mode-width` を
下限として与えてバーが動かないようにしています。どちらも Windows だけに掛けてあります。

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
`src/dark-theme.ts` が読むトークンも「使われている」と数えます。

見ているのは px / em / rem だけです。`50vh` のようなビューポート基準の値は、段階を持つ寸法ではなく
その場のレイアウトなので、トークンにせず直接書きます。

検査が壊れて黙って通るようになるのを防ぐため、実行のたびに、まず `lint-style.mjs` 末尾のフィクスチャに対して
ルールを走らせます。上の各項目に 1 つずつ、報告されるはずの最小の CSS が並んでいて、どれかが報告されなく
なったら「この検査は主張どおりのことを見ていない」と言って落ちます。

### トークンを足すとき

個数の予算は `lint-style.mjs` の先頭にあります。上限は現在の個数そのままなので、1 つ足すには予算も上げる
必要があります。上げるのは構いません。ただ 1 行の差分として残るので、レビューで「本当に必要か」を必ず一度
通ることになります。

機械で決められないことが 3 つ残ります。

- **既存で足りないか。** 1〜2px の違いで新しい値が欲しくなったときは、たいてい既存に寄せたほうが揃います。
  足す前にこれを試してください
- **どのファミリか。** 段階のあるもの(文字サイズ・余白・角丸)は序数か大小で名付け、役割が 1 つに決まるもの
  (`--label-width` など)は用途で名付けます
- **1 箇所しか使わない値をトークンにするか。** します。在庫を 1 箇所にまとめるのが目的なので、使用箇所が
  1 つでも `:root` に置きます。ただし予算を食うので、既存で足りるならそちらが先です

## 環境設定パネル

環境設定は `<dialog>` で、`showModal()` で開きます。モーダルダイアログはトップレイヤーに置かれるため、
スタッキングコンテキストの外側に出ます。背面が操作不能になるのもブラウザ側の保証です。CodeMirror は自前の
スタッキング順を持っているので(ツールチップなどが `z-index` 付きでページのルートに積まれます)、普通の要素で
重ねるとその数字を上回る `z-index` を書き続けることになります。トップレイヤーならその競争に参加しません。
`src/style.css` に `z-index` が 1 つもないのはこのためです。

2 点だけ注意があります。

- `dialog:not([open]) { display: none }` はブラウザ標準のスタイルなので、作成者スタイルの `display` に
  負けます。`#preferences` の表示切り替えを明示的に書いているのはこのためです。
- 暗幕は `::backdrop` ではなくダイアログ要素自身の背景で描いています。`::backdrop` が元の要素から
  カスタムプロパティを継承するようになったのは Safari 17.4 からで、それ以前では `var(--overlay)` が
  届きません。ブラウザ自身が `::backdrop` に付ける薄い黒はエンジンごとに濃さが違うので、透明にしてあります。

閉じたときは、開く前にフォーカスを持っていたペインへキーボードを返します。

## `state.json`

両方のテキストと設定は 1 つの `state.json` にまとめて、Tauri の app-data ディレクトリに置きます。

- macOS: `~/Library/Application Support/com.ysaeki.diffpad/state.json`
- Windows: `%APPDATA%\com.ysaeki.diffpad\state.json`

書き込みは 300ms のデバウンス後と、ウィンドウがフォーカスを失ったとき、終了の直前です。一時ファイルに書いて
`fsync` してから rename するので、途中でプロセスが落ちても前の内容か新しい内容のどちらかが残ります。読めない
ファイルは `state.json.broken` に退避して既定値で起動します(`src-tauri/src/state.rs`)。フィールドは
`src/state.ts` の `State` と 1 対 1 で、名前は TypeScript 側の camelCase(`textA` / `textB` / `diffMode` …)です。
