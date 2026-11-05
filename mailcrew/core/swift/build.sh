#!/bin/sh
# Builds the Rust library (../) as a static library for Apple silicon and
# writes the Swift bindings to it, so that this package can build. Runs on a
# Mac with Rust and Xcode's command line tools. Then:
#
#   swift test -Xlinker -L"$(./build.sh --lib-dir)"
set -eu
cd "$(dirname "$0")"

manifest=../Cargo.toml
triple=aarch64-apple-darwin
lib_dir="$(cd .. && pwd)/target/$triple/release"
case "${1:-}" in
    --lib-dir) echo "$lib_dir"; exit 0 ;;
    "") ;;
    *) echo "usage: $0 [--lib-dir]" >&2; exit 2 ;;
esac

# The oldest macOS the package and the app support (Package.swift,
# ../../project.yml); without it the library is built for the Mac building it.
export MACOSX_DEPLOYMENT_TARGET=14.0

cargo rustc --manifest-path "$manifest" --locked --release --target "$triple" \
    --lib --features ffi --crate-type staticlib
lib="$lib_dir/libmailcrew_core.a"

# The bindings are written from the library that was just built, so they
# always match it.
bindgen() {
    cargo run --manifest-path "$manifest" --locked --features ffi-bindgen --bin uniffi-bindgen-swift -- "$@"
}
bindgen --headers "$lib" Sources/mailcrew_coreFFI
bindgen --swift-sources "$lib" Sources/MailCrewCore/Generated
echo "built $lib"
