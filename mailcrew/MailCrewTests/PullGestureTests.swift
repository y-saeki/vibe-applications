import SwiftUI
import XCTest
@testable import MailCrewMac

/// Pull to reload: only fingers that start at the first card, move right
/// first and let go past the threshold reload.
final class PullGestureTests: XCTestCase {
    /// Finger travel that pulls the cards just past the threshold.
    private var armingTravel: CGFloat {
        -PullGesture.limit * log(1 - PullGesture.threshold / PullGesture.limit) + 1
    }

    private func pulled(by travel: CGFloat) -> PullGesture {
        var gesture = PullGesture()
        gesture.began(offset: 0)
        XCTAssertTrue(gesture.moved(dx: travel, dy: 0))
        return gesture
    }

    func testPullingPastTheThresholdAndLettingGoReloads() {
        var gesture = pulled(by: 20)
        XCTAssertTrue(gesture.pulling)
        XCTAssertFalse(gesture.armed)
        XCTAssertTrue(gesture.moved(dx: armingTravel - 20, dy: 0))
        XCTAssertTrue(gesture.armed)
        XCTAssertEqual(gesture.progress, 1)
        XCTAssertTrue(gesture.ended())
        XCTAssertEqual(gesture, PullGesture())
    }

    /// The cards follow the fingers closely at first and slow down toward
    /// the limit, so a long swipe pulls far without leaving the window.
    func testThePullSlowsTowardItsLimit() {
        XCTAssertEqual(PullGesture.distance(travel: 0), 0)
        XCTAssertEqual(PullGesture.distance(travel: 10), 10, accuracy: 0.2)
        let half = PullGesture.distance(travel: 400)
        let whole = PullGesture.distance(travel: 800)
        XCTAssertGreaterThan(half, PullGesture.threshold)
        XCTAssertGreaterThan(whole, half)
        XCTAssertLessThan(whole - half, half)
        XCTAssertLessThan(PullGesture.distance(travel: 10_000), PullGesture.limit)
    }

    func testLettingGoShortOfTheThresholdDoesNotReload() {
        var gesture = pulled(by: armingTravel - 2)
        XCTAssertFalse(gesture.armed)
        XCTAssertFalse(gesture.ended())
    }

    func testPullingBackBeforeLettingGoDoesNotReload() {
        var gesture = pulled(by: armingTravel)
        XCTAssertTrue(gesture.armed)
        XCTAssertTrue(gesture.moved(dx: -armingTravel / 2, dy: 0))
        XCTAssertFalse(gesture.armed)
        XCTAssertFalse(gesture.ended())
    }

    /// Pushed back past the first card, the fingers scroll the strip again.
    func testPushingBackPastTheFirstCardHandsTheStripBack() {
        var gesture = pulled(by: 30)
        XCTAssertTrue(gesture.moved(dx: -40, dy: 0))
        XCTAssertFalse(gesture.pulling)
        XCTAssertFalse(gesture.moved(dx: -40, dy: 0))
        XCTAssertFalse(gesture.ended())
    }

    /// A swipe that starts at older mail and runs on past the first card
    /// only bounces.
    func testAGestureStartedAwayFromTheFirstCardNeverPulls() {
        var gesture = PullGesture()
        gesture.began(offset: 600)
        XCTAssertFalse(gesture.moved(dx: 600, dy: 0))
        XCTAssertFalse(gesture.moved(dx: 400, dy: 0))
        XCTAssertFalse(gesture.pulling)
        XCTAssertFalse(gesture.ended())
    }

    /// At the first card, scrolling into the strip or a body scrolls as usual.
    func testOnlyARightwardSidewaysMoveStartsAPull() {
        var left = PullGesture()
        left.began(offset: 0)
        XCTAssertFalse(left.moved(dx: -10, dy: 0))
        XCTAssertFalse(left.moved(dx: 300, dy: 0))
        XCTAssertFalse(left.pulling)

        var down = PullGesture()
        down.began(offset: 0)
        XCTAssertFalse(down.moved(dx: 3, dy: -12))
        XCTAssertFalse(down.moved(dx: 300, dy: 0))
        XCTAssertFalse(down.pulling)

        var still = PullGesture()
        still.began(offset: 0)
        XCTAssertFalse(still.moved(dx: 0, dy: 0))
        XCTAssertTrue(still.moved(dx: 5, dy: 1))
        XCTAssertTrue(still.pulling)
    }

    func testCancellingDoesNotReload() {
        var gesture = pulled(by: armingTravel)
        gesture.cancelled()
        XCTAssertEqual(gesture, PullGesture())
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
