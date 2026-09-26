# winwin の開発

winwin をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

Rust で書いた単一の exe です。常駐部分は [`windows`](https://crates.io/crates/windows) クレート(Win32 API の
バインディング)だけで組み、設定画面は [`windows-reactor`](https://crates.io/crates/windows-reactor)(Rust から WinUI 3 を
使う Microsoft のクレート)で組んでいます。WinUI 3 を動かす Windows App Runtime は exe と同じフォルダに置いて配布します
(self-contained)。配布対象は Windows (x64) だけで、GitHub Actions の `windows-latest` でビルドし、
[NSIS](https://nsis.sourceforge.io/) でインストーラを作ります。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)(stable、1.95 以降) | `rustup` でインストール。`windows-reactor` が 1.95 以降を求めます |
| Windows: Visual Studio Build Tools | 「C++ によるデスクトップ開発」を選択。Rust は MSVC ツールチェーンを既定にする |
| [NSIS](https://nsis.sourceforge.io/) 3 | インストーラを作るときだけ。`makensis` が使えればよく、Linux のパッケージでも構いません |
| [PowerShell](https://learn.microsoft.com/powershell/) 7(`pwsh`) | インストーラを作るときだけ。同梱するファイルの一覧を書き出します |

Windows でのビルドは、初回に Windows App Runtime のパッケージを nuget.org からダウンロードします(`build.rs` が呼ぶ
`windows-reactor-setup`。`%LOCALAPPDATA%\windows-reactor-setup` にキャッシュされます)。

Windows 以外でも、Win32 に触れない部分(設定ファイル・配置の計算・ショートカットの表記・設定画面の編集中データ)は
そのままビルド・テストできます。Win32 側の型検査は `x86_64-pc-windows-msvc` ターゲットで行えます(リンクはできません)。

```sh
rustup target add x86_64-pc-windows-msvc
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

## ビルドと実行

```sh
cd winwin
cargo run                   # デバッグビルドで起動(コンソールが開き、終了はトレイメニューから)
cargo run -- --settings     # 設定画面だけを起動
cargo build --release       # 配布用ビルド: target/release/winwin.exe と、横に置かれる Windows App Runtime
```

インストーラ(`installer/installer.nsi`)は、リリースビルドのあとに同梱するファイルの一覧(`payload.nsh`)を書き出してから
作ります。一覧は exe と、`target/release` に置かれた Windows App Runtime のファイルです。

```sh
cd winwin/installer
pwsh payload.ps1 -Dir ../target/release -Out payload.nsh
makensis /INPUTCHARSET UTF8 /DVERSION=0.6.0 /DPAYLOAD=payload.nsh /DOUTFILE=../target/release/winwin_0.6.0_x64-setup.exe installer.nsi
```

デバッグビルドはコンソールサブシステムで、リリースビルドだけがコンソールを持たない GUI サブシステムです
(`src/main.rs` の `windows_subsystem`)。

## 検証コマンド

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI(`.github/workflows/winwin.yml`)は `windows-latest` でこの 3 つとリリースビルド、インストーラの作成を実行します。

## Pull Request のビルド

`winwin/` を変更する Pull Request では、GitHub Actions が Windows 版をビルドし、成功するとインストーラへの
ダウンロードリンクを Pull Request にコメントします。push のたびに新しいコメントを投稿し、前回までのコメントは outdated として畳みます。
リンク先のダウンロードには GitHub へのログインが必要で、成果物には保持期限があります(期限はコメントに書かれます)。

## 構成

```
winwin/
├── build.rs            # Windows App Runtime の配置とアプリケーションマニフェストの埋め込み
├── winwin.manifest     # DPI 対応などの宣言。build.rs が Windows App Runtime のマニフェストと合わせて埋め込む
├── installer/
│   ├── installer.nsi   # Windows インストーラ(NSIS)
│   └── payload.ps1     # インストーラに入れるファイルの一覧(payload.nsh)を書き出す
└── src/
    ├── main.rs
    ├── config.rs       # 設定ファイルの型・読み書き・初期値
    ├── layout.rs       # 配置(基準位置・幅・高さ)から矩形を求める計算と、プレビュー用に縮めた画面
    ├── hotkey.rs       # ショートカットの表記("Ctrl+Alt+Left")と RegisterHotKey の値の相互変換
    ├── cycle.rs        # 同じショートカットを持つ項目のまとめ方と、押すたびに進む順番
    ├── draft.rs        # 設定画面で編集中の一覧(Editor)と 1 行(Draft)。保存時にここで検証する
    └── win/            # Win32 に触れる部分。Windows でだけビルドされる
        ├── app.rs      # 常駐部分: 隠しウィンドウ、メッセージループ、ホットキー、通知領域アイコン
        ├── mover.rs    # 前面ウィンドウの移動
        ├── settings.rs # 設定画面のプロセスの起動・前面化・終了の検知(常駐側)
        ├── settings_ui.rs # 設定画面(WinUI 3。`--settings` で起動したプロセスで動く)
        ├── autostart.rs# サインイン時の自動起動(レジストリの Run キー)
        └── icon.rs     # 通知領域アイコンの描画
```

`win/` 以外の 5 つは Win32 に依存しないので、どのプラットフォームでもテストできます。挙動を決めるロジックはなるべく
こちらに置き、`win/` は Win32 との受け渡しに留めています。

## 設定ファイル

`%APPDATA%\winwin\config.toml` です。初回起動時に初期値で作られ、設定画面の「保存」で上書きされます。

```toml
theme = "dark"            # system(既定。書かない)/ light / dark。設定画面とトレイメニューの明暗

[[shortcut]]
keys = "Ctrl+Alt+K"
anchor = "bottom-right"   # top-left / top / top-right / left / center / right / bottom-left / bottom / bottom-right
width = "1/2"             # 作業領域に対する比率。"1/2" のような分数か "0.5" のような小数(0 より大きく 1 以下)
height = "1/2"
```

- `keys` は `Win` `Ctrl` `Alt` `Shift` と 1 つのキーを `+` でつなぎます。キー名は `hotkey.rs` の表にあるもの
  (`A`〜`Z`、`0`〜`9`、`F1`〜`F24`、`Left` などの名前)か、仮想キーコードの 16 進表記(`0xBA`)です
- `Ctrl` `Alt` `Win` のどれも含まないショートカットは受け付けません(`Shift` だけでは、そのキーを全アプリから奪うため)
- 同じ `keys` を複数の項目に使えます。修飾キーを押したまま続けて押すと、ファイルに書いた順に切り替わります
- 書き出すときは、分母 12 以下の分数で表せる値は分数(`2/3`)、それ以外は小数 4 桁まで(`0.0769`)にします
- 0.3 までの版は `"50%"` `"800px"` の形で書いていました。`%` は比率に読み替え、読み込んだ時点でファイルを比率で
  書き直します(`33.333%` は `1/3` になります)。`px` は比率に直せないので読み込みエラーになります

ファイルを直接編集した場合、次に設定画面を閉じたとき(保存・キャンセルどちらでも)か、winwin を起動し直したときに
読み込まれます。読み込みに失敗すると、ショートカットを 1 つも登録せずに起動し、エラーをメッセージボックスで伝えます。

## 実装メモ

### 常駐部分はメッセージを待つだけ

グローバルショートカットは `RegisterHotKey` で登録しています。キーボードフックは使わないので、キー入力のたびに
winwin が呼ばれることはなく、ショートカットが押されたときに `WM_HOTKEY` が届くだけです。常駐中のスレッドは
`GetMessageW` で眠っていて、ふだんはタイマーやポーリングはありません。`MOD_NOREPEAT` を付けているので、押しっぱなしでも
1 回しか届きません。

### 切り替え(サイクル)ショートカット

同じ組み合わせの項目は `cycle.rs` の `bindings` で 1 つにまとめ、`RegisterHotKey` は組み合わせごとに 1 回だけ呼びます
(ホットキー ID `n` が `bindings[n - 1]`)。何番目を使うかは `Cycle` が持ち、同じ ID が続けて届くと次へ進みます。

`WM_HOTKEY` には修飾キーを離したことが含まれないので、項目が 2 つ以上ある組み合わせが押されたときだけ 15ms 間隔の
タイマー(`CYCLE_TIMER`)を動かし、`GetAsyncKeyState` でその組み合わせの修飾キーを見ます。どれか 1 つでも離されて
いれば `Cycle` を戻してタイマーを止めるので、ポーリングが続くのは修飾キーを押している間だけです。キーボードフックは
使いません。設定画面を開いてショートカットを解除するときも、同じように戻します。

隠しウィンドウは message-only ウィンドウではなく、表示しないトップレベルウィンドウです。Explorer が再起動したときの
`TaskbarCreated` ブロードキャストはトップレベルウィンドウにしか届かず、通知領域アイコンを出し直せなくなるためです。

二重起動は名前付きミューテックスで防ぎます。2 つ目の winwin は 1 つ目の隠しウィンドウに `WM_APP_OPEN_SETTINGS` を
送って終了するので、起動中にもう一度起動すると設定画面が開きます。

### 見えている枠を合わせる

Windows 10 以降のウィンドウは、見えている枠の外側に透明なリサイズ用の縁を持っています。`GetWindowRect` の矩形を
そのまま合わせると隣のウィンドウとの間に隙間ができるので、`DWMWA_EXTENDED_FRAME_BOUNDS`(見えている枠)との差を
縁の幅として足してから `SetWindowPos` します。縁の幅は大きさによって変わることがあるため、動かしたあと枠が
狙いとずれていれば、もう一度だけ測り直して置き直します。

配置の計算(`layout.rs`)は左右・上下の辺を浮動小数のまま求めてから個別に丸めます。幅 1921px を 3 等分しても、
隣り合う配置の辺が同じ位置に揃います。

次のウィンドウは動かしません: 最小化中、表示されていない(トレイメニューを閉じた直後は winwin 自身の隠しウィンドウが
前面になる)、応答なし(`SetWindowPos` が winwin ごと止まるため)、デスクトップとタスクバー。

### DPI

`winwin.manifest` で Per-Monitor (v2) の DPI 対応と Common Controls v6(メッセージボックス用)を宣言しています。
`build.rs` は `windows-reactor-setup` が作る Windows App Runtime のマニフェスト(WinUI のクラスの登録)に、
リンカの `/MANIFESTINPUT` でこれを足して埋め込みます。座標はすべて物理ピクセルで、ピクセル指定の長さはウィンドウがある
モニターの DPI で拡大します。設定画面の拡大縮小は WinUI に任せています。

### 設定画面

設定画面は WinUI 3 で、`winwin.exe --settings` として別のプロセスで動きます(`settings_ui.rs`)。`windows-reactor` は
最後のウィンドウが閉じるとメッセージループを終え、同じプロセスで WinUI を立ち上げ直すことはできないためです。常駐側
(`settings.rs`)はプロセスを起動し、`RegisterWaitForSingleObject` で終了を待って `WM_APP_SETTINGS_CLOSED` を受け取ります。
WinUI の DLL は設定画面のプロセスでしか読み込まれないので、常駐中のメモリは増えません。

設定画面を開いている間はショートカットをすべて解除し、閉じると(保存したかどうかにかかわらず)設定ファイルを読み直して
登録し直します。開いている間にもう一度開こうとすると、そのプロセスのウィンドウを前面に出します。winwin を終了するとき
(インストーラが閉じるときも)は、開いている設定画面を保存せずに終わらせます。

ショートカットは「変更」で開くダイアログで、押したキーをそのまま記録します。`windows-reactor` 0.100 にはキー入力の
イベントがないため、ダイアログを開いている間だけ低レベルキーボードフック(`keyhook.rs`、`WH_KEYBOARD_LL`)を入れて
キーを受け取ります。フックは winwin のウィンドウが前面にあるときだけキーを取り、Windows や他のアプリには渡しません
(Win でスタートメニューが開いたり、Alt でメニューに移ったりしないように)。どのキーで何が記録されるかは `hotkey.rs` の
`Recorder` が決めます。`windows-reactor` にキー入力のイベントが入ったら、フックをやめてそちらに置き換える予定です。

ショートカットは Windows の設定画面や PowerToys と同じく、アクセントカラーのキーキャップで描きます(`hotkey::keycaps`)。
ボタンではなく、背景をアクセントカラーにした枠です(クリックしても何も起きません)。上に載せる文字の色には、テーマの
基本の背景色(`SolidBackgroundFillColorBase`。ダークでは暗く、ライトでは明るい)を使っています。矢印キーは文字では
なくシェブロンにし、文字と同じ色で描けるよう 2 本の線で描いています。

左下の「テーマ」で、設定画面とトレイメニューをライト・ダークに固定できます(既定は Windows の設定に合わせる)。設定画面
には選んだ時点で反映し、保存すると設定ファイルの `theme` に書きます。

ウィンドウの閉じるボタンも、未保存の変更があれば確認を出します。`windows-reactor` はウィンドウの HWND を渡さないので、
スレッドのウィンドウからクラス名 `WinUIDesktopWin32WindowClass` のものを探してサブクラス化し、`WM_CLOSE` をコンポーネントへの
メッセージ(キャンセルと同じ)に置き換えています。見つからなければ、閉じるボタンは確認なしで閉じます。

一覧の各行の図と右側のプレビューは、どちらも設定画面を開いたモニターの作業領域を `layout.rs` の `miniature` で縮め、
そこへ配置を当てはめて描きます(`Draft::picture`)。

編集中の値は保存を押すまで検証しません。幅の欄が一時的に `5` や空になるのは入力途中では普通のことなので、
入力のたびに止めずに `Draft` へ文字列のまま入れておき、保存時に `Editor::build_config` がまとめて検証します。
WinUI のコントロールは、値を設定し直しただけでも変更イベントを出すので、`Editor::edit` は値が変わったときだけ
「変更あり」にします。

### 状態の持ち方

常駐側のウィンドウプロシージャから触る状態は `thread_local!` の `RefCell` に置いています(`app.rs` の `with_app`)。このクロージャの中では、自分のウィンドウにメッセージを送る API(メッセージボックス、
コントロールへのテキスト設定など)を呼ばないでください。ウィンドウプロシージャが再入して同じ `RefCell` を借りようとし、
借用に失敗します(`try_borrow_mut` なので落ちはしませんが、その処理は黙って捨てられます)。値を取り出してから
クロージャの外で呼ぶのが決まりです。

### Windows インストーラ(NSIS)

`installer/installer.nsi` は winwin 用に書いた NSIS スクリプトです。winwin は Tauri を使っていないので Tauri の
テンプレートは使えませんが、振る舞いは draftpad のインストーラ(`draftpad/src-tauri/installer.nsi`)に揃えています。

| 項目 | 内容 |
|---|---|
| インストール先 | ユーザー単位(`%LOCALAPPDATA%\winwin`)。管理者権限を求めません。前回のインストール先があればそちらを既定にします |
| 旧バージョンがあるとき | 確認せずに上書きします(draftpad が既定で選ぶ「上書きする」と同じ) |
| 起動中の winwin | 確認ダイアログを出さずに終了させます。インストーラ・アンインストーラのどちらも |
| 同梱するもの | `winwin.exe` と Windows App Runtime のファイル一式(`payload.ps1` が `target/release` から一覧を作る)。`windows-reactor-setup` が一緒に置く WebView2 用の `Microsoft.Web.WebView2.Core.dll` は使わないので入れません。WinUI の言語ごとのリソースのフォルダ(`de-DE` など 90 余り)も、`ja-JP` と既定の `en-US` だけを入れます。`payload.ps1` は各ファイルの大きさを CI のログに出します |
| インストール後 | ログの画面で止まらず、完了画面まで自動で進みます |
| 完了画面 | 「winwin を起動する」と「デスクトップにショートカットを作成する」。後者は上書きインストールのときだけ外した状態で出します |
| スタートメニュー | `winwin` のショートカットを置きます |
| アンインストール | 「設定」→「アプリ」に登録します。インストールしたファイル(一覧にあるものだけ)・ショートカットに加えて、自動起動の `Run` の値も消します。`%APPDATA%\winwin` の設定は、消すかどうかを聞きます(既定は残す。サイレント実行では残します) |

起動中の winwin は、隠しウィンドウのクラス名 `winwin.main` を `FindWindow` で探し、`WM_CLOSE` を送って終わらせます。
トレイメニューの「終了」と同じ後始末(通知領域アイコンの削除など)を通るためです。`src/win/app.rs` のクラス名を
変えるときは、スクリプトの `MAIN_CLASS` も合わせてください。

スクリプトは BOM 付きの UTF-8 です。BOM がないと、Windows の `makensis` はシステムの文字コード(英語版の
ランナーでは CP1252)で読むため、スクリプトに直接書いた日本語(完了画面のチェックボックスやメッセージ)が
文字化けします。CI はさらに `/INPUTCHARSET UTF8` を渡しています。

スタブは NSIS 既定の 32 ビット(x86)版です。64 ビットでない Windows では `.onInit` で止めます。
Wine で試すときに 32 ビットの Wine がなければ、`makensis "-XTarget amd64-unicode" ...` で 64 ビット版を作れば動きます
(Linux の NSIS パッケージには amd64 のスタブが入っています)。

### 通知領域アイコン

リソースファイルを持たず、実行時に `icon.rs` がピクセルを描いて作ります。exe 自体のアイコンは Windows 既定のものです。

右クリックメニューは Win32 の標準メニューです。Windows のダークモード(または設定の `theme`)に合わせるため、表示のたびに
uxtheme.dll の非公開の関数(序数 135 `SetPreferredAppMode`、136 `FlushMenuThemes`)を呼んでいます(`app.rs` の
`apply_menu_theme`)。
Windows の更新でなくなった場合は、明るい色のメニューに戻るだけです。

## テスト

`cargo test` で走るのは、Win32 に依存しない 5 つのモジュールの単体テストです(Windows 以外でも走ります)。

| モジュール | 確かめていること |
|---|---|
| `hotkey.rs` | 表記の読み書き、すべての仮想キーが書いた表記から読み戻せること、ショートカットにできない組み合わせの拒否、設定画面で押されたキーの記録のされ方、キーの表示 |
| `cycle.rs` | 同じショートカットの項目を設定の順にまとめること、続けて押すと進んで最初に戻ること、離すと最初からになること |
| `layout.rs` | 比率の読み書き(分数・小数、範囲外の拒否、以前の版の `%`、分数で書き出せる値の判定)、半分・4 分の 1・3 分の 1 の矩形、他のモニター座標、最小 1px、プレビュー用に縮めた画面の縦横比と寄せ方、縮めた画面への配置 |
| `config.rs` | 初期値の妥当性と読み書きの往復、表示のテーマ(選んだときだけ書くこと)、手書きファイルの読み込み、以前の版の `name`・`offset_x`・`offset_y` を読み捨てること、以前の版の `%` を読んでファイルを比率で書き直すこと、`px` や範囲外の値の拒否、同じショートカットの項目を順番どおりに読むこと、ファイルの作成と上書き |
| `draft.rs` | 設定画面の 1 行と設定の往復、保存時の検証メッセージ、一覧の追加・複製・削除・並べ替えと選択の移り方、値が変わらない編集を変更と数えないこと、一覧の表示文字列、配置の図の位置 |

届かない範囲もあります。`win/` 以下は CI でビルドと clippy を通すだけで、動作は手で確かめます。

- ショートカットの登録と、押したときに `WM_HOTKEY` が届くこと
- 修飾キーを離したことの検出(`GetAsyncKeyState` とタイマー)
- 前面ウィンドウが実際に狙った位置へ動くこと(見えない縁の補正、最大化の解除、管理者権限のウィンドウ)
- 設定画面(WinUI 3)の表示・入力・保存、閉じるボタンでの確認、DPI の違うモニターへの移動、一覧の各行に描く配置の図
- ショートカットを記録するダイアログのキーボードフック
- 設定画面のプロセスの起動・前面化・終了の検知
- 通知領域アイコン、メニュー、バルーン通知、Explorer 再起動後の再表示
- 自動起動のレジストリ
- インストーラとアンインストーラ(CI は `makensis` が通ることまでを確かめます)

手で確かめるときは、少なくとも次を通してください: 初回起動で設定ファイルができる / 既定のショートカットで左右半分・
四隅・中央に動く / 最大化中のウィンドウが解除されて動く / 同じショートカットを 3 つの配置に設定し、修飾キーを押したまま 4 回押すと 1→2→3→1 と切り替わり、修飾キーを離して押し直すと 1 つ目に戻る / 設定画面で追加・複製・削除・保存ができ、閉じたあと新しい
ショートカットで動く / 設定画面の一覧の各行に配置の図が出て、幅などを変えるとその行の図も変わる / 変更してから閉じるボタンを
押すと確認が出る / 「変更」のダイアログで `Win + Shift + ←` などを押すと、スタートメニューが開かずにそのまま記録される。
ダイアログを開いたまま他のアプリに切り替えると、そちらでは普通に入力できる / ダークモードでトレイの右クリックメニューが暗い色になる / 設定画面の「テーマ」をライト・ダークに変えると画面がすぐ切り替わり、
保存するとトレイメニューも同じ明暗になる / 設定画面を開いたままトレイアイコンをクリックすると、同じ設定画面が前面に出る / 起動中にもう一度起動すると設定画面が開く / トレイメニューの「終了」でアイコンが消えて終了する。
インストーラを変えたときは: 新規インストール(Windows App Runtime を入れていない環境で設定画面が開く)/ winwin の起動中に上書きインストール(winwin が終了し、完了画面の
デスクトップショートカットが外れている)/ アンインストールで、ファイル・スタートメニュー・「設定」→「アプリ」の項目・
自動起動の値が消える。

Windows がない環境では確かめられません。WinUI 3 は Wine では動かず、`windows-reactor-setup` は MSVC か
`gnullvm` のリンカを前提にしています。

## バージョニングとリリース

[セマンティック バージョニング](https://semver.org/lang/ja/)に従います。0.x の間は、ユーザーに見える機能の追加や
挙動の変更で minor を、修正だけなら patch を上げます。

バージョンの実体は `Cargo.toml` の `version` です。書き換えたら `cargo update -p winwin` で `Cargo.lock` も追随させてください。

`main` ブランチで `Cargo.toml` の `version` が上がると、GitHub Actions が `winwin-v<version>` タグの Release を作り、
インストーラ `winwin_<version>_x64-setup.exe` を添付します。インストーラは常に同じフォルダの `winwin.exe` を
上書きするので、自動起動のレジストリが指す exe のパスは更新後も変わりません。

### 上げ忘れを CI が止めます

上げ忘れても Release ジョブは黙ってスキップするだけなので、`winwin-version.yml` が Pull Request を赤くします。
`winwin/` を変更する Pull Request では、次のどれかに当たると落ちます。

- バージョンが base ブランチの先端と同じ、または古い
- `Cargo.toml` と `Cargo.lock` のバージョンが食い違っている

リリースするつもりがない変更(ドキュメントだけ、CI だけ、など)では、Pull Request に `no-release` ラベルを付けてください。

配布物はコード署名をしていないため、ダウンロードしたインストーラの実行時に SmartScreen の警告が出ます。
手順は README.md に書いてあります。
