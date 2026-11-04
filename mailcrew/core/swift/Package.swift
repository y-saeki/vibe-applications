// swift-tools-version:5.9
// The Rust library (../) as a Swift module. build.sh builds the library and
// writes the bindings to it before this package can build; see
// mailcrew/DEVELOPMENT.md.

import PackageDescription

let package = Package(
    name: "MailCrewCore",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "MailCrewCore", targets: ["MailCrewCore"]),
    ],
    targets: [
        // The C side of the bindings: the header UniFFI writes, and the
        // static library (libmailcrew_core.a) it declares.
        .systemLibrary(name: "mailcrew_coreFFI", path: "Sources/mailcrew_coreFFI"),
        .target(
            name: "MailCrewCore",
            dependencies: ["mailcrew_coreFFI"],
            path: "Sources/MailCrewCore",
            linkerSettings: [
                // What the library's TLS, proxy and Keychain access need.
                .linkedFramework("CoreFoundation"),
                .linkedFramework("Security"),
                .linkedFramework("SystemConfiguration"),
            ]
        ),
        .testTarget(
            name: "MailCrewCoreTests",
            dependencies: ["MailCrewCore"],
            path: "Tests/MailCrewCoreTests"
        ),
    ]
)
