//! Writes the Swift side of the bindings in src/ffi.rs from the built
//! library: `cargo run --features ffi-bindgen --bin uniffi-bindgen-swift`.
//! macos/build.sh runs it.

fn main() {
    uniffi::uniffi_bindgen_swift()
}
