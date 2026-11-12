import AppKit
import SwiftUI

/// The list in a small window that floats above other apps' windows, so it
/// can be watched while working in them. It never takes the focus from the
/// app in front, follows to every Space, and opens where it was last left.
@MainActor
final class PinnedWindow: NSObject, NSWindowDelegate {
    private let panel: NSPanel
    private let model: Model
    /// Called when the window is closed with its close button.
    var onClose: (() -> Void)?

    init(model: Model) {
        self.model = model
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: ContentView.width, height: ContentView.height),
            styleMask: [.titled, .closable, .utilityWindow, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        super.init()
        panel.title = "myaku"
        panel.level = .floating
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.isMovableByWindowBackground = true
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        panel.contentView = NSHostingView(rootView: ContentView(model: model))
        panel.delegate = self
        if !panel.setFrameUsingName(Self.frameName) {
            panel.center()
        }
        panel.setFrameAutosaveName(Self.frameName)
    }

    private static let frameName = "pinnedList"

    var isOpen: Bool { panel.isVisible }

    func open() {
        model.windowOpen = true
        panel.orderFrontRegardless()
    }

    /// Closes it without telling `onClose`: the menu turned it off.
    func close() {
        let callback = onClose
        onClose = nil
        panel.close()
        onClose = callback
    }

    func windowWillClose(_ notification: Notification) {
        model.windowOpen = false
        onClose?()
    }
}
