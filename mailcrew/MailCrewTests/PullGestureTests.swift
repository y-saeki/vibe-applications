import SwiftUI
import XCTest
@testable import MailCrewMac

/// Pull to reload: only a finger that starts at the first card and lets go
/// past the threshold reloads.
final class PullGestureTests: XCTestCase {
    private let past = -(PullGesture.threshold + 10)

    func testPullingPastTheThresholdAndLettingGoReloads() {
        var gesture = PullGesture()
        gesture.began(offset: 0)
        gesture.scrolled(offset: -20)
        XCTAssertTrue(gesture.tracking)
        XCTAssertFalse(gesture.armed)
        XCTAssertEqual(gesture.progress, 20 / PullGesture.threshold, accuracy: 0.001)
        gesture.scrolled(offset: past)
        XCTAssertTrue(gesture.armed)
        XCTAssertEqual(gesture.progress, 1)
        XCTAssertTrue(gesture.ended())
        XCTAssertEqual(gesture, PullGesture())
    }

    func testLettingGoShortOfTheThresholdDoesNotReload() {
        var gesture = PullGesture()
        gesture.began(offset: 0)
        gesture.scrolled(offset: -(PullGesture.threshold - 1))
        XCTAssertFalse(gesture.ended())
    }

    func testPullingBackBeforeLettingGoDoesNotReload() {
        var gesture = PullGesture()
        gesture.began(offset: 0)
        gesture.scrolled(offset: past)
        XCTAssertTrue(gesture.armed)
        gesture.scrolled(offset: -10)
        XCTAssertFalse(gesture.armed)
        XCTAssertFalse(gesture.ended())
    }

    /// A swipe that starts at older mail and runs on past the first card
    /// only bounces.
    func testAGestureStartedAwayFromTheFirstCardNeverReloads() {
        var gesture = PullGesture()
        gesture.began(offset: 600)
        gesture.scrolled(offset: 0)
        gesture.scrolled(offset: past)
        XCTAssertFalse(gesture.tracking)
        XCTAssertEqual(gesture.distance, 0)
        XCTAssertFalse(gesture.ended())
    }

    func testAGestureStartedWhileStillBouncingCounts() {
        var gesture = PullGesture()
        gesture.began(offset: -30)
        XCTAssertTrue(gesture.tracking)
        XCTAssertEqual(gesture.distance, 30)
    }

    func testCancellingDoesNotReload() {
        var gesture = PullGesture()
        gesture.began(offset: 0)
        gesture.scrolled(offset: past)
        gesture.cancelled()
        XCTAssertEqual(gesture, PullGesture())
        XCTAssertFalse(gesture.ended())
    }

    func testScrollingIntoTheStripKeepsTheDistanceAtZero() {
        var gesture = PullGesture()
        gesture.began(offset: 0)
        gesture.scrolled(offset: 200)
        XCTAssertEqual(gesture.distance, 0)
        XCTAssertFalse(gesture.ended())
    }
}

/// The monitor relies on SwiftUI's horizontal scroll view being an
/// NSScrollView it can reach from inside the content, and reads how far the
/// strip is scrolled from it.
@MainActor
final class PullToRefreshMonitorTests: XCTestCase {
    /// In the inbox the sidebar floats over the strip, and the scroll view
    /// insets its content by the sidebar's width. The strip at rest must
    /// still read as 0, or the smallest pull would count the inset as well.
    func testTheOffsetIgnoresTheSidebarsInset() throws {
        var coordinator: PullToRefreshMonitor.Coordinator?
        let root = NavigationSplitView {
            List { Text("受信トレイ") }
        } detail: {
            ScrollView(.horizontal) {
                HStack { Color.red.frame(width: 3000, height: 100) }
                    .background {
                        Probe { coordinator = $0 }
                    }
            }
        }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1200, height: 600),
                              styleMask: [.titled, .resizable, .fullSizeContentView],
                              backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: root)
        window.makeKeyAndOrderFront(nil)
        RunLoop.main.run(until: Date().addingTimeInterval(1))
        defer {
            coordinator?.detach()
            window.orderOut(nil)
        }
        let monitor = try XCTUnwrap(coordinator)
        let scrollView = try XCTUnwrap(monitor.scrollView)
        let clip = scrollView.contentView
        XCTAssertEqual(monitor.offset(of: scrollView), 0)

        // Only the bounce moves the origin past the first card, so the test
        // can only scroll into the strip; the same sum reads the pull.
        // Scrolled 200 into the strip.
        clip.setBoundsOrigin(NSPoint(x: -clip.contentInsets.left + 200, y: clip.bounds.origin.y))
        XCTAssertEqual(monitor.offset(of: scrollView), 200)
    }

    /// Hands out the monitor's coordinator.
    private struct Probe: NSViewRepresentable {
        let found: (PullToRefreshMonitor.Coordinator) -> Void

        func makeNSView(context: Context) -> NSView {
            let monitor = PullToRefreshMonitor(gesture: .constant(PullGesture()), enabled: true) {}
            let coordinator = monitor.makeCoordinator()
            coordinator.parent = monitor
            let view = PullToRefreshMonitor.MonitorView()
            view.coordinator = coordinator
            coordinator.attach(to: view)
            found(coordinator)
            context.coordinator.keep = coordinator
            return view
        }

        func updateNSView(_ view: NSView, context: Context) {}
        func makeCoordinator() -> Holder { Holder() }

        final class Holder { var keep: PullToRefreshMonitor.Coordinator? }
    }
}
