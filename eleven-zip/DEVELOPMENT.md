# eleven-zip の開発

eleven-zip をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

Rust で書いた単一の exe です。設定画面は [`windows-reactor`](https://crates.io/crates/windows-reactor)(Rust から WinUI 3 を
使う Microsoft のクレート)で、それ以外は [`windows`](https://crates.io/crates/windows) クレート(Win32 API のバインディング)で
組んでいます。WinUI 3 を動かす Windows App Runtime は exe と同じフォルダに置いて配布します(self-contained)。
配布対象は Windows (x64) だけで、GitHub Actions の `windows-latest` でビルドし、[NSIS](https://nsis.sourceforge.io/) で
インストーラを作ります。

7-Zip は同梱せず、実行時に eleven-zip が GitHub の [ip7z/7zip](https://github.com/ip7z/7zip)(7-Zip の作者による公開先)
から取得して、ユーザーごとのフォルダに置きます。

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)(stable、1.95 以降) | `rustup` でインストール。`windows-reactor` が 1.95 以降を求めます |
| Windows: Visual Studio Build Tools | 「C++ によるデスクトップ開発」を選択。Rust は MSVC ツールチェーンを既定にする |
| [NSIS](https://nsis.sourceforge.io/) 3 | インストーラを作るときだけ。`makensis` が使えればよく、Linux のパッケージでも構いません |
| [PowerShell](https://learn.microsoft.com/powershell/) 7(`pwsh`) | インストーラを作るときだけ。同梱するファイルの一覧を書き出します |

Windows でのビルドは、初回に Windows App Runtime のパッケージを nuget.org からダウンロードします(`build.rs` が呼ぶ
`windows-reactor-setup`。`%LOCALAPPDATA%\windows-reactor-setup` にキャッシュされます)。

Windows 以外でも、Win32 に触れない部分(コマンドライン・設定ファイル・展開先の決定・7-Zip のリリースと版の管理・
レジストリに書く内容)はそのままビルド・テストできます。Win32 側の型検査は `x86_64-pc-windows-msvc` ターゲットで
行えます(リンクはできません)。

```sh
rustup target add x86_64-pc-windows-msvc
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

## ビルドと実行

```sh
cd eleven-zip
cargo run                               # 設定画面(デバッグビルドはコンソールも開きます)
cargo run -- --extract C:\path\to\a.zip # 展開
cargo run -- --register                 # エクスプローラーへの登録(--unregister で解除)
cargo build --release                   # 配布用ビルド: target/release/eleven-zip.exe と、横に置かれる Windows App Runtime
```

`--register` はレジストリに実行した exe のパスを書きます。`cargo run` で登録すると、右クリックメニューが
`target/debug/eleven-zip.exe` を指すので、試したあとは `--unregister` で消すか、インストールし直してください。

インストーラ(`installer/installer.nsi`)は、リリースビルドのあとに同梱するファイルの一覧(`payload.nsh`)を書き出してから
作ります。一覧は exe と、`target/release` に置かれた Windows App Runtime のファイルです。

```sh
cd eleven-zip/installer
pwsh payload.ps1 -Dir ../target/release -Out payload.nsh
makensis /INPUTCHARSET UTF8 /DVERSION=0.1.0 /DPAYLOAD=payload.nsh /DOUTFILE=../target/release/eleven-zip_0.1.0_x64-setup.exe installer.nsi
```

デバッグビルドはコンソールサブシステムで、リリースビルドだけがコンソールを持たない GUI サブシステムです
(`src/main.rs` の `windows_subsystem`)。

## 検証コマンド

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI(`.github/workflows/eleven-zip.yml`)は `windows-latest` でこの 3 つとリリースビルド、インストーラの作成を実行します。

## Pull Request のビルド

`eleven-zip/` を変更する Pull Request では、GitHub Actions が Windows 版をビルドし、成功するとインストーラへの
ダウンロードリンクを Pull Request にコメントします。push のたびに新しいコメントを投稿し、前回までのコメントは outdated として畳みます。
リンク先のダウンロードには GitHub へのログインが必要で、成果物には保持期限があります(期限はコメントに書かれます)。

## リリース

`main` に push されたとき、`Cargo.toml` の `version` に対応するタグ `eleven-zip-v<version>` の Release がまだなければ、
CI が作ったインストーラを添えて作成します。バージョンを上げずにマージしても Release は作られません。

Pull Request でビルドしたインストーラは、バージョンの後ろに Pull Request の先頭コミットの短いハッシュを semver の
ビルドメタデータとして付けます(`0.1.2+047ba8f`)。インストーラのファイル名、「設定」→「アプリ」に出るバージョン、
設定画面のタイトルがこの形になるので、手元に入っているのがどの Pull Request のどのコミットか見分けられます。
ハッシュは `cargo build` に環境変数 `ELEVEN_ZIP_COMMIT` で、`makensis` に `/DBUILD=<commit>` で渡します。数字しか
受け付けない `VIProductVersion` には付けません。`main` のビルドは Release に載るものなので、`0.1.2` のままです。

Pull Request では `.github/workflows/eleven-zip-version.yml` が、`eleven-zip/` を変更しているのにバージョンが base ブランチと
同じままなら失敗します。リリースしない変更には `no-release` ラベルを付けてください。バージョンを上げたら
`cargo update -p eleven-zip` で `Cargo.lock` も合わせます。

## 構成

```
eleven-zip/
├── build.rs              # Windows App Runtime の配置、アプリケーションマニフェストと exe のアイコンの埋め込み
├── eleven-zip.manifest   # DPI 対応などの宣言。build.rs が Windows App Runtime のマニフェストと合わせて埋め込む
├── icon.svg              # アイコンの原画。build.rs が exe のアイコンに描く
├── installer/
│   ├── installer.nsi     # Windows インストーラ(NSIS)
│   └── payload.ps1       # インストーラに入れるファイルの一覧(payload.nsh)を書き出す
└── src/
    ├── main.rs
    ├── cli.rs            # コマンドライン(--extract / --register / --unregister、引数なしは設定画面)
    ├── config.rs         # 設定ファイルの型・読み書き・初期値
    ├── archive.rs        # 展開先の決め方、フォルダ名、7z.exe の一覧の読み方、終了コードの意味
    ├── release.rs        # GitHub の最新リリースの読み方、MSI のファイル名、SHA-256 の照合、更新確認の間隔
    ├── store.rs          # 7-Zip を置くフォルダ(版ごとのフォルダ、使う版の切り替え、古い版の削除、更新のロック)
    ├── shell.rs          # レジストリに書く内容と消す内容
    ├── art.rs            # icon.svg を指定の大きさのピクセルに描く([resvg](https://crates.io/crates/resvg))。build.rs が使う
    ├── icon_res.rs       # exe に埋め込むアイコンのリソース(.res)を組み立てる。build.rs が使う
    ├── title.rs          # 設定画面のタイトル(バージョン入り)と、開いている設定画面の見分け方
    └── win/              # Win32・ネットワーク・7-Zip に触れる部分。Windows でだけビルドされる
        ├── extract.rs    # 展開: 7z.exe で一覧を取り、7zG.exe で展開し、エクスプローラーで開く
        ├── sevenzip.rs   # 7-Zip の取得と更新: 最新版の確認、MSI のダウンロード、msiexec での取り出し
        ├── net.rs        # WinHTTP での HTTPS の GET
        ├── registry.rs   # shell.rs の内容を HKEY_CURRENT_USER に書く・消す
        ├── settings_ui.rs # 設定画面(WinUI 3)
        └── icon.rs       # 設定画面のアイコン(exe のアイコンを読み込む)
```

`win/` 以外の 9 つは Win32 に依存しないので、どのプラットフォームでもテストできます。挙動を決めるロジックはなるべく
こちらに置き、`win/` は外部とのやり取りに留めています。

## 設定ファイル

`%APPDATA%\eleven-zip\config.toml` です。設定画面でスイッチを切り替えるたびに書き込まれ、それまではファイルがなくても
初期値で動きます。書かれていない項目は初期値になります。

```toml
open_in_explorer = true      # 展開が終わったら展開先をエクスプローラーで開く
unwrap_single_item = true    # 中身が 1 つのフォルダー(またはファイル)だけなら、書庫の横にそのまま展開する
context_menu = true          # 右クリックメニューに「eleven-zip で展開」を出す
auto_update = true           # 1 日 1 回、新しい 7-Zip を確認して入れる
```

## 実装メモ

### 起動のされ方

eleven-zip は常駐しません。exe は起動のされ方で 4 つの役を持ちます(`cli.rs`)。

| コマンドライン | 誰が起動するか | すること |
|---|---|---|
| なし | スタートメニュー、インストーラの完了画面 | 設定画面を開く。すでに開いていれば前に出す |
| `--extract <書庫>` | エクスプローラー(右クリックメニュー・ダブルクリック) | 展開する。オプションなしで渡されたファイル(exe へのドラッグ&ドロップ)も同じ |
| `--register` | インストーラ | エクスプローラーへの登録 |
| `--unregister` | アンインストーラ | 登録の解除 |

エクスプローラーは選ばれた書庫 1 つにつき 1 つのプロセスを起動します。

### 展開

1. 7-Zip がまだなければ取得します(`sevenzip::ensure`)。別の eleven-zip が取得中なら、最大 2 分待ちます
2. `7z.exe l -ba -slt` で書庫の中のパスを一覧します。ファイル名まで暗号化された書庫でパスワードの入力待ちにならないよう、
   ダミーのパスワード(`-p`)を渡します。その場合は一覧に失敗し、書庫名のフォルダへの展開になります
3. `archive::plan` が展開先を決めます。すべてのパスが 1 つの名前の下にあり(1 つのフォルダーか 1 つのファイル)、
   書庫の横にその名前のものがなければ、書庫のあるフォルダへそのまま展開します。それ以外は、書庫名から拡張子
   (`.tar.gz`、`.7z.001`、`.part1.rar` のような 2 段のものを含む)を除いた名前のフォルダを作ります。
   同名があれば ` (2)`、` (3)` …を付けます
4. `7zG.exe x -o<展開先>` で展開します。進み具合・パスワード・上書きの確認・エラーは 7-Zip 自身のウィンドウが出します
5. 終了コード 0(成功)か 1(警告)なら、展開先をエクスプローラーで開きます。フォルダならそのフォルダを、
   ファイルならそれを選んだ状態で親フォルダを開きます(`SHOpenFolderAndSelectItems`)
6. 自動更新が有効で、前回の確認から 24 時間たっていれば、ここで 7-Zip の更新を確認します。展開のウィンドウは
   もう閉じているので、何も表示しません

### 7-Zip の取得と更新

`https://api.github.com/repos/ip7z/7zip/releases/latest`(下書きとプレリリースを除いた最新のリリース)を読み、
タグ(`26.04`)から版を、アセットから 64 ビット版の MSI(`7z2604-x64.msi`)を探します。ダウンロード URL が
`https://github.com/ip7z/7zip/releases/download/` で始まらなければ拒みます。GitHub がアセットに付けている
`digest`(`sha256:...`)があれば、ダウンロードしたファイルと照合します。

MSI は `msiexec /a <msi> /qn TARGETDIR=<dir>`(管理者用インストール)で中のファイルを取り出すだけで、インストールは
しません。管理者権限も要りません。ファイルは `<dir>\Files\7-Zip` に出てきます。

置き場所は `%LOCALAPPDATA%\eleven-zip\7-Zip` です(`store.rs`)。

```
7-Zip\
├── 26.04\        # 版ごとのフォルダ(7z.exe、7zG.exe、7z.dll など)
├── current       # 使う版("26.04")
├── last-check    # 最後に最新版を確認した時刻(Unix 秒)
└── update.lock   # 更新中だけある。10 分より古いものは中断された更新の残りとして無視する
```

新しい版はまず `staging-<版>` に取り出し、版のフォルダへ移してから `current` を書き換えます(一時ファイルに書いて
rename)。古い版のフォルダは、そのあと消せるものだけ消します。古い版で展開中の 7-Zip がいればそのフォルダは消えず、
次の更新の確認のときに消えます。

通信は WinHTTP で、Windows のプロキシ設定(`WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY`)と証明書ストアを使います。
GitHub の API は認証なしで 1 時間に 60 回までですが、確認は 1 日 1 回と、設定画面の「更新を確認」のときだけです。

### エクスプローラーへの登録

すべて HKEY_CURRENT_USER に書きます(`shell.rs`)。

| キー | 内容 |
|---|---|
| `Software\Classes\eleven-zip.archive` | ProgID。`shell\open\command` が `"<exe>" --extract "%1"` |
| `Software\Classes\.<拡張子>\OpenWithProgids` | 値 `eleven-zip.archive`。「プログラムから開く」に eleven-zip が出る |
| `Software\Classes\SystemFileAssociations\.<拡張子>\shell\eleven-zip.extract` | 右クリックメニューの「eleven-zip で展開」。既定のアプリが何であっても出る |
| `Software\eleven-zip\Capabilities`、`Software\RegisteredApplications` の値 `eleven-zip` | Windows の「既定のアプリ」に eleven-zip のページを作る |
| `Software\Classes\Applications\eleven-zip.exe` | 「プログラムから開く」での表示名 |

既定のアプリ(`UserChoice`)は Windows がユーザー本人の操作でしか変えられないようにしているため、eleven-zip は設定しません。
設定画面の「Windows の設定を開く」は `ms-settings:defaultapps?registeredAppUser=eleven-zip` を開きます(Windows 11 で
eleven-zip のページが開きます)。

設定画面を開くたびに登録し直し、右クリックメニューのスイッチを切り替えたときも登録し直します。exe の場所が
変わっても登録が追従します。書いたあとは `SHChangeNotify(SHCNE_ASSOCCHANGED)` でエクスプローラーに知らせます。

対象の拡張子は `shell::EXTENSIONS` です。

### 設定画面

WinUI 3 の画面で、Windows 11 の設定アプリにならってカードを縦に並べています。スイッチは切り替えた時点で設定ファイルに
書き込み、保存ボタンはありません。WinUI のスイッチは値を設定し直しただけでも変更イベントを出すので、値が変わった
ときだけ書き込みます(`Settings::change`)。

開いたとき、7-Zip がまだないか、自動更新が有効で確認の時期なら、更新を確認します。確認と取得は
`spawn_background` で別スレッドで行い、終わると結果のメッセージが画面に届きます。

設定画面のタイトルにはバージョンが入ります(`eleven-zip の設定 - v0.1.2`。Pull Request のビルドは `v0.1.2+047ba8f`)。
すでに設定画面が開いていれば、クラス名(`WinUIDesktopWin32WindowClass`)のウィンドウからタイトルで設定画面を
見つけて前に出し、新しいウィンドウは開きません。タイトルは `eleven-zip の設定` だけか、`eleven-zip の設定 - v` で
始まるものを設定画面とみなします(`title.rs`)。バージョンの違う eleven-zip の画面も見つかります。

### Windows インストーラ(NSIS)

`installer/installer.nsi` は winwin のインストーラ(`winwin/installer/installer.nsi`)を元にしています。

| 項目 | 内容 |
|---|---|
| インストール先 | ユーザー単位(`%LOCALAPPDATA%\eleven-zip`)。管理者権限を求めません。前回のインストール先があればそちらを既定にします |
| 旧バージョンがあるとき | 確認せずに上書きします |
| 開いている設定画面 | 確認せずに閉じます(設定画面と同じくタイトルで探して `WM_CLOSE`)。前のバージョンの画面も閉じます。インストーラ・アンインストーラのどちらも |
| 同梱するもの | `eleven-zip.exe` と Windows App Runtime のファイル一式(`payload.ps1` が `target/release` から一覧を作る)。WebView2 用の `Microsoft.Web.WebView2.Core.dll` と、`ja-JP`・`en-US` 以外の言語のフォルダは入れません |
| インストール後 | `eleven-zip.exe --register` を実行します。完了画面の「eleven-zip の設定を開く」で設定画面が開き、7-Zip を取得します |
| スタートメニュー | `eleven-zip` のショートカットを置きます |
| アンインストール | 「設定」→「アプリ」に登録します。`eleven-zip.exe --unregister` を実行してからファイル(一覧にあるものだけ)とショートカットを消し、`%LOCALAPPDATA%\eleven-zip\7-Zip` も消します。`%APPDATA%\eleven-zip` の設定は、消すかどうかを聞きます(既定は残す。サイレント実行では残します) |

スクリプトは BOM 付きの UTF-8 です。BOM がないと、Windows の `makensis` はシステムの文字コードで読むため、
日本語が文字化けします。CI はさらに `/INPUTCHARSET UTF8` を渡しています。

### アイコン

アイコンの原画は `icon.svg` です。`build.rs` が `art.rs`(resvg)で 16〜256px の 8 サイズに描き、`icon_res.rs` で
リソースファイル(.res)にしてリンカに渡します。`rc.exe` もアイコンファイル(.ico)も要りません。resvg はビルドとテストで
だけ使い、exe には入りません。

エクスプローラー、タスクバー、右クリックメニュー、関連付けたファイルは exe のこのアイコンを使います。設定画面の
タイトルバーも、実行時に `win/icon.rs` が exe のアイコンを Windows の求める大きさで読み込み(`LoadImageW`)、
`WM_SETICON` で付けます。

## テスト

`cargo test` で走るのは、Win32 に依存しない 9 つのモジュールの単体テストです(Windows 以外でも走ります)。

| モジュール | 確かめていること |
|---|---|
| `cli.rs` | 引数なしで設定画面、`--extract`・`--register`・`--unregister` の読み分け、オプションなしのファイルを展開すること |
| `config.rs` | 初期値、ファイルがなければ初期値で読み、ファイルを作らないこと、読み書きの往復、書かれていない項目の初期値、壊れたファイルのエラー |
| `archive.rs` | 書庫名からのフォルダ名(2 段の拡張子、分割書庫、末尾のドットと空白)、1 つのフォルダー・ファイルを書庫の横に展開すること、フォルダーの項目がない一覧、複数の項目・設定がオフ・同名がある・一覧に失敗・空の書庫でフォルダを作ること、番号の付け方、`..` などを展開先にしないこと、一覧の読み方、終了コードの意味 |
| `release.rs` | 版の読み書きと比較、MSI のファイル名、最新リリースの読み方、digest がないリリース、MSI がないリリースと配布元以外の URL の拒否、API のエラー応答、SHA-256 の照合、確認の間隔、URL の分解 |
| `store.rs` | 何もない状態、版を置いて使う版にすること、7-Zip の入っていないパッケージを拒むこと、ファイルが消えた版を使わないこと、古い版と残骸の削除、更新のロック |
| `shell.rs` | 開くと展開するコマンド、関連付けと既定のアプリのための値、右クリックメニュー、オフにしたときに右クリックメニューだけを消すこと、解除で登録したものがすべて消えること、共有のキーを丸ごと消さないこと、拡張子の一覧 |
| `art.rs` | `icon.svg` が描けること、角が透明で上が緑・下がオレンジ・中央がファスナーの白であること、小さいサイズでも中央が不透明なこと |
| `icon_res.rs` | リソースファイルの各アイコンとグループの対応、行が下から並ぶこと |
| `title.rs` | タイトルにバージョンと Pull Request のコミットが入ること、どのバージョンの設定画面も見分けること、ほかのタイトルを設定画面とみなさないこと |

届かない範囲もあります。`win/` 以下は CI でビルドと clippy を通すだけで、動作は手で確かめます。

- GitHub の API と MSI のダウンロード(WinHTTP、プロキシ、タイムアウト)
- `msiexec /a` での取り出しと、取り出したファイルの配置(`Files\7-Zip`)
- 7z.exe での一覧と 7zG.exe での展開、パスワード付きの書庫、展開後にエクスプローラーで開くこと
- レジストリへの書き込みと、エクスプローラーの右クリックメニュー・「プログラムから開く」・既定のアプリの設定
- 設定画面(WinUI 3)の表示、スイッチ、更新の確認、二重起動の防止
- インストーラとアンインストーラ(CI は `makensis` が通ることまでを確かめます)

手で確かめるときは、少なくとも次を通してください: インストールの完了画面から設定画面を開くと 7-Zip が取得され、
バージョンが表示される / zip を右クリックして「eleven-zip で展開」で展開でき、展開先がエクスプローラーで開く /
1 つのフォルダーだけを含む zip が、フォルダーを二重にせず書庫の横に展開される。同じ zip をもう一度展開すると、
既存のものと重ならない名前のフォルダに展開される / 複数のファイルを含む 7z が書庫名のフォルダに展開される /
`.gz` 1 つのファイルが書庫の横に展開され、そのファイルが選ばれた状態でフォルダが開く / パスワード付きの zip と、
ファイル名まで暗号化した 7z で 7-Zip がパスワードを聞き、展開できる / 7-Zip のウィンドウで中止すると何も開かない /
「展開したらエクスプローラーで開く」をオフにすると開かない / 右クリックメニューをオフにすると消え、オンで戻る /
「プログラムから開く」で eleven-zip を選び「常に使う」にすると、ダブルクリックで展開される / 「Windows の設定を開く」で
既定のアプリの eleven-zip のページが開く / `current` を古い版に書き換え、`last-check` を消してから展開すると、展開の
あとで新しい版に入れ替わり、古い版のフォルダが消える / 設定画面を開いたまま eleven-zip をもう一度起動すると、
同じウィンドウが前に出る / 設定画面のタイトルにバージョンが出る / 0.1.1 の設定画面を開いたまま新しいインストーラを実行すると、その画面が閉じる / アンインストールで右クリックメニュー・関連付け・`%LOCALAPPDATA%\eleven-zip` が消える。
