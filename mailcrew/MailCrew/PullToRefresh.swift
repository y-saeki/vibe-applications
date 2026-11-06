import AppKit
import SwiftUI

/// Pulling the strip right past its first card, then letting go, reloads it.
///
/// Only a pull with a finger counts: the gesture must start while the strip
/// rests at its first card, and the finger must leave the trackpad while the
/// strip is pulled at least `threshold` past the edge. Momentum that carries
/// the strip back from older mail past the edge only bounces, and so does a
/// mouse wheel. The bounce itself is the scroll view's own (#298).
struct PullGesture: Equatable {
    /// How far past the first card the strip must be pulled.
    static let threshold: CGFloat = 72

    /// The gesture started at the first card, so it may reload.
    private(set) var tracking = false
    /// How far past the first card the strip is pulled now.
    private(set) var distance: CGFloat = 0

    /// Letting go now reloads.
    var armed: Bool { tracking && distance >= Self.threshold }
    /// From 0 to 1 as the pull nears the threshold.
    var progress: CGFloat { tracking ? min(1, distance / Self.threshold) : 0 }

    /// Fingers started moving on the trackpad, with the strip scrolled
    /// `offset` from its first card.
    mutating func began(offset: CGFloat) {
        tracking = offset <= 1
        distance = tracking ? max(0, -offset) : 0
    }

    /// The strip scrolled to `offset`; past the first card it is negative.
    mutating func scrolled(offset: CGFloat) {
        guard tracking else { return }
        distance = max(0, -offset)
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

/// What appears in the gap the pull opens left of the first card.
struct PullIndicator: View {
    let gesture: PullGesture

    var body: some View {
        VStack(spacing: 6) {
            Image(systemName: "arrow.clockwise")
                .font(.title3)
                .rotationEffect(.degrees(gesture.progress * 270))
            Text(gesture.armed ? "離すと再読み込み" : "引っ張って再読み込み")
                .font(.caption2)
                .multilineTextAlignment(.center)
        }
        .foregroundStyle(gesture.armed ? .primary : .secondary)
        .opacity(gesture.progress)
        .frame(width: PullGesture.threshold - 8)
        .frame(width: gesture.distance)
        .frame(maxHeight: .infinity)
        .clipped()
        .accessibilityHidden(true)
    }
}

/// Watches the scroll view that holds it: fingers on the trackpad from the
/// scroll events, and how far the strip is pulled from its clip view.
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

    /// Finds the scroll view as soon as it is in a window, so the clip
    /// view's movements are followed from the first gesture on.
    final class MonitorView: NSView {
        weak var coordinator: Coordinator?

        override func viewDidMoveToWindow() {
            super.viewDidMoveToWindow()
            if window != nil { _ = coordinator?.resolveScrollView() }
        }
    }

    func updateNSView(_ view: NSView, context: Context) {
        context.coordinator.parent = self
    }

    static func dismantleNSView(_ view: NSView, coordinator: Coordinator) {
        coordinator.detach()
    }

    @MainActor
    final class Coordinator {
        var parent: PullToRefreshMonitor?
        private weak var view: NSView?
        private(set) weak var scrollView: NSScrollView?
        private var monitor: Any?
        private var boundsObserver: NSObjectProtocol?

        func attach(to view: NSView) {
            self.view = view
            monitor = NSEvent.addLocalMonitorForEvents(matching: .scrollWheel) { [weak self] event in
                MainActor.assumeIsolated { self?.handle(event) }
                return event
            }
        }

        func detach() {
            if let monitor { NSEvent.removeMonitor(monitor) }
            if let boundsObserver { NotificationCenter.default.removeObserver(boundsObserver) }
            monitor = nil
            boundsObserver = nil
        }

        /// The scroll view this monitor sits in, found once it is on screen.
        func resolveScrollView() -> NSScrollView? {
            if let scrollView { return scrollView }
            guard let found = view?.enclosingScrollView else { return nil }
            scrollView = found
            let clip = found.contentView
            clip.postsBoundsChangedNotifications = true
            boundsObserver = NotificationCenter.default.addObserver(
                forName: NSView.boundsDidChangeNotification, object: clip, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated { self?.scrolled() }
            }
            return found
        }

        /// How far the strip is scrolled from its first card. The clip view's
        /// origin alone is not that: with the sidebar floating over the
        /// strip, the scroll view runs under it and insets its content by the
        /// sidebar's width, so the origin rests at minus that width.
        func offset(of scrollView: NSScrollView) -> CGFloat {
            let clip = scrollView.contentView
            return clip.bounds.origin.x + clip.contentInsets.left
        }

        private func handle(_ event: NSEvent) {
            guard let parent, let scrollView = resolveScrollView(),
                  event.window === scrollView.window
            else { return }
            // Momentum and mouse wheels carry no finger: never a pull.
            guard event.momentumPhase.isEmpty, !event.phase.isEmpty else { return }
            if event.phase.contains(.began) {
                let point = scrollView.convert(event.locationInWindow, from: nil)
                guard parent.enabled, scrollView.bounds.contains(point) else { return }
                var gesture = parent.gesture
                gesture.began(offset: offset(of: scrollView))
                parent.gesture = gesture
            } else if event.phase.contains(.ended) {
                guard parent.gesture.tracking else { return }
                var gesture = parent.gesture
                gesture.scrolled(offset: offset(of: scrollView))
                let reload = gesture.ended()
                parent.gesture = gesture
                if reload && parent.enabled { parent.reload() }
            } else if event.phase.contains(.cancelled) {
                guard parent.gesture.tracking else { return }
                var gesture = parent.gesture
                gesture.cancelled()
                parent.gesture = gesture
            }
        }

        private func scrolled() {
            guard let parent, parent.gesture.tracking, let scrollView else { return }
            var gesture = parent.gesture
            gesture.scrolled(offset: offset(of: scrollView))
            if gesture != parent.gesture { parent.gesture = gesture }
        }
    }
}
