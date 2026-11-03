// swift-tools-version:5.9
// meta-gesture for macOS. build.sh builds the Rust library and writes the
// bindings to it before this package can build; see DEVELOPMENT.md.

import PackageDescription

let package = Package(
    name: "MetaGesture",
    platforms: [.macOS(.v13)],
    targets: [
        // The C side of the bindings: the header UniFFI writes, and the
        // static library (libmeta_gesture.a) it declares.
        .systemLibrary(name: "meta_gestureFFI", path: "Sources/meta_gestureFFI"),
        .executableTarget(
            name: "MetaGesture",
            dependencies: ["meta_gestureFFI"],
            path: "Sources/MetaGesture"
        ),
    ]
)
