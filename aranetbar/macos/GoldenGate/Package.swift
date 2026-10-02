// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "AranetBarGoldenGate",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "AranetBarIntentsBridge", type: .static, targets: ["AranetBarIntentsBridge"]),
    ],
    targets: [
        .target(
            name: "AranetBarShared",
            path: "Sources/AranetBarShared"
        ),
        .target(
            name: "AranetBarIntentsBridge",
            dependencies: ["AranetBarShared"],
            path: "Sources/AranetBarIntentsBridge"
        ),
        // Control + Widget extension sources live under Sources/*Extension for Xcode targets
        // (see macos/GoldenGate/README.md). They are not built by this package to avoid @main clashes.
    ]
)
