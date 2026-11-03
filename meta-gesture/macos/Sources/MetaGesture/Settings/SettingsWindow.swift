import AppKit
import SwiftUI

/// The settings window around SettingsView. Its close button goes through
/// the model, which asks before throwing changes away.
@MainActor
final class SettingsWindow: NSObject, NSWindowDelegate {
    let model: SettingsModel
    private let window: NSWindow
    /// Takes the keys for the recording dialog while it is open.
    private var keyMonitor: Any?

    init(model: SettingsModel) {
        self.model = model
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        window = NSWindow(contentViewController: NSHostingController(rootView: SettingsView(model: model)))
        super.init()
        window.title = "meta-gesture の設定 - v\(version)"
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.center()
        // Every key typed into meta-gesture goes to the dialog while it is
        // open, Command and all, before a menu can take it.
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) { [weak self] event in
            guard let self else { return event }
            // Local monitors are called on the main thread.
            let taken = MainActor.assumeIsolated { () -> Bool in
                guard self.model.isRecording else { return false }
                self.model.record(event)
                return true
            }
            return taken ? nil : event
        }
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    func close() {
        if let keyMonitor {
            NSEvent.removeMonitor(keyMonitor)
        }
        keyMonitor = nil
        window.delegate = nil
        window.close()
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        model.cancel()
        return false
    }
}
