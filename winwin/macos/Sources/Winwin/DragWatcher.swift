import AppKit

/// Follows the left button for drags of the windows winwin has placed, and
/// gives such a window its own size back once it moves, as `Resident`
/// says. A monitor of other applications' events only watches: the drag
/// itself is the window server's or the application's, and carries on at
/// the size winwin gives the window.
@MainActor
final class DragWatcher {
    private let resident: Resident
    private var monitor: Any?
    /// The window under the button, while a drag of it is followed.
    private var window: (number: UInt64, element: AXUIElement)?

    init(resident: Resident) {
        self.resident = resident
    }

    func start() {
        guard monitor == nil else { return }
        monitor = NSEvent.addGlobalMonitorForEvents(matching: [.leftMouseDown, .leftMouseDragged, .leftMouseUp]) {
            [weak self] event in
            // Global monitors are called on the main thread.
            MainActor.assumeIsolated { self?.handle(event) }
        }
    }

    private func handle(_ event: NSEvent) {
        // Points from the top left of the main screen, as windows are moved in.
        guard let cursor = event.cgEvent?.location else { return }
        let (x, y) = (Int32(cursor.x.rounded()), Int32(cursor.y.rounded()))
        switch event.type {
        case .leftMouseDown:
            window = nil
            guard let under = Windows.at(cursor), resident.tracks(window: under.number),
                  let element = Windows.element(under.number, of: under.pid),
                  let current = Mover.frame(of: element),
                  resident.mouseDown(window: under.number, current: current)
            else { return }
            window = (under.number, element)
        case .leftMouseDragged:
            guard let window, let current = Mover.frame(of: window.element),
                  let to = resident.mouseDragged(window: window.number, current: current, cursorX: x, cursorY: y)
            else { return }
            Mover.setFrame(of: window.element, to: to)
        case .leftMouseUp:
            guard let window else { return }
            self.window = nil
            guard let current = Mover.frame(of: window.element),
                  let to = resident.mouseUp(window: window.number, current: current, cursorX: x, cursorY: y)
            else { return }
            Mover.setFrame(of: window.element, to: to)
        default:
            break
        }
    }
}
