import AppKit

/// Hears every key press before the application in front does, through an
/// event tap, and keeps the ones that are winwin's shortcuts from it. What
/// is a shortcut and what it does is the library's to say.
final class KeyTap {
    private let resident: Resident
    private var port: CFMachPort?

    init(resident: Resident) {
        self.resident = resident
    }

    /// Installs the tap on the main run loop. False when macOS refuses it,
    /// as it does without the Accessibility permission.
    func start() -> Bool {
        if port != nil { return true }
        let events = [CGEventType.keyDown, .flagsChanged]
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
        case .flagsChanged:
            resident.flagsChanged(flags: event.flags.rawValue)
        case .keyDown:
            let code = UInt16(truncatingIfNeeded: event.getIntegerValueField(.keyboardEventKeycode))
            let held = event.getIntegerValueField(.keyboardEventAutorepeat) != 0
            switch resident.keyDown(code: code, flags: event.flags.rawValue, held: held) {
            case .pass:
                break
            case .take:
                return nil
            case let .place(placement):
                // Moving another application's window can be slow; it is
                // done after the tap has answered.
                Task { @MainActor in Mover.place(placement) }
                return nil
            }
        default:
            break
        }
        return Unmanaged.passUnretained(event)
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
