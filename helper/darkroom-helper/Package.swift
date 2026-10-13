// swift-tools-version:5.9
// Darkroom's Mac helper: the one small Swift program that reaches PhotoKit and the macOS print
// system for the Rust app. Build with `swift build -c release` inside this folder; the app looks
// for the binary next to itself, then at `helper/darkroom-helper/.build/release/darkroom-helper`.
import PackageDescription

let package = Package(
    name: "darkroom-helper",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "darkroom-helper",
            path: "Sources/darkroom-helper",
            linkerSettings: [
                // A command-line tool has no bundle, so the usage strings PhotoKit requires are
                // embedded in the binary's __info_plist section.
                .unsafeFlags(["-Xlinker", "-sectcreate", "-Xlinker", "__TEXT", "-Xlinker", "__info_plist",
                              "-Xlinker", "Sources/darkroom-helper/Info.plist"])
            ]
        ),
    ]
)
