import AppKit

/// The name of the command a gesture stood for, shown for a moment beside
/// the pointer, in a panel that never takes the focus or a click.
@MainActor
final class Toast {
    private let panel: NSPanel
    private let label = NSTextField(labelWithString: "")
    private var hide: DispatchWorkItem?

    init() {
        panel = NSPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: true
        )
        panel.level = .popUpMenu
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true
        panel.collectionBehavior = [.canJoinAllSpaces, .transient, .ignoresCycle]

        let background = NSVisualEffectView()
        background.material = .hudWindow
        background.state = .active
        background.wantsLayer = true
        background.layer?.cornerRadius = 8
        label.font = .systemFont(ofSize: 14, weight: .semibold)
        label.textColor = .labelColor
        label.translatesAutoresizingMaskIntoConstraints = false
        background.addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: background.leadingAnchor, constant: 14),
            label.trailingAnchor.constraint(equalTo: background.trailingAnchor, constant: -14),
            label.topAnchor.constraint(equalTo: background.topAnchor, constant: 8),
            label.bottomAnchor.constraint(equalTo: background.bottomAnchor, constant: -8),
        ])
        panel.contentView = background
    }

    /// Shows `text` below and to the right of the pointer, kept on its
    /// screen.
    func show(_ text: String) {
        label.stringValue = text
        let size = panel.contentView?.fittingSize ?? NSSize(width: 120, height: 32)
        let mouse = NSEvent.mouseLocation
        var origin = NSPoint(x: mouse.x + 18, y: mouse.y - 18 - size.height)
        if let screen = NSScreen.screens.first(where: { NSMouseInRect(mouse, $0.frame, false) }) {
            let visible = screen.visibleFrame
            origin.x = min(max(origin.x, visible.minX), visible.maxX - size.width)
            origin.y = min(max(origin.y, visible.minY), visible.maxY - size.height)
        }
        panel.setFrame(NSRect(origin: origin, size: size), display: true)
        panel.orderFrontRegardless()
        hide?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.panel.orderOut(nil) }
        hide = work
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.9, execute: work)
    }
}
