import AppKit
import ApplicationServices

/// The screens, in the coordinates windows are moved in: points, counting
/// down from the top left of the main screen.
@MainActor
enum Screens {
    /// The main screen's height, which AppKit's coordinates count up from.
    static var mainHeight: Double {
        // The first screen is the main one.
        Double(NSScreen.screens.first?.frame.height ?? 0)
    }

    static func frame(of rect: NSRect) -> Frame {
        fromAppkit(x: rect.origin.x, y: rect.origin.y, width: rect.width, height: rect.height, mainHeight: mainHeight)
    }

    static func rect(of frame: Frame) -> NSRect {
        let r = toAppkit(frame: frame, mainHeight: mainHeight)
        return NSRect(x: r.x, y: r.y, width: r.width, height: r.height)
    }

    static var frames: [Frame] {
        NSScreen.screens.map { frame(of: $0.frame) }
    }

    /// The visible frame, less the menu bar and the Dock, of the screen the
    /// window at `window` is on.
    static func work(for window: Frame) -> Frame? {
        let screens = NSScreen.screens
        guard let i = areaOf(window: window, areas: screens.map { frame(of: $0.frame) }) else { return nil }
        return frame(of: screens[Int(i)].visibleFrame)
    }

    /// The frames of the ordinary windows on screen, front to back, as a
    /// placement that fits takes them, but for `skipping`'s.
    static func windowFrames(skipping: (pid_t, Frame) -> Bool) -> [Frame] {
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]]
        else { return [] }
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
            let owner = info[kCGWindowOwnerPID as String] as? pid_t ?? 0
            return skipping(owner, frame) ? nil : frame
        }
    }
}

/// Moves the frontmost window where a shortcut says, through the
/// Accessibility API, and tells the library where it was so that a reset
/// can give it back.
@MainActor
enum Mover {
    private static let ids = WindowIDs()

    static func place(_ placement: Placing, resident: Resident) {
        guard let focused = frontmost(), let work = Screens.work(for: focused.frame) else { return }
        let (pid, window, current) = (focused.pid, focused.window, focused.frame)
        var others: [Frame] = []
        if placement.fits() {
            // The window being placed is the frontmost of its application's.
            var skipped = false
            others = Screens.windowFrames { owner, frame in
                guard !skipped, owner == pid, frame == current else { return false }
                skipped = true
                return true
            }
        }
        guard let target = placement.resolve(work: work, current: current, others: others) else { return }
        setFrame(of: window, to: target)
        ids.forgetClosed(resident: resident)
        resident.placed(window: ids.id(of: window), before: current, after: frame(of: window) ?? target)
    }

    /// Gives the frontmost window back where it was before a shortcut first
    /// placed it; a window winwin has not placed stays.
    static func reset(resident: Resident) {
        guard let focused = frontmost() else { return }
        let (window, current) = (focused.window, focused.frame)
        ids.forgetClosed(resident: resident)
        guard let to = resident.reset(window: ids.id(of: window), current: current, screens: Screens.frames)
        else { return }
        setFrame(of: window, to: to)
    }

    /// The focused window of the frontmost application, and where it is.
    private struct Focused {
        let pid: pid_t
        let window: AXUIElement
        let frame: Frame
    }

    private static func frontmost() -> Focused? {
        guard let app = NSWorkspace.shared.frontmostApplication else { return nil }
        let pid = app.processIdentifier
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(
            AXUIElementCreateApplication(pid), kAXFocusedWindowAttribute as CFString, &value
        ) == .success,
            let value, CFGetTypeID(value) == AXUIElementGetTypeID()
        else { return nil }
        let window = value as! AXUIElement
        guard let current = frame(of: window) else { return nil }
        return Focused(pid: pid, window: window, frame: current)
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
}

/// A number for each window winwin has placed, which is how the library
/// tells them apart. The Accessibility API hands out a new element for a
/// window each time it is asked, but elements for the same window compare
/// equal.
@MainActor
final class WindowIDs {
    private var known: [(element: AXUIElement, id: UInt64)] = []
    private var next: UInt64 = 1

    func id(of window: AXUIElement) -> UInt64 {
        if let known = known.first(where: { CFEqual($0.element, window) }) {
            return known.id
        }
        let id = next
        next += 1
        known.append((window, id))
        return id
    }

    /// Lets go of the windows that have closed, here and in the library.
    func forgetClosed(resident: Resident) {
        known.removeAll { entry in
            var role: CFTypeRef?
            return AXUIElementCopyAttributeValue(entry.element, kAXRoleAttribute as CFString, &role)
                == .invalidUIElement
        }
        resident.retain(alive: known.map { $0.id })
    }
}
