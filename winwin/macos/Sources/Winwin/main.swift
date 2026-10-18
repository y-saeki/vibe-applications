// winwin for macOS: a menu bar app that places the frontmost window when one
// of the config's shortcuts is pressed. What a shortcut does is decided by
// the Rust library (src/ffi.rs); this side passes it what macOS says and does
// what comes back.

import AppKit

let app = NSApplication.shared
let delegate = AppDelegate()
app.delegate = delegate
app.run()
