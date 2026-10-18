import AppKit

/// The test window: a plain pane of frosted glass the settings window opens
/// on request, which the shortcuts being edited move about before they are
/// saved. It has no title or buttons; it can be dragged anywhere on it and
/// resized from its edges, and it takes any size a placement asks for. A
/// reset puts it back where it was before the first placement since it
/// opened or was last reset.
@MainActor
final class TestWindow: NSObject, NSWindowDelegate {
    private let window: NSWindow
    /// Where the window was before the first placement since it opened or
    /// was last reset.
    private var origin: Frame?
    /// Asked to close (Command-W): the settings window decides.
    private let onClose: () -> Void

    init(onClose: @escaping () -> Void) {
        self.onClose = onClose
        window = NSWindow(
            contentRect: TestWindow.firstFrame(),
            styleMask: [.titled, .closable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        super.init()
        window.title = "winwin テスト用ウィンドウ"
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = true
        for button in [NSWindow.ButtonType.closeButton, .miniaturizeButton, .zoomButton] {
            window.standardWindowButton(button)?.isHidden = true
        }
        window.isMovableByWindowBackground = true
        window.isReleasedWhenClosed = false
        window.minSize = NSSize(width: 1, height: 1)
        window.collectionBehavior = [.moveToActiveSpace]
        let glass = NSVisualEffectView()
        glass.material = .hudWindow
        glass.blendingMode = .behindWindow
        glass.state = .active
        window.contentView = glass
        window.delegate = self
    }

    /// Half the visible frame of the screen the pointer is on, in its
    /// middle.
    private static func firstFrame() -> NSRect {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        let visible = screen?.visibleFrame ?? NSRect(x: 0, y: 0, width: 800, height: 600)
        return visible.insetBy(dx: visible.width / 4, dy: visible.height / 4)
    }

    func show() {
        window.orderFront(nil)
    }

    func close() {
        window.delegate = nil
        window.close()
    }

    /// Where the window is, in the coordinates frames are placed in.
    private var frame: Frame {
        Screens.frame(of: window.frame)
    }

    func place(_ placement: Placing) {
        let current = frame
        guard let work = Screens.work(for: current) else { return }
        // winwin's own windows, the settings window among them, do not count.
        let own = ProcessInfo.processInfo.processIdentifier
        let others = placement.fits() ? Screens.windowFrames { owner, _ in owner == own } : []
        guard let target = placement.resolve(work: work, current: current, others: others) else { return }
        origin = origin ?? current
        window.setFrame(Screens.rect(of: target), display: true)
    }

    func reset() {
        guard let origin else { return }
        self.origin = nil
        window.setFrame(Screens.rect(of: origin), display: true)
    }

    func windowShouldClose(_ sender: NSWindow) -> Bool {
        onClose()
        return false
    }
}
