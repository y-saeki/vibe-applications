import AppKit

/// What meta-gesture puts in the events it posts ("MGST"), so that the tap
/// lets them through.
let postedMark: Int64 = 0x4D47_5354

/// Hears the right button and the drags made with it before the application
/// under the pointer does, through an event tap, and hands them to
/// `handler`, which says whether to keep each from the application.
final class MouseTap {
    /// The event, and its type; true keeps it from the application.
    var handler: @MainActor (CGEventType, CGEvent) -> Bool = { _, _ in false }
    private var port: CFMachPort?

    /// Installs the tap on the main run loop. False when macOS refuses it,
    /// as it does without the Accessibility permission.
    func start() -> Bool {
        if port != nil { return true }
        let events = [CGEventType.rightMouseDown, .rightMouseUp, .rightMouseDragged]
            .reduce(CGEventMask(0)) { $0 | CGEventMask(1) << $1.rawValue }
        guard let port = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: events,
            callback: mouseTapCallback,
            userInfo: Unmanaged.passUnretained(self).toOpaque()
        ) else { return false }
        self.port = port
        let source = CFMachPortCreateRunLoopSource(nil, port, 0)
        CFRunLoopAddSource(CFRunLoopGetMain(), source, .commonModes)
        CGEvent.tapEnable(tap: port, enable: true)
        return true
    }

    /// The event passed on, or nil to keep it from the application.
    fileprivate func handle(_ type: CGEventType, _ event: CGEvent) -> Unmanaged<CGEvent>? {
        switch type {
        case .tapDisabledByTimeout, .tapDisabledByUserInput:
            // macOS switches a tap off when it answers too slowly.
            if let port { CGEvent.tapEnable(tap: port, enable: true) }
        case .rightMouseDown, .rightMouseUp, .rightMouseDragged:
            if event.getIntegerValueField(.eventSourceUserData) == postedMark {
                break
            }
            // The tap's run loop source is on the main run loop.
            let take = MainActor.assumeIsolated { handler(type, event) }
            if take { return nil }
        default:
            break
        }
        return Unmanaged.passUnretained(event)
    }
}

private func mouseTapCallback(
    proxy: CGEventTapProxy,
    type: CGEventType,
    event: CGEvent,
    userInfo: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
    guard let userInfo else { return Unmanaged.passUnretained(event) }
    let tap = Unmanaged<MouseTap>.fromOpaque(userInfo).takeUnretainedValue()
    return tap.handle(type, event)
}
