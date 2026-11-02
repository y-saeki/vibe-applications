// The executable's icon is drawn from the same pixels as the app icon.
#[path = "src/art.rs"]
#[allow(dead_code)] // Only the Windows picture is used here.
mod art;
#[path = "src/icon_res.rs"]
mod icon_res;

fn main() {
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        // Per-monitor (v2) DPI awareness, so that the list is drawn sharp on
        // every monitor, and version 6 of the Common Controls for the menu.
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("myaku.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
        // The icon Explorer shows for myaku.exe, as a compiled resource the
        // linker takes like an object file.
        let res = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("icon.res");
        std::fs::write(&res, icon_res::icon_resource(art::pixels)).expect("writing icon.res");
        println!("cargo:rustc-link-arg-bins={}", res.display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=myaku.manifest");
    println!("cargo:rerun-if-changed=src/art.rs");
    println!("cargo:rerun-if-changed=src/icon_res.rs");
}
