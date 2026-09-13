# draftpad の開発

draftpad をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

フロントエンドは CodeMirror 6、シェルは Tauri v2(Rust)です。配布対象は macOS(Apple Silicon)と
Windows(x64)の 2 つで、どちらも GitHub Actions でビルドします。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)(stable) | `rustup` でインストール |
| Node.js 22 以降 | |
| [pnpm](https://pnpm.io/) 10.16 以降 | `minimumReleaseAge` を使うため |
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
成果物へのダウンロードリンクを Pull Request にコメントします。push のたびにコメントを増やさず、同じコメントを
書き換えます。

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

届かない範囲もあります。webview の中に無いものは一切見えません。

- macOS のメニューバーそのもの、Windows のタスクバーメニュー(ジャンプリスト)
- IME の未確定文字列と変換候補の位置
- 常に手前に表示・フルスクリーン・ウィンドウサイズが実際にどうなるか
  (`invoke` が正しく呼ばれたところまでは確認します)
- コード署名していない配布物を各 OS が警告する挙動

これらは実機で確認するしかありません。裏を返せば、実機で見るべきものはこの一覧に絞られます。

### CI

`draftpad/` を変更する Pull Request では、上の 2 つが両方走ります。E2E テストは
`ubuntu-latest` で 1 回、`cargo test` は macOS / Windows のビルドと同じジョブの中です。
E2E テストが落ちると、その run に `playwright-report` が添付されます。

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

`main` ブランチで `src-tauri/tauri.conf.json` の `version` が上がると、GitHub Actions が
`draftpad-v<version>` タグの Release を作り、上記の 2 つを添付します。上げずにマージした場合、タグが既に存在するため
Release は作られません。リリースするつもりの変更では、マージ前にバージョンを上げてください。

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

ジャンプリストは起動のたびに登録し直します。インストーラや配布物には手を入れていないので、
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
- Cargo: 同等の設定は Cargo 本体に未実装です(RFC 3923 `registry.global-min-publish-age` が承認済みで、クライアント側の実装待ち)。
  `Cargo.lock` をコミットし、`cargo update` を自動では実行しません。クライアント側が安定したら `.cargo/config.toml` に設定を追加してください

## ディレクトリ構成

```
draftpad/
  build.mjs             esbuild によるバンドル(dist/)。--serve で開発サーバー
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
  src-tauri/
    src/lib.rs          Tauri Builder。起動時のウィンドウサイズ復元
    src/state.rs        state.json の読み書き(原子的書き込み)
    src/commands.rs     load_state / save_state / list_fonts / quit_app
    src/menu.rs         macOS のメニュー
    src/jumplist.rs     Windows のタスクバーメニュー(ジャンプリスト)
    src/autofill.rs     Windows の WebView2 オートフィル抑止
    src/fonts.rs        フォント列挙
    tauri.conf.json     ウィンドウ・バンドル設定
    capabilities/       webview に許可する API
```
