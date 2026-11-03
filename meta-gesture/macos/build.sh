#!/bin/sh
# Builds meta-gesture.app here: the Rust library as a static library, the
# Swift bindings to it, the Swift app linked against both, and the bundle
# around it. Runs on a Mac with Rust and Xcode's command line tools.
#
# META_GESTURE_SIGN_IDENTITY names the certificate to sign with (its SHA-1
# hash or its name, as codesign takes it). Without it the app is signed ad
# hoc, which macOS treats as a different app after every build, so the
# Accessibility permission has to be granted again.
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
bindgen --headers "$target/libmeta_gesture.a" Sources/meta_gestureFFI
bindgen --swift-sources "$target/libmeta_gesture.a" Sources/MetaGesture/Generated

swift build -c release -Xlinker -L"$target"

version="$(awk '/^\[package\]/{p=1;next} /^\[/{p=0} p && /^version *=/{gsub(/[" ]/,"");sub(/^version=/,"");print;exit}' "$manifest")"
app=meta-gesture.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp .build/release/MetaGesture "$app/Contents/MacOS/meta-gesture"
sed "s/@VERSION@/$version/g" Info.plist > "$app/Contents/Info.plist"
# The icon is drawn by the app, from the same pixels as on Windows.
iconset="$(mktemp -d)/meta-gesture.iconset"
"$app/Contents/MacOS/meta-gesture" --iconset "$iconset"
iconutil -c icns -o "$app/Contents/Resources/meta-gesture.icns" "$iconset"
rm -rf "$(dirname "$iconset")"
codesign --force --sign "${META_GESTURE_SIGN_IDENTITY:--}" "$app"
echo "built $(pwd)/$app"
