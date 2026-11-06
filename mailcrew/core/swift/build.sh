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

# Xcode runs this from the scheme's pre-action (../../project.yml) with a bare
# PATH, and rustup from Homebrew puts no cargo on it at all; rustup knows where
# the toolchain's cargo is.
if ! command -v cargo >/dev/null 2>&1; then
    PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
    if ! command -v cargo >/dev/null 2>&1 && command -v rustup >/dev/null 2>&1; then
        PATH="$(dirname "$(rustup which cargo)"):$PATH"
    fi
    export PATH
fi

# The oldest macOS the package and the app support (Package.swift,
# ../../project.yml); without it the library is built for the Mac building it.
export MACOSX_DEPLOYMENT_TARGET=14.0

cargo rustc --manifest-path "$manifest" --locked --release --target "$triple" \
    --lib --features ffi --crate-type staticlib
lib="$lib_dir/libmailcrew_core.a"

# The bindings are written from the library that was just built, so they
# always match it. Only the files that changed are replaced: Xcode runs this
# before every build, and a file rewritten as it was would rebuild the package.
bindgen() {
    cargo run --manifest-path "$manifest" --locked --features ffi-bindgen --bin uniffi-bindgen-swift -- "$@"
}
fresh="$(mktemp -d)"
trap 'rm -rf "$fresh"' EXIT
update() {
    mkdir -p "$2"
    for file in "$1"/*; do
        cmp -s "$file" "$2/${file##*/}" || cp "$file" "$2/"
    done
}
bindgen --headers "$lib" "$fresh/ffi"
update "$fresh/ffi" Sources/mailcrew_coreFFI
bindgen --swift-sources "$lib" "$fresh/swift"
update "$fresh/swift" Sources/MailCrewCore/Generated
echo "built $lib"
