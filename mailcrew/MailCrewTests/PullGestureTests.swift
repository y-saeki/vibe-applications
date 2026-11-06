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
/// NSScrollView it can reach from inside the content.
@MainActor
final class PullToRefreshMonitorTests: XCTestCase {
    func testTheMonitorFindsTheStripsScrollView() throws {
        var coordinator: PullToRefreshMonitor.Coordinator?
        let root = ScrollView(.horizontal) {
            HStack { Color.red.frame(width: 2000, height: 100) }
                .background {
                    Probe { coordinator = $0 }
                }
        }
        let window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 600, height: 200),
                              styleMask: [.titled], backing: .buffered, defer: false)
        window.contentView = NSHostingView(rootView: root)
        window.layoutIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.2))
        let scrollView = try XCTUnwrap(coordinator?.scrollView)
        XCTAssertTrue(scrollView.hasHorizontalScroller || scrollView.documentView != nil)
        XCTAssertEqual(scrollView.contentView.bounds.origin.x, 0)
        coordinator?.detach()
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
