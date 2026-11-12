# myaku の開発

myaku をビルド・変更するための情報です。使い方やインストール方法は [README.md](./README.md) を参照してください。

[hmarr/vitals](https://github.com/hmarr/vitals)(macOS 専用、Swift、開発終了)の機能を、macOS と Windows の両方で動くように作り直したものです。
コードは引き継いでおらず、ゼロから書いています。

## 構成の方針

共通部分は Rust のライブラリで、その上に OS ごとのネイティブな UI を載せます。winwin と同じ形です。

| 部分 | 技術 | 選んだ理由 |
|---|---|---|
| 履歴・並べ替え・表示する値・OS の出力の解析・アイコンの絵 | Rust(`src/` のライブラリ) | 両 OS で同じ動きにするため。OS に触れないので Linux でもテストできる |
| macOS の UI | Swift(AppKit の `NSStatusItem` と `NSMenu`、中身は SwiftUI) | Vitals と同じく、メニューの中に一覧を描く。メニューの見た目・動きは OS 標準のまま |
| Windows の UI | Win32 と Direct2D・DirectWrite(`windows` クレート) | 通知領域から開くフライアウトを、Windows 11 の標準のもの(DWM の半透明の背景、角丸、アクセントカラー)に合わせて自前で描く |
| Rust と Swift の橋渡し | [UniFFI](https://github.com/mozilla/uniffi-rs) 0.32 | Swift のバインディングを `ffi.rs` から生成する |

次の 2 つは検討しましたが、採っていません。

- Tauri・Electron などの Web ベースの UI: 常駐アプリとしてはメモリが重く、メニューやフライアウトの見た目を OS に合わせるのも難しい
- Windows の UI に WinUI 3(`windows-reactor`): 見た目は最も標準に近いが、Windows App Runtime の同梱で配布物と常駐時のメモリが大きくなる。一覧は 1 画面だけで、Direct2D と DWM の背景で同等の見た目を作れる

### プロセスの読み方

1 秒ごとに、全プロセスの CPU 時間とメモリを読みます(`runner.rs` のスレッド)。CPU 使用率は前回との差を経過時間で割った値です。

- macOS: `/bin/ps -A -o pid= -o time= -o rss= -o comm=` を実行して出力を読みます(`mac_sample.rs`)。`ps` は set-user-ID root なので、root や他のユーザーのプロセスの CPU 時間も読めます。libproc の `proc_pid_rusage` は一般ユーザーには自分のプロセスの分しか返しません。メモリは常駐セットサイズ(RSS)です
- macOS のネットワーク: 「ネットワークの使用量を測る」がオンのときだけ、`/usr/bin/nettop -P -L 1 -J bytes_in,bytes_out` を実行して、プロセスごとの累計バイト数を読みます。前回との差を 1 秒の使用量にします。値が前回より減ることがあるため、減った分は 0 にします(Vitals と同じ扱いです)
- Windows: `NtQuerySystemInformation(SystemProcessInformation)` を 1 回呼び、全プロセスの一覧を読みます(`win/sampler.rs`、一覧の解析は `win_sample.rs`)。管理者権限なしで他のユーザーやサービスのプロセスも読めます。メモリはタスクマネージャーと同じプライベートワーキングセットです。アイドルプロセス(PID 0)は除きます
- Windows のネットワーク: 測りません。プロセスごとの通信量を得る手段は ETW や `GetPerTcpConnectionEStats` で、どちらも管理者権限が要ります

### 一覧の作り方

`monitor.rs` が、プロセスごとに直近 60 回分の値を持ちます(`stats.rs` のリングバッファー)。

- 並び順: 選んだ指標の 60 回分の合計が大きい順。上位 20 件を表示します
- 終了したプロセス: 値が欠けたまま残し、60 回分がすべて欠けたら消します。表示は斜体で薄くします
- 同じ PID で名前が変わった、または CPU 時間が減ったときは、PID が使い回された別のプロセスとして測り直します
- グラフの高さ(`format.rs`): CPU は 1 コアで 1 段、メモリは 512 MB で 1 段、ネットワークは 1 MB/秒で 1 段です。1 段を超えた分は濃い色で重ね、4 段で頭打ちにします(Vitals と同じ描き方です)。メモリもどの行も同じ尺度で比べられるよう固定の量にしています
- メモリの対数表示: メニューの「メモリを対数で表示」をオンにすると、メモリのグラフは 4 倍ごとに 1 段になります(128 MB で 1 段、512 MB で 2 段、2 GB で 3 段、8 GB で 4 段。32 MB 以下は描きません)。数 GB を使うプロセスが並ぶ環境でも、大きいプロセス同士を見分けられるようにするためです。見出しは「メモリ(対数)」になります。設定は macOS では `UserDefaults` の `logMemoryScale`、Windows では `HKEY_CURRENT_USER\Software\myaku` の `MemoryScale` に残します

## 開発に必要なもの

| ツール | 備考 |
|---|---|
| [Rust](https://rustup.rs/)(stable) | `rustup` でインストール |
| Windows: Visual Studio Build Tools | 「C++ によるデスクトップ開発」を選択。Rust は MSVC ツールチェーンを既定にする |
| macOS: Xcode のコマンドラインツール | Swift 5.9 以降 |
| [NSIS](https://nsis.sourceforge.io/) 3 | Windows のインストーラを作るときだけ |

Windows 以外でも、ライブラリ(`src/` のうち `main.rs` と `win/` 以外)はそのままビルド・テストできます。
Win32 側の型検査は `x86_64-pc-windows-msvc` ターゲットで行えます(リンクはできません)。

```sh
rustup target add x86_64-pc-windows-msvc
cargo clippy --target x86_64-pc-windows-msvc --all-targets -- -D warnings
```

## ビルドと実行

### Windows

```sh
cd myaku
cargo run                   # デバッグビルドで起動(コンソールが開き、終了は通知領域のメニューから)
cargo build --release       # 配布用ビルド: target/release/myaku.exe
```

インストーラはリリースビルドのあとに作ります。

```sh
cd myaku/installer
makensis /INPUTCHARSET UTF8 /DVERSION=0.1.0 installer.nsi
```

### macOS

```sh
myaku/macos/build.sh        # myaku/macos/myaku.app ができる
open myaku/macos/myaku.app
```

`build.sh` は、ライブラリを静的ライブラリ(`libmyaku.a`)としてビルドし、UniFFI で Swift のバインディングを書き出し、
Swift のアプリをリンクして `.app` にまとめます。バインディング(`macos/Sources/Myaku/Generated/`)と C のヘッダー
(`macos/Sources/myakuFFI/myakuFFI.h`)は生成物で、コミットしません。アプリのアイコンは、ビルドしたアプリ自身に
`--iconset` で書き出させ、`iconutil` で `.icns` にします。絵は Windows 版と同じ `art.rs` のものです。

macOS 26 以降では、`.icns` ではなく Liquid Glass のアイコンを使います。元は Icon Composer のデータ `macos/AppIcon.icon` で、背景の色と、前景の層(緑の脈の線)を分けて持っています。光沢や透け感は OS が付けるので、絵には描き込んでいません。`build.sh` は、これを `actool` で変換した `macos/Assets.car` をアプリに入れます。`Info.plist` の `CFBundleIconName` がこれを指し、macOS 26 より前の OS では `CFBundleIconFile` の `.icns` が使われます。`actool` は Xcode にしかないので、コマンドラインツールだけでもビルドできるよう、変換した結果をコミットしてあります。

層の SVG は `art.rs` の座標を 1024 倍して手で書き写したものです(角丸の枠と余白はありません)。図柄を変えるときは、`art.rs` と層の SVG の両方を直してください。座標と線の太さが `art.rs` と合っているかは、`art.rs` のテストが確かめます。`AppIcon.icon` を変えたら、Xcode 26 以降が入った Mac で次を実行し、できた `Assets.car` を差し替えてコミットします。`actool` には絶対パスを渡します。相対パスはシェルのカレントディレクトリでなく別の場所を基準に解決されることがあるためです。

```sh
cd myaku/macos
mkdir -p out && xcrun actool "$PWD/AppIcon.icon" --compile "$PWD/out" --output-format human-readable-text --errors \
  --output-partial-info-plist "$PWD/out/info.plist" --app-icon AppIcon --include-all-app-icons \
  --enable-on-demand-resources NO --development-region en --target-device mac \
  --minimum-deployment-target 26.0 --platform macosx
mv out/Assets.car Assets.car && rm -r out
```

署名はアドホックです。myaku は OS の許可(アクセシビリティなど)を求めないので、ビルドごとに署名が変わっても困りません。

## 検証コマンド

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features ffi-bindgen -- -D warnings
cargo test --features ffi
```

## テストの範囲

`cargo test` が確かめるのは、OS に触れない部分です。

| 対象 | ファイル |
|---|---|
| 60 回分のリングバッファーと合計 | `src/stats.rs` |
| CPU 使用率の計算、並び順、上位の件数、終了したプロセスの扱い、PID の使い回し、ネットワークの差分 | `src/monitor.rs` |
| 表示する値の書式と、グラフの高さ(メモリの線形と対数) | `src/format.rs` |
| 並べる基準の切り替え順と、ネットワークを切ったときの戻り先 | `src/resource.rs` |
| `ps` と `nettop` の出力の解析 | `src/mac_sample.rs` |
| `SystemProcessInformation` の一覧の解析 | `src/win_sample.rs` |
| アイコンの絵と、exe に埋め込むリソース | `src/art.rs`、`src/icon_res.rs` |
| macOS 版が呼ぶ入口 | `src/ffi.rs` |

`src/win/` と `macos/` は `cargo test` では確かめられません。変更したときは、次を手で確かめてください。

- アイコンをクリックすると一覧が開き、1 秒ごとにグラフが流れる
- 矢印で並べる基準が切り替わる。Windows では `←` `→` でも切り替わり、`Esc` で閉じる
- 一覧の外をクリックすると閉じる。Windows では、開いている一覧をアイコンのクリックで閉じたとき、開き直さない
- Windows: 一覧がアイコンの近くに、タスクバーと画面の端から離れて開く。タスクバーを上・左・右に置いたときと、高 DPI のモニターでも同じ
- Windows: ライト・ダークとアクセントカラーを変えると、一覧の色、通知領域のアイコンの色、右クリックメニューの色が追従する
- Windows 11 22H2 以降で背景が半透明になり、角が丸い。Windows 10 では単色の背景で崩れない
- Windows: Explorer を再起動しても通知領域にアイコンが戻る
- macOS: 「ネットワークの使用量を測る」をオンにすると、受信・送信が基準に加わる。オフにすると CPU に戻る
- 「ログイン時に開く」(Windows では「サインイン時に起動」)が効く
- 「メモリを対数で表示」をオンにすると、メモリの見出しが「メモリ(対数)」になり、グラフが対数の高さになる。オフで元に戻り、再起動しても選んだほうが残る
- インストーラで更新するとき、起動中の myaku が終了してから上書きされる

## Pull Request のビルド

`myaku/` を変更する Pull Request では、GitHub Actions(`.github/workflows/myaku.yml`)が Windows 版と macOS 版をビルドし、
Windows 版が成功するとインストーラへのダウンロードリンクを Pull Request にコメントします。macOS 版も成功していれば、
`.dmg`(Apple シリコン用)へのリンクも並べます。

## リリース

`Cargo.toml` の `version` を上げて `main` にマージすると、`myaku-v<version>` のタグで Release が作られ、
インストーラと `.dmg` が添付されます。バージョンを上げずにマージしても Release は作られません。
`.github/workflows/myaku-version.yml` が、`myaku/` を変更する Pull Request でバージョンが上がっているかを確かめます。
上げないときは `no-release` ラベルを付けます。

バージョンは次のように上げます。

- 機能の追加や、見た目・動作が変わる変更: マイナーバージョン
- 不具合の修正: パッチバージョン

macOS 版は Apple シリコン用だけをビルドし、Intel Mac 用と universal binary は作りません(リポジトリの `CLAUDE.md`)。

## 構成

```
myaku/
├── build.rs            # アプリケーションマニフェストと exe のアイコンの埋め込み(Windows)
├── myaku.manifest      # DPI 対応などの宣言
├── installer/
│   └── installer.nsi   # Windows インストーラ(NSIS)
├── macos/              # macOS 版
│   ├── build.sh        # ライブラリ・バインディング・アプリをビルドし、myaku.app にまとめる
│   ├── Info.plist      # アプリのバンドルの情報。build.sh がバージョンを埋める
│   ├── AppIcon.icon/   # macOS 26 以降の Liquid Glass のアイコン(Icon Composer のデータ)
│   ├── Assets.car      # AppIcon.icon を actool で変換したもの。build.sh がアプリに入れる
│   ├── Package.swift
│   └── Sources/
│       ├── myakuFFI/   # バインディングの C 側(build.sh が書くヘッダーと、静的ライブラリを指す module.modulemap)
│       └── Myaku/
│           ├── AppDelegate.swift  # メニューバーのアイコンとメニュー
│           ├── Model.swift        # 一覧の状態。メニューを開いている間だけ更新する
│           ├── ContentView.swift  # 一覧(SwiftUI)
│           ├── Sparkline.swift    # グラフ
│           ├── Autostart.swift    # ログイン時の起動(SMAppService)
│           └── Icon.swift         # build.sh が使う .iconset の書き出し
└── src/
    ├── main.rs         # Windows アプリの入口。win/ と合わせて bin になる
    ├── lib.rs          # win/ 以外のモジュールをまとめたライブラリ。OS に依存しない
    ├── stats.rs        # 60 回分のリングバッファー
    ├── sample.rs       # 1 回分の読み取り結果の型
    ├── monitor.rs      # プロセスごとの履歴と、一覧の行
    ├── format.rs       # 表示する値の書式と、グラフの高さ
    ├── resource.rs     # 並べる基準(CPU・メモリ・ネットワーク)
    ├── runner.rs       # 1 秒ごとに読むスレッド
    ├── mac_sample.rs   # ps と nettop の実行と、出力の解析
    ├── win_sample.rs   # SystemProcessInformation の一覧の解析
    ├── art.rs          # アイコンの絵(ピクセル)
    ├── icon_res.rs     # exe に埋め込むアイコンのリソース(.res)。build.rs が使う
    ├── ffi.rs          # macOS 版が呼ぶ入口(UniFFI)。feature `ffi` のときだけビルドされる
    ├── bin/uniffi-bindgen-swift.rs # ffi.rs から Swift のバインディングを書き出すツール(feature `ffi-bindgen`)
    └── win/            # Win32 に触れる部分。Windows でだけビルドされる
        ├── app.rs      # 常駐部分: 隠しウィンドウ、メッセージループ、通知領域のアイコンとメニュー
        ├── flyout.rs   # 一覧のウィンドウ(Direct2D・DirectWrite、DWM の背景)
        ├── sampler.rs  # NtQuerySystemInformation の呼び出し
        ├── theme.rs    # ライト・ダーク、アクセントカラー、Windows のビルド番号
        ├── settings.rs # 次の起動に残す設定(HKCU の Software\myaku)
        ├── icon.rs     # 通知領域のアイコン
        └── autostart.rs # サインイン時の起動(HKCU の Run キー)
```
