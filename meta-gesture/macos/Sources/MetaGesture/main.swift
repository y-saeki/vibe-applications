// meta-gesture for macOS: a menu bar app that turns a drag with the right
// button held into keys sent to the application under it. What a gesture
// comes to is decided by the Rust library (src/ffi.rs); this side passes it
// what macOS says and does what comes back.

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
