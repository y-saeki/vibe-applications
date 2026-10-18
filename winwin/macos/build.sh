#!/bin/sh
# Builds winwin.app here: the Rust library as a static library, the Swift
# bindings to it, the Swift app linked against both, and the bundle around
# it, signed ad hoc. Runs on a Mac with Rust and Xcode's command line tools.
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
bindgen --headers "$target/libwinwin.a" Sources/winwinFFI
bindgen --swift-sources "$target/libwinwin.a" Sources/Winwin/Generated

swift build -c release -Xlinker -L"$target"

version="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/[" ]/,"");sub(/^version=/,"");print;exit}' "$manifest")"
app=winwin.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS"
cp .build/release/Winwin "$app/Contents/MacOS/winwin"
sed "s/@VERSION@/$version/g" Info.plist > "$app/Contents/Info.plist"
codesign --force --sign - "$app"
echo "built $(pwd)/$app"
