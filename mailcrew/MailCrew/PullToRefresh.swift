import AppKit
import SwiftUI

/// Pulling the strip right past its first card, then letting go, reloads it.
///
/// Only a pull with a finger counts: the gesture must start while the strip
/// rests at its first card and move right first. From then on the strip does
/// not scroll: the scroll events go to the pull, which moves the cards itself
/// with a resistance of its own. The scroll view's own bounce stops after a
/// few dozen points however far the fingers travel, too short to pull on.
/// The fingers must leave the trackpad while the cards are pulled at least
/// `threshold`. Momentum that carries the strip back from older mail only
/// bounces, and so does a mouse wheel.
struct PullGesture: Equatable {
    /// How far the cards must be pulled to reload.
    static let threshold: CGFloat = 336
    /// How far the cards can be pulled at most; the pull slows toward it.
    static let limit: CGFloat = 784

    enum Phase: Equatable {
        /// No gesture, or one this pull leaves to the scroll view.
        case idle
        /// Fingers started at the first card; the first movement decides.
        case pending
        /// The fingers are pulling the cards.
        case pulling
    }

    private(set) var phase = Phase.idle
    /// How far the fingers have moved right since the pull began.
    private(set) var travel: CGFloat = 0

    var pulling: Bool { phase == .pulling }
    /// How far the cards are pulled: the fingers' travel, slowing toward `limit`.
    var distance: CGFloat { pulling ? Self.distance(travel: travel) : 0 }
    /// Letting go now reloads.
    var armed: Bool { distance >= Self.threshold }
    /// From 0 to 1 as the pull nears the threshold.
    var progress: CGFloat { min(1, distance / Self.threshold) }

    static func distance(travel: CGFloat) -> CGFloat {
        guard travel > 0 else { return 0 }
        return limit * (1 - exp(-travel / limit))
    }

    /// Fingers started moving on the trackpad, with the strip scrolled
    /// `offset` from its first card.
    mutating func began(offset: CGFloat) {
        phase = offset <= 1 ? .pending : .idle
        travel = 0
    }

    /// The fingers moved by `dx` (right is positive) and `dy`. True when the
    /// pull takes the movement, so the strip must not scroll by it.
    mutating func moved(dx: CGFloat, dy: CGFloat) -> Bool {
        switch phase {
        case .idle:
            return false
        case .pending:
            if dx == 0 && dy == 0 { return false }
            // Right and mostly sideways pulls; anything else scrolls as usual.
            guard dx > 0, abs(dx) >= abs(dy) else {
                phase = .idle
                return false
            }
            phase = .pulling
            travel = dx
            return true
        case .pulling:
            travel += dx
            if travel <= 0 {
                // Pushed back past the first card: the strip scrolls again.
                self = PullGesture()
            }
            return true
        }
    }

    /// The fingers left the trackpad. True when that should reload.
    mutating func ended() -> Bool {
        let reload = armed
        self = PullGesture()
        return reload
    }

    /// The system took the gesture away; nothing reloads.
    mutating func cancelled() {
        self = PullGesture()
    }
}

/// What appears in the gap the pull opens left of the first card: an arrow
/// that turns as the pull grows, a filled one once letting go would reload,
/// and a spinner while the reload runs.
struct PullIndicator: View {
    let gesture: PullGesture
    let refreshing: Bool
    let width: CGFloat

    var body: some View {
        VStack(spacing: 10) {
            if refreshing {
                ProgressView()
                    .frame(height: 32)
                Text("読み込み中")
            } else if gesture.armed {
                Image(systemName: "arrow.clockwise.circle.fill")
                    .font(.system(size: 30))
                    .foregroundStyle(.tint)
                    .frame(height: 32)
                    .symbolEffect(.bounce, value: gesture.armed)
                Text("離すと再読み込み")
                    .fontWeight(.semibold)
                    .foregroundStyle(.tint)
            } else {
                ReloadArrow()
                    .frame(width: 24, height: 24)
                    .frame(width: 32, height: 32)
                    .rotationEffect(.degrees(gesture.progress * 300))
                    // Faded out while the gap is narrower than the arrow, so
                    // no sliver of it is left at the gap's edge.
                    .opacity(min(1, max(0, (width - 24) / 32)))
                // Only once the gap is wide enough to show it whole.
                Text("引っ張って再読み込み")
                    .opacity(width >= 150 ? 1 : 0)
            }
        }
        .font(.callout)
        .foregroundStyle(.secondary)
        .lineLimit(1)
        .fixedSize()
        .opacity(refreshing ? 1 : 0.5 + 0.5 * gesture.progress)
        // Switching between the states never fades one over the other; only
        // the gap around them animates.
        .transaction { $0.animation = nil }
        .frame(width: max(0, width))
        .frame(maxHeight: .infinity)
        .clipped()
        .accessibilityHidden(true)
    }
}

/// A clockwise arrow drawn on a circle centred in its frame. SF Symbols'
/// `arrow.clockwise` is not: its arrowhead stands out of the circle, so its
/// frame's centre is off the circle's and the arrow wobbles as it turns.
struct ReloadArrow: View {
    /// Where the arc starts and how far it runs, clockwise from the right.
    static let start = Angle.degrees(-40)
    static let sweep = Angle.degrees(300)

    var body: some View {
        GeometryReader { proxy in
            let size = min(proxy.size.width, proxy.size.height)
            let line = size * 0.11
            // Room for the arrowhead, which is wider than the line.
            let radius = size / 2 - line * 1.4
            let center = CGPoint(x: proxy.size.width / 2, y: proxy.size.height / 2)
            // One path, filled once, so a translucent colour does not
            // darken where the line and the head overlap.
            Path { path in
                path.addPath(Self.arc(center: center, radius: radius)
                    .strokedPath(StrokeStyle(lineWidth: line, lineCap: .round)))
                path.addPath(Self.head(center: center, radius: radius, line: line))
            }
            .fill(style: FillStyle(eoFill: false))
        }
    }

    /// In a view, y points down, so a growing angle turns clockwise.
    private static func point(_ center: CGPoint, _ radius: CGFloat, _ angle: Angle) -> CGPoint {
        CGPoint(x: center.x + radius * cos(angle.radians), y: center.y + radius * sin(angle.radians))
    }

    private static func arc(center: CGPoint, radius: CGFloat) -> Path {
        Path { path in
            let steps = 60
            for step in 0...steps {
                let angle = start + sweep * (Double(step) / Double(steps))
                let p = point(center, radius, angle)
                if step == 0 { path.move(to: p) } else { path.addLine(to: p) }
            }
        }
    }

    /// A triangle at the arc's end, pointing the way the arrow turns.
    private static func head(center: CGPoint, radius: CGFloat, line: CGFloat) -> Path {
        let end = start + sweep
        let base = point(center, radius, end)
        let tangent = CGPoint(x: -sin(end.radians), y: cos(end.radians))
        let outward = CGPoint(x: cos(end.radians), y: sin(end.radians))
        let length = line * 2.6
        let half = line * 1.4
        return Path { path in
            path.move(to: CGPoint(x: base.x + tangent.x * length, y: base.y + tangent.y * length))
            path.addLine(to: CGPoint(x: base.x + outward.x * half, y: base.y + outward.y * half))
            path.addLine(to: CGPoint(x: base.x - outward.x * half, y: base.y - outward.y * half))
            path.closeSubpath()
        }
    }
}

/// Watches the scroll view that holds it: the fingers on the trackpad from
/// the scroll events, which it keeps from the scroll view while they pull.
struct PullToRefreshMonitor: NSViewRepresentable {
    @Binding var gesture: PullGesture
    let enabled: Bool
    let reload: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator() }

    func makeNSView(context: Context) -> NSView {
        let view = MonitorView()
        view.coordinator = context.coordinator
        context.coordinator.attach(to: view)
        return view
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.parent = self
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        coordinator.detach()
    }

    /// Finds the scroll view as soon as it is in a window.
    final class MonitorView: NSView {
        weak var coordinator: Coordinator?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if window != nil { _ = coordinator?.resolveScrollView() }
        }
    }

    @MainActor
    final class Coordinator {
        var parent: PullToRefreshMonitor?
        private weak var view: NSView?
        private(set) weak var scrollView: NSScrollView?
        private var monitor: Any?
        /// The momentum after a pull is the pull's too: the strip stays put.
        private var swallowingMomentum = false

        func attach(to view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                guard let self else { return event }
                return MainActor.assumeIsolated { self.handle(event) } ? nil : event
            }
        }

        func detach() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            monitor = nil
        }

        /// The scroll view this monitor sits in, found once it is on screen.
        func resolveScrollView() -> NSScrollView? {
            if let scrollView { return scrollView }
            scrollView = view?.enclosingScrollView
            return scrollView
        }

        /// How far the strip is scrolled from its first card. The clip view's
        /// origin alone is not that: with the sidebar floating over the
        /// strip, the scroll view runs under it and insets its content by the
        /// sidebar's width, so the origin rests at minus that width.
        func offset(of scrollView: NSScrollView) -> CGFloat {
            let clip = scrollView.contentView
            return clip.bounds.origin.x + clip.contentInsets.left
        }

        /// True when the event belongs to the pull and must not scroll the strip.
        private func handle(_ event: NSEvent) -> Bool {
            guard let parent, let scrollView = resolveScrollView(),
                  event.window === scrollView.window
            else { return false }

            if !event.momentumPhase.isEmpty {
                let swallowed = swallowingMomentum
                if event.momentumPhase.contains(.ended) || event.momentumPhase.contains(.cancelled) {
                    swallowingMomentum = false
                }
                return swallowed
            }
            // A mouse wheel carries no finger: never a pull.
            guard !event.phase.isEmpty else { return false }

            var gesture = parent.gesture
            if event.phase.contains(.began) {
                swallowingMomentum = false
                let point = scrollView.convert(event.locationInWindow, from: nil)
                if parent.enabled, scrollView.bounds.contains(point) {
                    gesture.began(offset: offset(of: scrollView))
                    _ = gesture.moved(dx: event.scrollingDeltaX, dy: event.scrollingDeltaY)
                } else {
                    gesture.cancelled()
                }
                update(gesture, from: parent)
                // The scroll view still sees the start, so it is not left
                // waiting for a gesture that never began.
                return false
            }
            if event.phase.contains(.changed) {
                let taken = gesture.moved(dx: event.scrollingDeltaX, dy: event.scrollingDeltaY)
                update(gesture, from: parent)
                return taken
            }
            if event.phase.contains(.ended) || event.phase.contains(.cancelled) {
                guard gesture.phase != .idle else { return false }
                swallowingMomentum = gesture.pulling
                let reload = event.phase.contains(.ended) ? gesture.ended() : false
                gesture.cancelled()
                withAnimation(.spring(duration: 0.35)) {
                    parent.gesture = gesture
                }
                if reload && parent.enabled { parent.reload() }
                // The scroll view sees the end of what it saw begin.
                return false
            }
            return false
        }

        private func update(_ gesture: PullGesture, from parent: PullToRefreshMonitor) {
            let before = parent.gesture
            guard gesture != before else { return }
            if gesture.armed != before.armed {
                // A tick under the fingers where letting go starts to count, and
                // a lighter one where it stops counting again.
                NSHapticFeedbackManager.defaultPerformer.perform(
                    gesture.armed ? .levelChange : .alignment, performanceTime: .now
                )
            }
            parent.gesture = gesture
        }
    }
}
