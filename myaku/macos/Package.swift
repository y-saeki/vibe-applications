// swift-tools-version:5.9
// myaku for macOS. build.sh builds the Rust library and writes the bindings
// to it before this package can build; see DEVELOPMENT.md.

import PackageDescription

let package = Package(
    name: "Myaku",
    platforms: [.macOS(.v13)],
    targets: [
        // The C side of the bindings: the header UniFFI writes, and the
        // static library (libmyaku.a) it declares.
        .systemLibrary(name: "myakuFFI", path: "Sources/myakuFFI"),
        .executableTarget(
            name: "Myaku",
            dependencies: ["myakuFFI"],
            path: "Sources/Myaku"
        ),
    ]
)
