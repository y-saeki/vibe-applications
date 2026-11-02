// myaku for macOS: a menu bar app that keeps the last minute of every
// process's CPU, memory and network use. The history and the list are kept
// by the Rust library (src/ffi.rs); this side draws them.

import AppKit

// build.sh draws the app's icon with the app itself: `--iconset <folder>`.
let arguments = CommandLine.arguments
if arguments.count == 3, arguments[1] == "--iconset" {
    do {
        try Icon.writeIconset(to: URL(fileURLWithPath: arguments[2]))
        exit(0)
    } catch {
        FileHandle.standardError.write(Data("\(error)\n".utf8))
        exit(1)
    }
}

// Top-level code runs on the main thread.
MainActor.assumeIsolated {
    let app = NSApplication.shared
    let delegate = AppDelegate()
    app.delegate = delegate
    app.run()
}
