fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // The settings window is WinUI 3. This stages the Windows App Runtime
        // next to the executable and embeds the manifest that registers its
        // classes, so that winwin needs no runtime installed separately.
        windows_reactor_setup::as_self_contained();
        // Merged into the manifest above by the linker: per-monitor (v2) DPI
        // awareness, so that window coordinates are physical pixels on every
        // monitor, and version 6 of the Common Controls for message boxes.
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("winwin.manifest");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=winwin.manifest");
}
