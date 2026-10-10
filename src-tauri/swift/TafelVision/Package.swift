// swift-tools-version:5.9
import PackageDescription

// Thin bridge from Rust to Apple Vision: hand-pose landmarks in, JSON out.
// Built and linked by src-tauri/build.rs through swift-rs.
let package = Package(
    name: "TafelVision",
    platforms: [
        .macOS(.v12),
    ],
    products: [
        .library(
            name: "TafelVision",
            type: .static,
            targets: ["TafelVision"]),
    ],
    dependencies: [
        .package(url: "https://github.com/Brendonovich/swift-rs", from: "1.0.7"),
    ],
    targets: [
        .target(
            name: "TafelVision",
            dependencies: [.product(name: "SwiftRs", package: "swift-rs")],
            path: "Sources/TafelVision"),
    ]
)
