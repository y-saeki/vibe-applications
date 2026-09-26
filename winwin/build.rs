use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    // The defaults are what winwin needs: per-monitor (v2) DPI awareness, so
    // that window coordinates are physical pixels on every monitor, and
    // version 6 of the Common Controls for the settings window.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        embed_manifest(new_manifest("winwin")).expect("unable to embed the manifest");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
