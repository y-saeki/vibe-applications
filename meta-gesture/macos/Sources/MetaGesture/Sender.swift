import AppKit

/// What a gesture does to the application: finding which one is under the
/// pointer, bringing it forward and sending it keys or choosing its menu
/// item, or giving it back the click the tap kept from it. Every event posted carries `postedMark`.
@MainActor
enum Sender {
    /// The application whose window is at `point`, in the coordinates of
    /// an event (points from the top left of the main screen): the frontmost
    /// window there in the normal layer. Reading the windows' frames and
    /// owners needs no permission.
    static func application(at point: CGPoint) -> NSRunningApplication? {
        let options: CGWindowListOption = [.optionOnScreenOnly, .excludeDesktopElements]
        guard let windows = CGWindowListCopyWindowInfo(options, kCGNullWindowID) as? [[String: Any]] else {
            return nil
        }
        for window in windows {
            guard (window[kCGWindowLayer as String] as? Int) == 0,
                  let bounds = window[kCGWindowBounds as String] as? NSDictionary,
                  let rect = CGRect(dictionaryRepresentation: bounds),
                  rect.contains(point),
                  let pid = window[kCGWindowOwnerPID as String] as? pid_t
            else { continue }
            return NSRunningApplication(processIdentifier: pid)
        }
        return nil
    }

    /// Brings `app` forward and then presses the key `code` with the
    /// modifier `flags` in it. An application in the back is asked to come
    /// forward and given a moment to; if it still is not in front, the keys
    /// go to it directly.
    static func send(code: UInt16, flags: UInt64, to app: NSRunningApplication) {
        if app.isActive {
            post(code: code, flags: flags, to: nil)
            return
        }
        app.activate(options: [])
        waitUntilActive(app, tries: 10) { active in
            post(code: code, flags: flags, to: active ? nil : app.processIdentifier)
        }
    }

    /// Brings `app` forward as `send` does, and then chooses its menu item
    /// `path`: the titles from the menu bar down.
    static func choose(_ path: [String], in app: NSRunningApplication, then done: @escaping (Menus.Pressed) -> Void) {
        let press = { done(Menus.press(path, in: app.processIdentifier)) }
        if app.isActive {
            press()
            return
        }
        app.activate(options: [])
        waitUntilActive(app, tries: 10) { _ in press() }
    }

    private static func waitUntilActive(_ app: NSRunningApplication, tries: Int, then: @escaping (Bool) -> Void) {
        if app.isActive || tries == 0 {
            then(app.isActive)
            return
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.03) {
            waitUntilActive(app, tries: tries - 1, then: then)
        }
    }

    /// The key down and up, through the session (to whatever is in front),
    /// or to the process `pid`.
    private static func post(code: UInt16, flags: UInt64, to pid: pid_t?) {
        let source = CGEventSource(stateID: .privateState)
        for down in [true, false] {
            guard let event = CGEvent(keyboardEventSource: source, virtualKey: code, keyDown: down) else { continue }
            event.flags = CGEventFlags(rawValue: flags)
            event.setIntegerValueField(.eventSourceUserData, value: postedMark)
            if let pid {
                event.postToPid(pid)
            } else {
                event.post(tap: .cghidEventTap)
            }
        }
    }

    /// A right click at `point`, in place of the one the tap kept.
    static func clickRight(at point: CGPoint) {
        let source = CGEventSource(stateID: .hidSystemState)
        for type in [CGEventType.rightMouseDown, .rightMouseUp] {
            guard let event = CGEvent(
                mouseEventSource: source, mouseType: type, mouseCursorPosition: point, mouseButton: .right
            ) else { continue }
            event.setIntegerValueField(.eventSourceUserData, value: postedMark)
            event.post(tap: .cghidEventTap)
        }
    }
}
