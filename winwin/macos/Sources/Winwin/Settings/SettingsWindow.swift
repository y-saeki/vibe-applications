import AppKit
import SwiftUI

/// The settings window around SettingsView. Its close button goes through
/// the model, which asks before throwing changes away. Its width is fixed,
/// since a wider window only adds empty space; only the list grows taller.
@MainActor
final class SettingsWindow: NSObject, NSWindowDelegate {
    let model: SettingsModel
    private let window: NSWindow
    /// Takes ↑ and ↓ for the list, and the keys for the recording dialog.
    private var keyMonitor: Any?

    init(model: SettingsModel, path: String) {
        self.model = model
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
        window = NSWindow(
            contentViewController: NSHostingController(rootView: SettingsView(model: model, path: path))
        )
        super.init()
        window.title = "winwin の設定 - v\(version)"
        window.styleMask = [.titled, .closable, .miniaturizable, .resizable]
        window.isReleasedWhenClosed = false
        window.delegate = self
        window.center()
        keyMonitor = NSEvent.addLocalMonitorForEvents(matching: [.keyDown, .keyUp, .flagsChanged]) { [weak self] event in
            guard let self else { return event }
            // Local monitors are called on the main thread.
            let taken = MainActor.assumeIsolated { self.take(event) }
            return taken ? nil : event
        }
    }

    /// True when the event was taken: by the recording dialog while it is
    /// open, which gets every key typed into winwin, or by the list.
    private func take(_ event: NSEvent) -> Bool {
        if model.isRecording {
            model.record(event)
            return true
        }
        return event.type == .keyDown && arrow(event)
    }

    /// ↑ and ↓ move the list's selection, Shift+↑ and Shift+↓ widen it, as
    /// on Windows; whatever field is focused, but for one being typed into,
    /// which keeps them for its text. True when the key was taken.
    private func arrow(_ event: NSEvent) -> Bool {
        guard event.window === window, !(window.firstResponder is NSText) else { return false }
        let held = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        guard held.subtracting([.shift, .numericPad, .function]).isEmpty else { return false }
        switch event.keyCode {
        case 126: model.step(up: true, shift: held.contains(.shift))
        case 125: model.step(up: false, shift: held.contains(.shift))
        default: return false
        }
        return true
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

    /// Kept above the test window, which a shortcut may stretch over the
    /// whole screen, so that it can always be closed.
    func setAbove(_ on: Bool) {
        window.level = on ? .floating : .normal
    }

    func setAppearance(_ appearance: NSAppearance?) {
        window.appearance = appearance
    }

    var isKey: Bool { window.isKeyWindow }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        model.cancel()
        return false
    }
}
