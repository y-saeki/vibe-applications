import AppKit
import SwiftUI

/// The settings window around SettingsView. Its close button goes through
/// the model, which asks before throwing changes away. Its width is fixed,
/// since a wider window only adds empty space; only the list grows taller.
@MainActor
final class SettingsWindow: NSObject, NSWindowDelegate {
    let model: SettingsModel
    private let window: NSWindow

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
    }

    func show() {
        NSApp.activate(ignoringOtherApps: true)
        window.makeKeyAndOrderFront(nil)
    }

    func close() {
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
