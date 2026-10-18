import AppKit

/// Hears every key event before the application in front does, through an
/// event tap, and hands it to `handler`, which says whether to keep it from
/// the application. Who handles the keys changes with what winwin is doing:
/// the resident part's shortcuts, the test window's, or the recording dialog.
final class KeyTap {
    /// The event, and its type; true keeps it from the application.
    var handler: @MainActor (CGEventType, CGEvent) -> Bool = { _, _ in false }
    private var port: CFMachPort?

    /// Installs the tap on the main run loop. False when macOS refuses it,
    /// as it does without the Accessibility permission.
    func start() -> Bool {
        if port != nil { return true }
        let events = [CGEventType.keyDown, .keyUp, .flagsChanged]
            .reduce(CGEventMask(0)) { $0 | CGEventMask(1) << $1.rawValue }
        guard let port = CGEvent.tapCreate(
            tap: .cgSessionEventTap,
            place: .headInsertEventTap,
            options: .defaultTap,
            eventsOfInterest: events,
            callback: keyTapCallback,
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
        case .keyDown, .keyUp, .flagsChanged:
            // The tap's run loop source is on the main run loop.
            let take = MainActor.assumeIsolated { handler(type, event) }
            if take { return nil }
        default:
            break
        }
        return Unmanaged.passUnretained(event)
    }
}

extension CGEvent {
    var keyCode: UInt16 {
        UInt16(truncatingIfNeeded: getIntegerValueField(.keyboardEventKeycode))
    }

    var isRepeat: Bool {
        getIntegerValueField(.keyboardEventAutorepeat) != 0
    }
}

private func keyTapCallback(
    proxy: CGEventTapProxy,
    type: CGEventType,
    event: CGEvent,
    userInfo: UnsafeMutableRawPointer?
) -> Unmanaged<CGEvent>? {
    guard let userInfo else { return Unmanaged.passUnretained(event) }
    let tap = Unmanaged<KeyTap>.fromOpaque(userInfo).takeUnretainedValue()
    return tap.handle(type, event)
}
