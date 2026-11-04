//! Writes the Swift side of the bindings in src/ffi.rs from the built
//! library: `cargo run --features ffi-bindgen --bin uniffi-bindgen-swift`.

fn main() {
    uniffi::uniffi_bindgen_swift()
}
