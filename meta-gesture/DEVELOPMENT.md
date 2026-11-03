# meta-gesture の開発

meta-gesture をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

ジェスチャーの判定、設定ファイル、設定画面の編集中データなど、動作を決める部分は OS に触れない Rust のライブラリです。
Windows 版はこのライブラリを使う単一の exe で、常駐部分は [`windows`](https://crates.io/crates/windows) クレート(Win32 API
のバインディング)、設定画面は [`windows-reactor`](https://crates.io/crates/windows-reactor)(Rust から WinUI 3 を使う
Microsoft のクレート)で組んでいます。macOS 版は同じライブラリを [UniFFI](https://mozilla.github.io/uniffi-rs/) で
Swift から呼ぶアプリです。構成は winwin と同じで、ビルドや署名の仕組みも winwin から引き継いでいます。配布するのは
Windows (x64) と macOS (Apple シリコン) です。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)(stable、1.95 以降) | `rustup` でインストール。`windows-reactor` が 1.95 以降を求めます |
| Windows: Visual Studio Build Tools | 「C++ によるデスクトップ開発」を選択。Rust は MSVC ツールチェーンを既定にする |
| macOS: Xcode のコマンドラインツール | Swift 5.9 以降 |
| [NSIS](https://nsis.sourceforge.io/) 3 | インストーラを作るときだけ |
| [PowerShell](https://learn.microsoft.com/powershell/) 7(`pwsh`) | インストーラを作るときだけ。同梱するファイルの一覧を書き出します |

Windows でのビルドは初回に Windows App Runtime のパッケージを nuget.org からダウンロードします(`build.rs` が呼ぶ
`windows-reactor-setup`)。

ライブラリは Windows でも macOS でもない環境でビルド・テストできます。Win32 側の型検査は `x86_64-pc-windows-msvc`
ターゲットで行えます(リンクはできません)。

```sh
rustup target add x86_64-pc-windows-msvc
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

## ビルドと実行

```sh
cd meta-gesture
cargo run                   # Windows: デバッグビルドで起動(終了はトレイメニューから)
cargo run -- --settings     # Windows: 設定画面だけを起動
cargo build --release       # Windows: 配布用ビルド
macos/build.sh              # macOS: macos/meta-gesture.app を作る
```

インストーラはリリースビルドのあとに同梱するファイルの一覧(`payload.nsh`)を書き出してから作ります。

```sh
cd meta-gesture/installer
pwsh payload.ps1 -Dir ../target/release -Out payload.nsh
makensis /INPUTCHARSET UTF8 /DVERSION=0.1.0 /DPAYLOAD=payload.nsh /DOUTFILE=../target/release/meta-gesture_0.1.0_x64-setup.exe installer.nsi
```

`macos/build.sh` は Rust のライブラリを静的ライブラリとしてビルドし、UniFFI で Swift 側のバインディングとヘッダーを
書き出してから、Swift のパッケージをビルドして `.app` にまとめます。書き出したファイルはコミットしません。

## 検証コマンド

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --features ffi
```

CI(`.github/workflows/meta-gesture.yml`)は `windows-latest` でこれらとリリースビルド、インストーラの作成を、
`macos-latest` で `ffi` 付きのテストと `.app`・`.dmg` の作成を行います。

## Pull Request のビルド

`meta-gesture/` を変更する Pull Request では、GitHub Actions が Windows 版と macOS 版をビルドし、Windows 版が成功すると
インストーラへのダウンロードリンクを Pull Request にコメントします。macOS 版も成功していれば、`.dmg` へのリンクも並べます。

## macOS 版の署名

macOS はアクセシビリティの許可を、許可した時点のアプリの署名の要件(designated requirement)と結び付けて記録します。
アドホック署名では、要件がアプリのハッシュ(`cdhash`)になります。そのため、ビルドし直すと要件が変わり、システム設定の
一覧では meta-gesture がオンのままなのに、新しいアプリには許可が効かず、イベントタップが動きません。

配布するアプリは自己署名の証明書で署名します。要件はバンドル ID と証明書
(`identifier "io.github.y-saeki.meta-gesture" and certificate leaf = H"…"`)になり、同じ証明書で署名する限り、
ビルドをまたいでも変わりません。公証はしないので、隔離属性を外す手順は変わりません。

CI の `build-macos` はリポジトリのシークレットの証明書を一時的なキーチェーンに読み込んで署名します。証明書を読み込んだ
のに要件に `certificate leaf` がなければ失敗します。シークレットを読めない Pull Request(フォークからのものと
Dependabot のもの)はアドホック署名でビルドし、`main` への push でシークレットがなければジョブを失敗させます。

| シークレット | 値 |
|---|---|
| `META_GESTURE_MACOS_CERTIFICATE_P12` | `.p12` を base64 にした文字列 |
| `META_GESTURE_MACOS_CERTIFICATE_PASSWORD` | `.p12` のパスワード |

証明書の作り方は winwin の DEVELOPMENT.md の「証明書の作り方」と同じです。要件はアプリごとにバンドル ID を含むので、
winwin と同じ証明書を登録しても構いません。鍵はリポジトリに入れません。作り直すと要件が変わり、利用者は一度だけ
許可し直す必要があります。そのリリースではリリースノートにそのことを書いてください。

手元では、証明書をキーチェーンに読み込んで `META_GESTURE_SIGN_IDENTITY="<証明書の名前>" meta-gesture/macos/build.sh`
とすると、許可を保ったままビルドし直せます。

## 構成

| パス | 内容 |
|---|---|
| `src/gesture.rs` | ジェスチャーの表記と、ポインタの軌跡から方向の並びを読み取る `Tracker` |
| `src/config.rs` | 設定ファイルの読み書きと検証、ジェスチャーがアプリで何になるか(`Config::lookup`) |
| `src/keys.rs` | 送るキーの組み合わせの表記、設定画面でのキーの記録、キーキャップの表示 |
| `src/vk.rs` / `src/mac.rs` | キーと Windows の仮想キーコード / macOS のキーコード・修飾フラグの変換 |
| `src/editor.rs` | 設定画面の編集中データ。Windows の画面が直接、macOS の画面が `ffi.rs` 越しに使います |
| `src/ffi.rs` | macOS アプリに見せる API(`ffi` フィーチャー) |
| `src/art.rs` / `src/icon_res.rs` | アイコンの絵と、exe に埋め込むリソース |
| `src/win/` | Windows 版。`app.rs`(常駐部分)、`mousehook.rs`(右ボタンの追跡)、`send.rs`(前面化とキー送信)、`process.rs`(ウィンドウからアプリを知る)、`toast.rs`(コマンド名の表示)、`settings_ui.rs`(設定画面) |
| `macos/` | macOS 版。`MouseTap.swift`(イベントタップ)、`Sender.swift`(アプリの特定、前面化とキー送信)、`Toast.swift`、`AppDelegate.swift`、`Settings/`(設定画面) |
| `installer/` | Windows のインストーラ(NSIS) |

## 設定ファイル

Windows は `%APPDATA%\meta-gesture\config.toml`、macOS は `~/Library/Application Support/meta-gesture/config.toml` です。
設定画面で編集するもので、手で書くことは想定していませんが、読めない内容は理由とともに知らせます。

```toml
os = "windows"

[[common]]
name = "ウィンドウを閉じる"
gesture = "↓→"
keys = "Ctrl+W"

[[sets]]
name = "ブラウザ"
apps = [
  { id = "chrome.exe", name = "Google Chrome" },
  { id = "firefox.exe", name = "Firefox" },
]

[[sets.commands]]
name = "戻る"
gesture = "←"
keys = "Alt+Left"
apps = { "firefox.exe" = "Alt+Left" }
```

- `os` は `windows` か `macos`。違う OS のファイルは読み込みもインポートも拒否します。アプリの見分け方とキーの意味が
  OS ごとに違うためです
- アプリの `id` は Windows では実行ファイル名(小文字)、macOS ではバンドル ID です。大文字と小文字は区別しません
- `common` はすべてのアプリに効くコマンドです。アプリのセットに同じジェスチャーのコマンドがあれば、そちらが優先されます
- コマンドの `keys` はセットの既定、`apps` はアプリごとのキーです。アプリごとのキーがなければ既定を送ります
- `gesture` は `←↑→↓` の並び(`LURD` でも読めます)で、同じ方向を続けず 6 つまでです
- キーの修飾キーは Windows では `Win`・`Ctrl`・`Alt`・`Shift`、macOS では `Ctrl`・`Option`・`Shift`・`Cmd` と書きます
- 1 つのアプリが入れるセットは 1 つまでです。セット名と、セット内のジェスチャーは重複できません

## 実装メモ

### ジェスチャーの判定

右ボタンを押した位置から 10pt 動くと、クリックではなくジェスチャーとして扱います。そこからポインタが一方向に 24pt
動くたびに、縦横どちらに多く動いたかで方向を 1 つ決め、直前と違う方向なら並びに足します。7 つ目の方向が出たら
ジェスチャーとして扱いません。Windows のフックの座標は物理ピクセルなので、ポインタのあるモニターの DPI で pt に
直します。

### 右ボタンの横取り

右ボタンが下りた時点で、ポインタの下のウィンドウのアプリにジェスチャーがあるかを調べ、あるときだけそのボタンを
アプリに渡さずに追います。ボタンが上がったときにジェスチャーでなければ、右クリック(押して離す)を送り直します。
送り直したイベントには目印(Windows は `dwExtraInfo`、macOS は `eventSourceUserData` に `0x4D475354`)を付け、
自分では追いません。ボタンを押しているあいだのポインタの移動は、Windows ではそのままアプリに届き、macOS では
ドラッグのイベントを止めます。

Windows の低レベルマウスフックは、応答が遅いと Windows に外されます。フックではポインタを追って結果を残すだけにし、
判定の続きとキーの送信はメインウィンドウへ `WM_APP_GESTURE` を投げてから行います。

### キーの送信

Windows はジェスチャーを始めた位置のトップレベルウィンドウを前面に出してから `SendInput` で送ります。
`SetForegroundWindow` が拒まれたときは、前面のウィンドウのスレッドに入力を一時的につないでやり直します。それでも
前面に出せなければ送りません。矢印や Home などは拡張キーとして送ります。

macOS はポインタの下のウィンドウを `CGWindowListCopyWindowInfo` で探し(権限は要りません)、そのアプリを
`activate` してから HID のイベントタップへキーを送ります。0.3 秒待っても前面に来なければ、そのプロセスへ直接送ります。

### 設定画面

Windows の設定画面は winwin と同じく `meta-gesture.exe --settings` の別プロセスで動かします。WinUI は最後のウィンドウが
閉じるとメッセージループを終え、同じプロセスでは再開できないためです。常駐部分は設定画面のプロセスが終わったら
設定ファイルを読み直します。

## テスト

`cargo test` で走るのは、OS に依存しないモジュールの単体テストです。`ffi.rs` のテストは `cargo test --features ffi` で
走ります。

| モジュール | 確かめていること |
|---|---|
| `gesture.rs` | 矢印と `LURD` の表記の読み書き、空・同じ方向の連続・7 つ以上の拒否、ほとんど動かない押下をクリックとすること、4 方向の読み取り、ぶれた線を 1 方向とすること、短すぎる移動では方向を足さないこと、DPI に合わせた距離、多すぎる方向をジェスチャーとしないこと |
| `config.rs` | 手書きの設定の読み込みと読み書きの往復、OS ごとの修飾キーの書き方、別の OS のファイルや OS のないファイルの拒否、読めない値の場所と理由、保存前の検証(名前のないセット・コマンド、同名のセット、2 つのセットに入ったアプリ、ジェスチャーのないコマンド、同じジェスチャー)、セットのコマンドが共通セットより優先されること、アプリごとのキーとセットの既定、ジェスチャーのないアプリでは右ボタンを追わないこと、ファイルの作成と保存 |
| `keys.rs` | キーの組み合わせの読み書き(修飾キーなしを含む)、Windows と macOS の書き方の往復、組み合わせにならない表記の拒否、OS ごとのキーキャップの並び、押されたキーの記録(修飾キーでやり直すこと) |
| `vk.rs` | すべての仮想キーコードがキーか修飾キーになり、表記からも読み戻せること、送る修飾キーのコード、拡張キーの判定 |
| `mac.rs` | macOS のキーコードがそれぞれ 1 つのキーになること、位置で決まるキーの名前、Command・Option の扱い、送るキーのコードとフラグが聞いたキーに戻ること |
| `editor.rs` | セット一覧で共通セットが先頭に来ること、追加したセット・コマンドの名前、セットを変えるとコマンドの選択が外れること、アプリを 1 つのセットにしか入れないこと、アプリを外すとそのキーも消えること、キーの行(セットの既定とアプリごと)、ジェスチャーの編集と重複の指摘、並べ替えと削除、保存できないときに問題の場所を選ぶこと、インポート |
| `ffi.rs` | 押下からジェスチャーの結果(送るキー、未設定、未登録、クリック)を返すこと、ジェスチャーのないアプリを追わないこと、設定画面の編集・キーの記録・保存が常駐部分に届くこと、別の OS のファイルのインポートの拒否、アイコンの RGBA |
| `art.rs` / `icon_res.rs` | アイコンの角の透明・線の色・macOS の余白・メニューバー用の黒一色、リソースファイルの各アイコンとグループの対応 |

届かない範囲もあります。`src/win/` と `macos/` は CI でビルドと clippy を通すだけで、動作は手で確かめます。

- Windows: 低レベルマウスフックで右ボタンを横取りし、ジェスチャーのないアプリでは横取りしないこと
- Windows: クリックの送り直しで、エクスプローラーやブラウザのコンテキストメニューが開くこと
- Windows: 背面のウィンドウの上で描いたジェスチャーで、そのウィンドウが前面に来てキーが届くこと
- Windows: 矢印・Home などの拡張キーと修飾キーの組み合わせが、狙いどおりに届くこと
- Windows: 実行ファイルの説明(`FileDescription`)からアプリ名を取ること、起動中のアプリの一覧
- Windows: コマンド名の表示が DPI の違うモニターで崩れず、フォーカスを奪わないこと
- Windows: 設定画面(WinUI 3)の表示・入力・保存、キーを記録するダイアログ、エクスポート・インポート・実行ファイルの選択ダイアログ、閉じるボタンでの確認
- Windows: 通知領域アイコンとメニュー、自動起動のレジストリ、二重起動で設定画面が開くこと、インストーラとアンインストーラ
- macOS: 初回起動でアクセシビリティの許可を求め、許可すると起動し直さずにジェスチャーが使えること、自己署名のビルドを入れ替えても許可が残ること
- macOS: イベントタップでの右ボタンの横取りとクリックの送り直し、背面のアプリの前面化とキーの送信
- macOS: コマンド名の表示、設定画面(SwiftUI)の表示・入力・保存、キーを記録するシート(Command を含む組み合わせ)、ログイン時の起動

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や
挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `Cargo.toml` の `version` です。書き換えたら `cargo update -p meta-gesture` で `Cargo.lock` も
追随させてください。

`main` ブランチで `Cargo.toml` の `version` が上がると、GitHub Actions が `meta-gesture-v<version>` タグの Release を作り、
インストーラ `meta-gesture_<version>_x64-setup.exe` と `meta-gesture_<version>_macos-arm64.dmg` を添付します。

上げ忘れても Release ジョブは何も知らせずにスキップするだけなので、`meta-gesture-version.yml` が Pull Request を赤くします。
リリースするつもりがない変更では、Pull Request に `no-release` ラベルを付けてください。

依存ライブラリの新しいバージョンは Dependabot が週に 1 回確かめ、見つかると Pull Request を作ります(設定は
`.github/dependabot.yml`)。この Pull Request には `no-release` ラベルが付き、ビルドとテストがすべて通ると
`.github/workflows/dependabot-merge.yml` がマージします。手で確かめる項目は次にこのアプリを変更する Pull Request で
まとめて確認します。
