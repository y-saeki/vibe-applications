import AppKit
import ApplicationServices

/// Moves the frontmost window where a placement says, on the screen it is
/// on. Frames are in the coordinates the Accessibility API uses: points,
/// counting down from the top left of the main screen.
@MainActor
enum Mover {
    static func place(_ placement: Placing) {
        guard let app = NSWorkspace.shared.frontmostApplication,
              let window = focusedWindow(of: app.processIdentifier),
              let current = frame(of: window)
        else { return }
        let screens = NSScreen.screens
        // The first screen is the main one, whose top AppKit counts from.
        guard let mainHeight = screens.first?.frame.height else { return }
        let convert = { (r: NSRect) in
            fromAppkit(x: r.origin.x, y: r.origin.y, width: r.width, height: r.height, mainHeight: mainHeight)
        }
        guard let i = areaOf(window: current, areas: screens.map { convert($0.frame) }) else { return }
        let work = convert(screens[Int(i)].visibleFrame)
        let others = placement.fits() ? otherFrames(excluding: current, of: app.processIdentifier) : []
        guard let target = placement.resolve(work: work, current: current, others: others) else { return }
        setFrame(of: window, to: target)
    }

    private static func focusedWindow(of pid: pid_t) -> AXUIElement? {
        let app = AXUIElementCreateApplication(pid)
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(app, kAXFocusedWindowAttribute as CFString, &value) == .success,
              let value, CFGetTypeID(value) == AXUIElementGetTypeID()
        else { return nil }
        return (value as! AXUIElement)
    }

    private static func frame(of window: AXUIElement) -> Frame? {
        guard let origin: CGPoint = attribute(kAXPositionAttribute, of: window, as: .cgPoint),
              let size: CGSize = attribute(kAXSizeAttribute, of: window, as: .cgSize)
        else { return nil }
        return Frame(
            left: Int32(origin.x.rounded()),
            top: Int32(origin.y.rounded()),
            right: Int32((origin.x + size.width).rounded()),
            bottom: Int32((origin.y + size.height).rounded())
        )
    }

    private static func attribute<T>(_ name: String, of window: AXUIElement, as type: AXValueType) -> T? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(window, name as CFString, &value) == .success,
              let value, CFGetTypeID(value) == AXValueGetTypeID()
        else { return nil }
        let pointer = UnsafeMutablePointer<T>.allocate(capacity: 1)
        defer { pointer.deallocate() }
        guard AXValueGetValue(value as! AXValue, type, pointer) else { return nil }
        return pointer.pointee
    }

    /// Size first, then position, then size again: moving to a smaller
    /// screen may have cut the first size short.
    private static func setFrame(of window: AXUIElement, to frame: Frame) {
        var origin = CGPoint(x: CGFloat(frame.left), y: CGFloat(frame.top))
        var size = CGSize(width: CGFloat(frame.right - frame.left), height: CGFloat(frame.bottom - frame.top))
        guard let position = AXValueCreate(.cgPoint, &origin),
              let extent = AXValueCreate(.cgSize, &size)
        else { return }
        AXUIElementSetAttributeValue(window, kAXSizeAttribute as CFString, extent)
        AXUIElementSetAttributeValue(window, kAXPositionAttribute as CFString, position)
        AXUIElementSetAttributeValue(window, kAXSizeAttribute as CFString, extent)
    }

    /// The frames of the ordinary windows on screen, front to back, other
    /// than the one being placed, as a placement that fits takes them.
    private static func otherFrames(excluding current: Frame, of pid: pid_t) -> [Frame] {
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]]
        else { return [] }
        var skipped = false
        return list.compactMap { info in
            // Layer 0 holds ordinary windows; the menu bar, the Dock and
            // windows kept above the others are on layers of their own.
            guard (info[kCGWindowLayer as String] as? Int) == 0,
                  let bounds = info[kCGWindowBounds as String] as? NSDictionary,
                  let rect = CGRect(dictionaryRepresentation: bounds)
            else { return nil }
            let frame = Frame(
                left: Int32(rect.minX.rounded()),
                top: Int32(rect.minY.rounded()),
                right: Int32(rect.maxX.rounded()),
                bottom: Int32(rect.maxY.rounded())
            )
            let owner = info[kCGWindowOwnerPID as String] as? pid_t
            if !skipped && owner == pid && frame == current {
                skipped = true
                return nil
            }
            return frame
        }
    }
}
