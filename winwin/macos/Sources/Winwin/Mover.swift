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
/// or a drag can give it back.
@MainActor
enum Mover {
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
        guard let number = Windows.number(of: window) else { return }
        Windows.forgetClosed(resident: resident)
        resident.placed(
            window: number, owner: Windows.owner(of: pid), before: current, after: frame(of: window) ?? target
        )
    }

    /// Gives the frontmost window back where it was before a shortcut first
    /// placed it; a window winwin has not placed stays.
    static func reset(resident: Resident) {
        guard let focused = frontmost(), let number = Windows.number(of: focused.window) else { return }
        Windows.forgetClosed(resident: resident)
        guard let to = resident.reset(window: number, current: focused.frame, screens: Screens.frames)
        else { return }
        setFrame(of: focused.window, to: to)
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

    static func frame(of window: AXUIElement) -> Frame? {
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
    static func setFrame(of window: AXUIElement, to frame: Frame) {
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

/// The window server's number for an Accessibility API window. No public
/// call gives it; Rectangle, yabai and AltTab use this one too.
@_silgen_name("_AXUIElementGetWindow")
private func axWindowNumber(_ element: AXUIElement, _ number: UnsafeMutablePointer<CGWindowID>) -> AXError

/// How the library tells windows apart: by the window server's number,
/// which stays the same while a window is open, also across a restart of
/// winwin. The Accessibility API hands out a new element for a window each
/// time it is asked.
@MainActor
enum Windows {
    static func number(of window: AXUIElement) -> UInt64? {
        var number = CGWindowID(0)
        guard axWindowNumber(window, &number) == .success, number != 0 else { return nil }
        return UInt64(number)
    }

    /// The application `pid` as it was started, so that a window number
    /// met after a restart of winwin is known to be the same window: a
    /// process number alone may come round again.
    static func owner(of pid: pid_t) -> String {
        let launched = NSRunningApplication(processIdentifier: pid)?.launchDate?.timeIntervalSince1970 ?? 0
        return "\(pid)-\(Int(launched))"
    }

    /// Every window there is, on any Space, and its application's pid.
    private static func all() -> [(number: UInt64, pid: pid_t)] {
        guard let list = CGWindowListCopyWindowInfo(.optionAll, kCGNullWindowID) as? [[String: Any]]
        else { return [] }
        return list.compactMap { info in
            guard let number = info[kCGWindowNumber as String] as? CGWindowID,
                  let pid = info[kCGWindowOwnerPID as String] as? pid_t
            else { return nil }
            return (UInt64(number), pid)
        }
    }

    /// The windows open now, for the library to pick up what an earlier
    /// winwin remembered of them.
    static func open() -> [OpenWindow] {
        all().map { OpenWindow(window: $0.number, owner: owner(of: $0.pid)) }
    }

    /// Lets the library go of the windows that have closed.
    static func forgetClosed(resident: Resident) {
        resident.retain(alive: all().map { $0.number })
    }

    /// The ordinary window under `point`, the frontmost one there, as the
    /// window server sees it.
    static func at(_ point: CGPoint) -> (number: UInt64, pid: pid_t)? {
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]]
        else { return nil }
        for info in list {
            guard (info[kCGWindowLayer as String] as? Int) == 0,
                  let bounds = info[kCGWindowBounds as String] as? NSDictionary,
                  let rect = CGRect(dictionaryRepresentation: bounds),
                  rect.contains(point)
            else { continue }
            guard let number = info[kCGWindowNumber as String] as? CGWindowID,
                  let pid = info[kCGWindowOwnerPID as String] as? pid_t
            else { return nil }
            return (UInt64(number), pid)
        }
        return nil
    }

    /// The Accessibility API's element for window `number` of `pid`.
    static func element(_ number: UInt64, of pid: pid_t) -> AXUIElement? {
        var value: CFTypeRef?
        guard AXUIElementCopyAttributeValue(
            AXUIElementCreateApplication(pid), kAXWindowsAttribute as CFString, &value
        ) == .success,
            let windows = value as? [AXUIElement]
        else { return nil }
        return windows.first { Self.number(of: $0) == number }
    }
}
