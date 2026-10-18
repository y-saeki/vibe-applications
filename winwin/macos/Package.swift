// swift-tools-version:5.9
// winwin for macOS. build.sh builds the Rust library and writes the bindings
// to it before this package can build; see DEVELOPMENT.md.

import PackageDescription

let package = Package(
    name: "Winwin",
    platforms: [.macOS(.v13)],
    targets: [
        // The C side of the bindings: the header UniFFI writes, and the
        // static library (libwinwin.a) it declares.
        .systemLibrary(name: "winwinFFI", path: "Sources/winwinFFI"),
        .executableTarget(
            name: "Winwin",
            dependencies: ["winwinFFI"],
            path: "Sources/Winwin"
        ),
    ]
)
