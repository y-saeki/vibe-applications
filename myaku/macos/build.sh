#!/bin/sh
# Builds myaku.app here: the Rust library as a static library, the Swift
# bindings to it, the Swift app linked against both, and the bundle around
# it. Runs on an Apple silicon Mac with Rust and Xcode's command line tools.
set -eu
cd "$(dirname "$0")"

manifest=../Cargo.toml
target=../target/release
cargo rustc --manifest-path "$manifest" --locked --release --lib --features ffi --crate-type staticlib

# The bindings are written from the library that was just built, so they
# always match it.
bindgen() {
    cargo run --manifest-path "$manifest" --locked --features ffi-bindgen --bin uniffi-bindgen-swift -- "$@"
}
bindgen --headers "$target/libmyaku.a" Sources/myakuFFI
bindgen --swift-sources "$target/libmyaku.a" Sources/Myaku/Generated

swift build -c release -Xlinker -L"$target"

version="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/[" ]/,"");sub(/^version=/,"");print;exit}' "$manifest")"
app=myaku.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp .build/release/Myaku "$app/Contents/MacOS/myaku"
sed "s/@VERSION@/$version/g" Info.plist > "$app/Contents/Info.plist"
# The icon is drawn by the app, from the same pixels as on Windows.
iconset="$(mktemp -d)/myaku.iconset"
"$app/Contents/MacOS/myaku" --iconset "$iconset"
iconutil -c icns -o "$app/Contents/Resources/myaku.icns" "$iconset"
rm -rf "$(dirname "$iconset")"
# macOS 26 and later show the Liquid Glass icon in Assets.car instead, compiled
# from AppIcon.icon by actool and committed (see DEVELOPMENT.md).
cp Assets.car "$app/Contents/Resources/Assets.car"
# Ad hoc: myaku asks for no permission that a stable signature would keep.
codesign --force --sign - "$app"
echo "built $(pwd)/$app"
