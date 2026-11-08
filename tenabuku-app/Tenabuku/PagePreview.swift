import HatenaFeed
import SwiftUI
import UIKit
import WebKit

/// One of the actions under a row's page preview.
struct PagePreviewAction {
    let title: String
    let systemImage: String
    let handler: () -> Void
}

/// Opens an entry's article at once, without the push animation, for a page
/// preview that has already grown to fill the screen.
struct OpenEntryFromPreviewAction: Equatable {
    private let handler: (FeedItem) -> Void

    init(_ handler: @escaping (FeedItem) -> Void) {
        self.handler = handler
    }

    func callAsFunction(_ entry: FeedItem) { handler(entry) }

    // The list sets it once, and what it captures is the list's state, which
    // stays the same however often the list is rebuilt: no view needs
    // updating when it is set again.
    static func == (lhs: Self, rhs: Self) -> Bool { true }
}

extension EnvironmentValues {
    @Entry var openEntryFromPreview = OpenEntryFromPreviewAction { _ in }
}

extension View {
    /// Shows the page at `url` and `actions` when the row is long pressed, the
    /// way App Store previews an app. Tapping the preview calls `open`, which
    /// shows the same page already loaded.
    ///
    /// SwiftUI's own `contextMenu(menuItems:preview:)` cannot tell when the
    /// preview is tapped, so this goes through `UIContextMenuInteraction`.
    func pagePreviewMenu(
        url: URL,
        title: String,
        actions: [PagePreviewAction],
        open: @escaping () -> Void
    ) -> some View {
        // In front of the row, because SwiftUI keeps the touches on the row's
        // content from reaching a view behind it. The view takes no touch of
        // its own, so a tap still reaches the row underneath.
        overlay(PagePreviewMenu(url: url, title: title, actions: actions, open: open))
    }
}

/// Holds the one page loaded ahead of time: the row a finger rests on.
///
/// A long press takes about half a second to become one. Starting the page
/// when the finger settles, rather than when the preview opens, gives it that
/// head start; a plain tap on the row then opens the same page, too, through
/// `ArticleBrowserModel`. Only one page is kept, because each web view holds
/// a web content process and its memory.
@MainActor
final class PagePreloader: NSObject {
    static let shared = PagePreloader()

    private var url: URL?
    private var webView: ArticleWebView?
    private var hasFailed = false

    /// The web view showing `url`, started now unless it already is.
    func start(_ url: URL) -> ArticleWebView {
        if let webView, self.url == url, !hasFailed {
            return webView
        }
        discard()
        let webView = ArticleWebView()
        webView.navigationDelegate = self
        webView.load(URLRequest(url: url))
        self.url = url
        self.webView = webView
        hasFailed = false
        return webView
    }

    /// Hands the web view over to the browser, which takes it from here.
    /// Nil when nothing is loading `url`, or its load failed: the browser
    /// then loads the page itself and reports what goes wrong.
    func take(_ url: URL) -> ArticleWebView? {
        guard self.url == url, let webView else { return nil }
        let failed = hasFailed
        self.webView = nil
        self.url = nil
        webView.navigationDelegate = nil
        if failed {
            webView.stopLoading()
            return nil
        }
        return webView
    }

    /// Stops the page for `url`, as when the finger turns out to scroll.
    func discard(_ url: URL) {
        if self.url == url { discard() }
    }

    private func discard() {
        webView?.stopLoading()
        webView?.navigationDelegate = nil
        webView = nil
        url = nil
        hasFailed = false
    }
}

extension PagePreloader: WKNavigationDelegate {
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        record(error, in: webView)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        record(error, in: webView)
    }

    private func record(_ error: Error, in webView: WKWebView) {
        guard webView === self.webView, (error as? URLError)?.code != .cancelled else { return }
        hasFailed = true
    }
}

private struct PagePreviewMenu: UIViewRepresentable {
    let url: URL
    let title: String
    let actions: [PagePreviewAction]
    let open: () -> Void

    func makeCoordinator() -> Coordinator { Coordinator(self) }

    func makeUIView(context: Context) -> UIView {
        let view = UIView()
        view.backgroundColor = .clear
        view.addInteraction(UIContextMenuInteraction(delegate: context.coordinator))
        let touch = RestingTouchRecognizer(
            onRest: { [coordinator = context.coordinator] in
                _ = PagePreloader.shared.start(coordinator.menu.url)
            },
            onMove: { [coordinator = context.coordinator] in
                PagePreloader.shared.discard(coordinator.menu.url)
            }
        )
        touch.delegate = context.coordinator
        view.addGestureRecognizer(touch)
        return view
    }

    func updateUIView(_ uiView: UIView, context: Context) {
        context.coordinator.menu = self
    }

    final class Coordinator: NSObject, UIContextMenuInteractionDelegate, UIGestureRecognizerDelegate {
        var menu: PagePreviewMenu

        init(_ menu: PagePreviewMenu) {
            self.menu = menu
        }

        func contextMenuInteraction(
            _ interaction: UIContextMenuInteraction,
            configurationForMenuAtLocation location: CGPoint
        ) -> UIContextMenuConfiguration? {
            let menu = menu
            return UIContextMenuConfiguration(
                identifier: nil,
                previewProvider: {
                    PagePreviewController(
                        webView: PagePreloader.shared.start(menu.url),
                        title: menu.title,
                        pageURL: menu.url
                    )
                },
                actionProvider: { _ in
                    UIMenu(children: menu.actions.map { action in
                        UIAction(title: action.title, image: UIImage(systemName: action.systemImage)) { _ in
                            action.handler()
                        }
                    })
                }
            )
        }

        func contextMenuInteraction(
            _ interaction: UIContextMenuInteraction,
            configuration: UIContextMenuConfiguration,
            highlightPreviewForItemWithIdentifier identifier: any NSCopying
        ) -> UITargetedPreview? {
            rowPreview(interaction)
        }

        func contextMenuInteraction(
            _ interaction: UIContextMenuInteraction,
            configuration: UIContextMenuConfiguration,
            dismissalPreviewForItemWithIdentifier identifier: any NSCopying
        ) -> UITargetedPreview? {
            rowPreview(interaction)
        }

        func contextMenuInteraction(
            _ interaction: UIContextMenuInteraction,
            willPerformPreviewActionForMenuWith configuration: UIContextMenuConfiguration,
            animator: any UIContextMenuInteractionCommitAnimating
        ) {
            // The preview grows to fill the screen and fades over the article,
            // which opens beneath it without sliding in.
            animator.preferredCommitStyle = .pop
            let open = menu.open
            let preview = animator.previewViewController as? PagePreviewController
            animator.addAnimations {
                preview?.freeze()
                open()
            }
        }

        /// Lifts the whole row rather than this view, which lies clear over
        /// the row's content and would show nothing of it.
        private func rowPreview(_ interaction: UIContextMenuInteraction) -> UITargetedPreview? {
            guard let view = interaction.view, let window = view.window else { return nil }
            let parameters = UIPreviewParameters()
            parameters.backgroundColor = .systemBackground
            // In a list, the cell itself.
            if let row = Self.enclosingCell(view) {
                parameters.visiblePath = UIBezierPath(roundedRect: row.bounds, cornerRadius: 16)
                return UITargetedPreview(view: row, parameters: parameters)
            }
            // On a home screen shelf there is no cell, and SwiftUI draws the
            // row in views of its own: lift a picture of what is on screen
            // under this view, which has the row's size.
            guard let snapshot = window.resizableSnapshotView(
                from: view.convert(view.bounds, to: window),
                afterScreenUpdates: false,
                withCapInsets: .zero
            ) else { return nil }
            parameters.visiblePath = UIBezierPath(roundedRect: snapshot.bounds, cornerRadius: 16)
            let target = UIPreviewTarget(container: view, center: CGPoint(x: view.bounds.midX, y: view.bounds.midY))
            return UITargetedPreview(view: snapshot, parameters: parameters, target: target)
        }

        private static func enclosingCell(_ view: UIView) -> UIView? {
            sequence(first: view, next: \.superview).first { $0 is UICollectionViewCell || $0 is UITableViewCell }
        }

        func gestureRecognizer(
            _ gestureRecognizer: UIGestureRecognizer,
            shouldRecognizeSimultaneouslyWith otherGestureRecognizer: UIGestureRecognizer
        ) -> Bool {
            true
        }
    }
}

/// Reports a finger that settles on the row, and one that then moves off it
/// to scroll. It never recognizes, so the row's tap, the list's scrolling and
/// the long press all carry on as if it were not there.
private final class RestingTouchRecognizer: UIGestureRecognizer {
    /// Long enough for a scroll to have moved the finger first.
    private static let restDelay: TimeInterval = 0.12
    private static let moveTolerance: CGFloat = 8

    private let onRest: () -> Void
    private let onMove: () -> Void
    private var start: CGPoint?
    private var pending: DispatchWorkItem?
    private var hasRested = false

    init(onRest: @escaping () -> Void, onMove: @escaping () -> Void) {
        self.onRest = onRest
        self.onMove = onMove
        super.init(target: nil, action: nil)
        cancelsTouchesInView = false
        delaysTouchesBegan = false
        delaysTouchesEnded = false
    }

    override func touchesBegan(_ touches: Set<UITouch>, with event: UIEvent) {
        guard touches.count == 1, let touch = touches.first, start == nil else {
            state = .failed
            return
        }
        start = touch.location(in: view)
        let work = DispatchWorkItem { [weak self] in
            guard let self else { return }
            hasRested = true
            onRest()
        }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.restDelay, execute: work)
    }

    override func touchesMoved(_ touches: Set<UITouch>, with event: UIEvent) {
        guard let start, let point = touches.first?.location(in: view) else { return }
        if hypot(point.x - start.x, point.y - start.y) > Self.moveTolerance {
            if hasRested { onMove() }
            state = .failed
        }
    }

    override func touchesEnded(_ touches: Set<UITouch>, with event: UIEvent) {
        state = .failed
    }

    override func touchesCancelled(_ touches: Set<UITouch>, with event: UIEvent) {
        // The long press cancels the touch when the menu opens, and keeps the page.
        state = .failed
    }

    override func reset() {
        super.reset()
        pending?.cancel()
        pending = nil
        start = nil
        hasRested = false
    }
}

/// The page under preview. Until the page has drawn something, the row's
/// site and title stand in for it above gray lines where the text will be,
/// so that the preview does not open on a blank sheet.
private final class PagePreviewController: UIViewController {
    private let webView: WKWebView
    private let pageTitle: String
    private let pageURL: URL
    private let placeholder = UIView()
    private let progressBar = UIProgressView(progressViewStyle: .bar)
    private var observations: [NSKeyValueObservation] = []

    init(webView: WKWebView, title: String, pageURL: URL) {
        self.webView = webView
        self.pageTitle = title
        self.pageURL = pageURL
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    override func loadView() {
        view = UIView()
        view.backgroundColor = .systemBackground

        webView.frame = view.bounds
        webView.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        view.addSubview(webView)

        placeholder.backgroundColor = .systemBackground
        placeholder.frame = view.bounds
        placeholder.autoresizingMask = [.flexibleWidth, .flexibleHeight]
        let stack = UIStackView(arrangedSubviews: [siteLine(), titleLabel(), skeleton()])
        stack.axis = .vertical
        stack.alignment = .fill
        stack.spacing = 10
        stack.setCustomSpacing(28, after: stack.arrangedSubviews[1])
        stack.translatesAutoresizingMaskIntoConstraints = false
        placeholder.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: placeholder.topAnchor, constant: 28),
            stack.leadingAnchor.constraint(equalTo: placeholder.leadingAnchor, constant: 20),
            stack.trailingAnchor.constraint(equalTo: placeholder.trailingAnchor, constant: -20),
        ])
        view.addSubview(placeholder)

        progressBar.frame = CGRect(x: 0, y: 0, width: view.bounds.width, height: 2)
        progressBar.autoresizingMask = [.flexibleWidth, .flexibleBottomMargin]
        view.addSubview(progressBar)

        observations = [
            webView.observe(\.estimatedProgress, options: [.initial]) { [weak self] _, _ in
                MainActor.assumeIsolated { self?.update() }
            },
            webView.observe(\.isLoading, options: [.initial]) { [weak self] _, _ in
                MainActor.assumeIsolated { self?.update() }
            },
        ]
    }

    /// The site's icon and host, as the list row shows them.
    private func siteLine() -> UIView {
        let hostLabel = UILabel()
        hostLabel.text = pageURL.host() ?? pageURL.absoluteString
        hostLabel.font = .preferredFont(forTextStyle: .footnote)
        hostLabel.textColor = .secondaryLabel
        let line = UIStackView(arrangedSubviews: [hostLabel])
        line.axis = .horizontal
        line.alignment = .center
        line.spacing = 6
        if case let icon?? = FaviconCache.shared.cached(pageURL) {
            let iconView = UIImageView(image: icon)
            iconView.contentMode = .scaleAspectFit
            iconView.layer.cornerRadius = 3
            iconView.clipsToBounds = true
            NSLayoutConstraint.activate([
                iconView.widthAnchor.constraint(equalToConstant: 16),
                iconView.heightAnchor.constraint(equalToConstant: 16),
            ])
            line.insertArrangedSubview(iconView, at: 0)
        }
        // Keeps the icon and host together at the leading edge.
        let row = UIStackView(arrangedSubviews: [line, UIView()])
        row.axis = .horizontal
        return row
    }

    private func titleLabel() -> UIView {
        let label = UILabel()
        label.text = pageTitle
        label.font = .preferredFont(forTextStyle: .title2).withTraits(.traitBold)
        label.numberOfLines = 0
        return label
    }

    /// Lines of text yet to come, pulsing gently while the page loads. The
    /// bar along the top already says how far along it is.
    private func skeleton() -> UIView {
        let lines = UIStackView()
        lines.axis = .vertical
        lines.alignment = .leading
        lines.spacing = 12
        for fraction in [1.0, 0.94, 1.0, 0.88, 0.62] as [CGFloat] {
            let bar = UIView()
            bar.backgroundColor = .tertiarySystemFill
            bar.layer.cornerRadius = 6
            bar.layer.cornerCurve = .continuous
            bar.translatesAutoresizingMaskIntoConstraints = false
            lines.addArrangedSubview(bar)
            NSLayoutConstraint.activate([
                bar.heightAnchor.constraint(equalToConstant: 12),
                bar.widthAnchor.constraint(equalTo: lines.widthAnchor, multiplier: fraction),
            ])
        }
        let pulse = CABasicAnimation(keyPath: "opacity")
        pulse.fromValue = 1
        pulse.toValue = 0.4
        pulse.duration = 0.9
        pulse.autoreverses = true
        pulse.repeatCount = .infinity
        pulse.timingFunction = CAMediaTimingFunction(name: .easeInEaseOut)
        lines.layer.add(pulse, forKey: "pulse")
        return lines
    }

    /// Lets the page show once it is far enough along to have drawn its top.
    private func update() {
        let progress = webView.estimatedProgress
        progressBar.setProgress(Float(progress), animated: true)
        progressBar.isHidden = !webView.isLoading
        let hasDrawn = !webView.isLoading || progress >= 0.5
        guard hasDrawn, placeholder.alpha > 0 else { return }
        UIView.animate(withDuration: 0.2) { self.placeholder.alpha = 0 }
    }

    /// Leaves a picture of the page behind while the web view itself moves
    /// on to the browser, so that the preview does not go blank as it fades.
    func freeze() {
        guard webView.superview === view, let picture = webView.snapshotView(afterScreenUpdates: false) else { return }
        picture.frame = webView.frame
        view.insertSubview(picture, aboveSubview: webView)
    }
}

private extension UIFont {
    func withTraits(_ traits: UIFontDescriptor.SymbolicTraits) -> UIFont {
        fontDescriptor.withSymbolicTraits(traits).map { UIFont(descriptor: $0, size: 0) } ?? self
    }
}
