# draftpad

チャットなどに送る文章を下書きするための、単一バッファのテキストエディタです。
Tauri v2 + CodeMirror 6 で作られており、方針は「高速」「シンプル」「macOS / Windows 対応」。

- 本文は入力のたびに自動保存され、次回起動時にそのまま復元されます。ファイルの概念はありません
- 起動は速く、閉じると終了します(常駐しません)
- 日本語 IME 前提。既定フォントは OS ごとの日本語対応等幅フォント
- 20 言語のシンタックスハイライト、正規表現対応の検索・置換、単語補完、Vim モード
- ライト / ダークテーマ(OS 追従または固定)
- 文字数(コードポイント数)と行数の表示、「常に手前に表示」
- 環境設定はステータスバー右下の歯車から開けます(Windows ではタスクバーアイコンの右クリックメニューからも)

## 動作環境

- macOS 11 以降(Apple Silicon / Intel)
- Windows 10 (1809 以降) / Windows 11。WebView2 ランタイムが必要です(通常はプリインストール済み。無い場合はインストーラが自動で取得します)

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

### バージョニング

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や
挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `src-tauri/tauri.conf.json` の `version` の 1 箇所だけです。Release のタグ名、配布物のファイル名、
環境設定パネルの表示のすべてがここから決まります。`package.json` と `src-tauri/Cargo.toml` の `version` は実際には
参照されませんが(Tauri は `tauri.conf.json` に `version` があればそちらを使います)、紛らわしいので同じ値に揃えます。

`main` ブランチで `src-tauri/tauri.conf.json` の `version` が上がると、GitHub Actions が
`draftpad-v<version>` タグの Release を作り、上記の 2 つを添付します。上げずにマージした場合、タグが既に存在するため
Release は作られません。リリースするつもりの変更では、マージ前にバージョンを上げてください。

### macOS で「壊れているため開けません」と表示される場合

コード署名・公証を行っていないため、ダウンロードした `.app` は Gatekeeper に拒否されます。
「"draftpad.app" は壊れているため開けません。ゴミ箱に入れる必要があります。」と表示されますが、
ファイルは壊れていません。ダウンロード時に付与される `com.apple.quarantine` 属性が原因です。

`.app` を「アプリケーション」フォルダに移したうえで、次のコマンドで属性を外してください。

```sh
xattr -dr com.apple.quarantine /Applications/draftpad.app
```

macOS Sequoia (15.0) 以降では、以前あった Control クリック →「開く」による回避はできません。
自分でビルドした `.app` には quarantine 属性が付かないため、この操作は不要です。

### 検証コマンド

```sh
pnpm typecheck                     # TypeScript の型検査
pnpm build                         # フロントエンドのバンドル(dist/)
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
```

### Pull Request のビルド

`draftpad/` を変更する Pull Request では、GitHub Actions が macOS / Windows の両方をビルドします。両方が成功すると、
成果物へのダウンロードリンクを Pull Request にコメントします。push のたびにコメントを増やさず、同じコメントを
書き換えます。

リンク先のダウンロードには GitHub へのログインが必要です。成果物には保持期限があり、過ぎるとリンクは無効になります
(期限はコメントに書かれます)。

## 使い方

ウィンドウ全体がエディタです。書いた内容はそのまま保存され、コピーして貼り付け先へ送る、という使い方を想定しています。
コピーはプレーンテキストのみをクリップボードに書くので、貼り付け先に書式が混入しません。

### 環境設定を開く

| 方法 | macOS | Windows |
|---|---|---|
| ステータスバー右下の歯車 | ○ | ○ |
| コマンドパレットの「環境設定…」 | ○ | ○ |
| キーボードショートカット | ⌘ , | Ctrl + , |
| メニューバー `draftpad` → `Preferences…` | ○ | — |
| タスクバーアイコンの右クリックメニュー → `設定` | — | ○ |

Windows のタスクバーメニュー(ジャンプリスト)の項目はショートカットなので、押すと draftpad が
もう一度起動されます。すでに起動していれば、その 2 つ目のプロセスは引数を既存のウィンドウへ渡して
自分は終了します。このため Windows 版は多重起動しません(同じ `state.json` を 2 つのプロセスが
奪い合わないという利点もあります)。macOS 版の挙動は変わりません。

ジャンプリストは起動のたびに登録し直します。インストーラや配布物には手を入れていないので、
バージョンを上げたり別の場所へ移したりしても、次の起動で正しい実行ファイルを指し直します。

### キーボードショートカット

| 操作 | macOS | Windows |
|---|---|---|
| 環境設定を開く | ⌘ , | Ctrl + , |
| コマンドパレット | ⌘ ⇧ P | Ctrl + Shift + P |
| 検索・置換 | ⌘ F | Ctrl + F |
| 次を検索 / 前を検索 | ⌘ G / ⌘ ⇧ G | F3 / Shift + F3 |
| フォントを大きく / 小さく | ⌘ = / ⌘ - | Ctrl + = / Ctrl + - |
| フルスクリーン切替 | ⌃ ⌘ F | F11 |
| 閉じる(= 終了) | ⌘ W | Ctrl + W |
| 終了 | ⌘ Q | Ctrl + Q / Alt + F4 |
| 環境設定・パレットを閉じる | Esc | Esc |

macOS ではメニューバーのアクセラレータとして、Windows ではアプリ内のキー処理として実装しています。
Windows にはメニューバーがありません。

### 検索・置換

検索パネルには「正規表現」「大文字小文字を区別」「単語単位」のトグルがあります。
正規表現モードでは置換文字列に `$1` `$2` … と `$&` が使えます(例: `(\d+)` → `$1円`)。

### 環境設定の項目

| 項目 | 内容 |
|---|---|
| モード | Normal / Vim |
| テーマ | システムに合わせる / ライト / ダーク |
| フォントサイズ | 10〜100 px |
| フォント | CSS の `font-family` として解釈します。空欄なら OS 別の既定値。候補にはインストール済みフォントのファミリ名が出ます |
| タブ幅 | 1〜10 |
| 入力候補を表示 | 文書内の単語を候補として補完します |

既定フォント:

- macOS: `"Osaka-Mono", "Osaka−等幅", Menlo, "Hiragino Sans", monospace`
- Windows: `"BIZ UDGothic", "BIZ UDゴシック", "MS Gothic", "ＭＳ ゴシック", Consolas, "Yu Gothic", monospace`

macOS の Osaka は候補一覧では「Osaka」というファミリ名で出ます。等幅の面を使うには `Osaka-Mono` と入力してください。

### 対応言語

Markdown、プレーンテキスト、YAML、Batch、HTML、XML、Dockerfile、JavaScript、TypeScript、Ruby、Go、CSS、LESS、SCSS、
Solidity、MySQL、pgSQL、PHP、PowerShell、Rust。ステータスバーのセレクトかコマンドパレットで切り替えます。
Markdown のフェンスコードブロック(```js など)も同じ文法でハイライトされます。

## 設定と下書きの保存場所

設定と本文は 1 つの JSON にまとめて保存されます。書き込みは一時ファイルに書いてから置き換える方式なので、
途中でプロセスが落ちても壊れたファイルは残りません。壊れていた場合は `state.json.broken` として退避します。

- macOS: `~/Library/Application Support/com.ysaeki.draftpad/state.json`
- Windows: `%APPDATA%\com.ysaeki.draftpad\state.json`

ウィンドウのサイズは保存しますが、位置は保存しません(常に画面中央に開きます)。

## 依存関係の方針

Tauri 公式・CodeMirror 公式・Microsoft 公式以外の依存は次の 2 つだけです。

- `@replit/codemirror-vim`(Vim モード)。外す場合は `src/vim.ts` と `Editor` の `vim` Compartment を削除
- `font-kit`(フォント一覧の取得)。外す場合は `src-tauri/src/fonts.rs` と `list_fonts` コマンドを削除

Windows 向けのビルドだけが使う依存が 2 つあります。どちらもタスクバーメニューのためのもので、
外す場合は `src-tauri/src/jumplist.rs` と `lib.rs` の該当箇所をまとめて削除してください。

- `windows`(Microsoft 公式の Win32 バインディング)。ジャンプリストを作る Shell COM API に使います
- `tauri-plugin-single-instance`(Tauri 公式)。ジャンプリストから起動された 2 つ目のプロセスの
  引数を、動作中のインスタンスへ渡します

### 更新の運用(サプライチェーン対策)

- npm: `pnpm-workspace.yaml` の `minimumReleaseAge: 10080` により、公開から 7 日未満のバージョンは解決しません。
  `pnpm-lock.yaml` をコミットし、更新は `pnpm update` を明示的に実行したときだけ行います
- Cargo: 同等の設定は Cargo 本体に未実装です(RFC 3923 `registry.global-min-publish-age` が承認済みで、クライアント側の実装待ち)。
  `Cargo.lock` をコミットし、`cargo update` を自動では実行しません。クライアント側が安定したら `.cargo/config.toml` に設定を追加してください

## ディレクトリ構成

```
draftpad/
  build.mjs             esbuild によるバンドル(dist/)。--serve で開発サーバー
  src/
    main.ts             起動処理と各部品の配線
    editor.ts           CodeMirror の構成(Compartment で動的切替)
    languages.ts        言語一覧と遅延ロード
    modes/              Batch / Solidity / PHP の自作ハイライト
    state.ts            永続化する状態と保存のデバウンス
    commands.ts         コマンド表(メニュー・ショートカット・パレット共用)
    preferences.ts      環境設定パネル
    palette.ts          コマンドパレット
    statusbar.ts        ステータスバー
    theme.ts            ライト / ダークの解決
  src-tauri/
    src/lib.rs          Tauri Builder。起動時のウィンドウサイズ復元
    src/state.rs        state.json の読み書き(原子的書き込み)
    src/commands.rs     load_state / save_state / list_fonts / quit_app
    src/menu.rs         macOS のメニュー
    src/jumplist.rs     Windows のタスクバーメニュー(ジャンプリスト)
    src/fonts.rs        フォント列挙
    tauri.conf.json     ウィンドウ・バンドル設定
    capabilities/       webview に許可する API
```

## 制限事項

- 自動アップデートはありません。更新は新しいバイナリを取得してください
- 配布物はコード署名・公証をしていません(ダウンロードした macOS 版は[初回のみ属性の解除が必要](#macos-で壊れているため開けませんと表示される場合))
- コマンドパレットに出るのはアプリのコマンドのみで、エディタの全操作は含みません
- ウィンドウ位置は記憶しません(サイズのみ)
- Windows 版は多重起動しません(タスクバーメニューの項目が実行ファイルを起動し直す作りのため)
- Batch / Solidity / PHP のハイライトは簡易的な自作パーサです
