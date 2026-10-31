// swift-tools-version: 5.10
import PackageDescription

// The data layer of Tenabuku, split by how much each source can be trusted to
// stay put. Each kind lives in its own module so that a breakage on Hatena's
// side is fixed in one place:
//
// - HatenaOfficial:   documented APIs (OAuth REST API, entry info JSON)
// - HatenaFeed:       RSS feeds
// - HatenaUnofficial: undocumented endpoints the website itself uses
//
// None of the three depends on another; what they share is in HatenaShared.
// The package has no third-party dependencies and builds on Linux as well, so
// `swift test` runs anywhere a Swift toolchain does.
let package = Package(
    name: "TenabukuKit",
    platforms: [.iOS(.v17), .macOS(.v14)],
    products: [
        .library(name: "HatenaShared", targets: ["HatenaShared"]),
        .library(name: "HatenaOfficial", targets: ["HatenaOfficial"]),
        .library(name: "HatenaFeed", targets: ["HatenaFeed"]),
        .library(name: "HatenaUnofficial", targets: ["HatenaUnofficial"]),
    ],
    targets: [
        .target(name: "HatenaShared"),
        .target(name: "HatenaOfficial", dependencies: ["HatenaShared"]),
        .target(name: "HatenaFeed", dependencies: ["HatenaShared"]),
        .target(name: "HatenaUnofficial", dependencies: ["HatenaShared"]),
        // Stub transport shared by the test targets; not part of any product.
        .target(name: "HatenaTestSupport", dependencies: ["HatenaShared"]),
        .testTarget(name: "HatenaSharedTests", dependencies: ["HatenaShared", "HatenaTestSupport"]),
        .testTarget(name: "HatenaOfficialTests", dependencies: ["HatenaOfficial", "HatenaTestSupport"]),
        .testTarget(name: "HatenaFeedTests", dependencies: ["HatenaFeed", "HatenaTestSupport"]),
        .testTarget(name: "HatenaUnofficialTests", dependencies: ["HatenaUnofficial", "HatenaTestSupport"]),
    ]
)
