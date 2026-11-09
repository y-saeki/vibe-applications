import HatenaFeed
import SwiftUI
import UIKit
import WebKit

/// Owns the web view, so that its page survives the view being rebuilt when the
/// available space changes, as it does when iPhone Duo opens or closes.
@MainActor
@Observable
final class ArticleBrowserModel: NSObject {
    let entry: FeedItem
    private(set) var pageURL: URL?
    private(set) var canGoBack = false
    private(set) var canGoForward = false
    private(set) var isLoading = false
    private(set) var progress = 0.0
    private(set) var loadError: Error?
    /// Where the page starts to show below what covers the web view's top.
    private(set) var pageTop: CGFloat = 0
    /// Whether the reader last scrolled down the page, which hides the bar
    /// along the bottom. Scrolling up, or reaching the top of the page,
    /// brings it back.
    private(set) var isScrollingDown = false
    @ObservationIgnored private var lastOffset: CGFloat = 0
    @ObservationIgnored private var travel: CGFloat = 0
    /// How far a finger moves the page one way before the bar follows, so
    /// that a page nudged back and forth while reading leaves it alone.
    private static let scrollThreshold: CGFloat = 24

    @ObservationIgnored private(set) lazy var webView = makeWebView()
    @ObservationIgnored private var observations: [NSKeyValueObservation] = []

    init(entry: FeedItem) {
        self.entry = entry
        self.pageURL = entry.link
    }

    func goBack() { webView.goBack() }
    func goForward() { webView.goForward() }

    func reload() {
        loadError = nil
        if webView.url == nil {
            webView.load(URLRequest(url: entry.link))
        } else {
            webView.reload()
        }
    }

    func extendsToBottomEdge(_ extends: Bool) {
        (webView as? ArticleWebView)?.extendsToBottomEdge = extends
    }

    private func makeWebView() -> WKWebView {
        // A page the list started when the finger rested on its row, or that
        // a long press previewed, carries on from where it got to.
        let preloaded = PagePreloader.shared.take(entry.link)
        let webView = preloaded ?? ArticleWebView()
        webView.onPageTopChange = { [weak self] top in self?.pageTop = top }
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.allowsBackForwardNavigationGestures = true
        observations = [
            webView.scrollView.observe(\.contentOffset, options: [.new]) { [weak self] scrollView, _ in
                MainActor.assumeIsolated { self?.didScroll(scrollView) }
            },
            observe(webView, \.url) { model, webView in model.pageURL = webView.url ?? model.pageURL },
            observe(webView, \.canGoBack) { model, webView in model.canGoBack = webView.canGoBack },
            observe(webView, \.canGoForward) { model, webView in model.canGoForward = webView.canGoForward },
            observe(webView, \.isLoading) { model, webView in model.isLoading = webView.isLoading },
            observe(webView, \.estimatedProgress) { model, webView in model.progress = webView.estimatedProgress },
        ]
        if preloaded == nil {
            webView.load(URLRequest(url: entry.link))
        }
        return webView
    }

    private func didScroll(_ scrollView: UIScrollView) {
        let offset = scrollView.contentOffset.y
        defer { lastOffset = offset }
        if offset <= -scrollView.adjustedContentInset.top {
            travel = 0
            isScrollingDown = false
            return
        }
        // Only the reader's own scrolling: a page that moves itself, or the
        // insets changing as the bar comes and goes, leaves the bar as it is.
        guard scrollView.isDragging || scrollView.isDecelerating else { return }
        let delta = offset - lastOffset
        // At the foot of the page the bounce pulls it back up.
        let bottom = scrollView.contentSize.height - scrollView.bounds.height + scrollView.adjustedContentInset.bottom
        guard offset < bottom else { return }
        travel = (travel >= 0) == (delta >= 0) ? travel + delta : delta
        if travel > Self.scrollThreshold {
            isScrollingDown = true
        } else if travel < -Self.scrollThreshold {
            isScrollingDown = false
        }
    }

    // WKWebView documents these key paths as observable and reports them on the main thread.
    private func observe<Value>(
        _ webView: WKWebView,
        _ keyPath: KeyPath<WKWebView, Value>,
        update: @escaping @MainActor (ArticleBrowserModel, WKWebView) -> Void
    ) -> NSKeyValueObservation {
        webView.observe(keyPath, options: [.initial]) { [weak self] webView, _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                update(self, webView)
            }
        }
    }
}

extension ArticleBrowserModel: WKNavigationDelegate {
    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        // A link to something other than a web page belongs to another app.
        if navigationAction.navigationType == .linkActivated,
           let url = navigationAction.request.url,
           let scheme = url.scheme?.lowercased(),
           !["http", "https"].contains(scheme) {
            UIApplication.shared.open(url)
            decisionHandler(.cancel)
            return
        }
        decisionHandler(.allow)
    }

    func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
        loadError = nil
        // A new page starts with the bar in view.
        isScrollingDown = false
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        record(error)
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        record(error)
    }

    private func record(_ error: Error) {
        // Following another link cancels the load in progress, which is not a failure.
        guard (error as? URLError)?.code != .cancelled else { return }
        FailureLog.record(error)
        loadError = error
    }
}

extension ArticleBrowserModel: WKUIDelegate {
    // A link that asks for a new window opens here: this browser has only one.
    func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        webView.load(navigationAction.request)
        return nil
    }
}

struct ArticleBrowserView: View {
    @State private var model: ArticleBrowserModel
    @State private var isShowingComments = false
    @State private var isShowingBookmarkEditor = false
    /// Whether the page is wider than it is tall, which with a vertical bar
    /// tells iPhone Duo's inner display held sideways from its outer display.
    @State private var isWide = false
    /// Whether the page has taken the whole screen, in place of the vertical
    /// bar, on iPhone Duo's inner display held sideways.
    @State private var isFullScreen = false
    /// Whether the reader has put the bars away, so that a page that fills
    /// its window, such as a web comic viewer enlarged in place, has the whole
    /// screen. Unlike a page's own full screen, WebKit plays no part: many
    /// viewers enlarge without asking for it.
    ///
    /// Only on iPhone Duo's inner display, where turning the screen or folding
    /// it is a natural way back. Elsewhere going back to the list would be the
    /// only one, and the wish to read a page without the bars is rare.
    @State private var hidesBars = false
    @State private var screen = ScreenState()
    @Environment(ReadHistoryStore.self) private var readHistory

    init(entry: FeedItem) {
        _model = State(initialValue: ArticleBrowserModel(entry: entry))
    }

    /// Where the bar's buttons go.
    private enum BarLayout {
        /// Along the bottom, going back included, where a thumb reaches them.
        case bottom
        /// In the vertical bar at the side: going back and the page arrows at
        /// its top, the rest at its foot.
        case side
        /// On iPhone Duo's inner display held sideways, the page takes the
        /// whole screen, so that its middle is the screen's: no status bar,
        /// no bar at the top, and the buttons along the bottom.
        case fullScreen
    }

    var body: some View {
        VerticalBarReader { barsMayBeVertical in
            browser(layout: isFullScreen ? .fullScreen : barsMayBeVertical ? .side : .bottom)
                .onChange(of: [barsMayBeVertical, isWide], initial: true) {
                    // Turning the vertical bar off also tells this screen there
                    // is none, so the full screen holds until the screen turns
                    // upright or folds, rather than handing back to the side.
                    if !isWide {
                        isFullScreen = false
                    } else if barsMayBeVertical {
                        isFullScreen = true
                    }
                }
        }
    }

    private func browser(layout: BarLayout) -> some View {
        // The toolbar goes through the navigation container, not a hand-made bar,
        // because that is what lets iPhone Duo lay it out vertically and leave the
        // height to the page. There is no title for the same reason: on iPhone Duo
        // it would take a bar of its own, and the page shows its title anyway.
        // Edge to edge, so that the scroll indicator runs along the screen's
        // edge past the vertical bar, as Safari's does. The page itself keeps
        // clear of the bar through the insets the web view gives WebKit.
        WebViewContainer(webView: model.webView)
            .ignoresSafeArea()
            .onGeometryChange(for: Bool.self) { $0.size.width > $0.size.height } action: { isWide = $0 }
            .background(EdgeOnlyPopGesture())
            .overlay(alignment: .top) {
                // At the top of the page as it shows, not of the safe area: on
                // iPhone Duo the page also runs under the empty bar's slot.
                // Across the whole screen, past the page's side safe areas.
                if model.isLoading {
                    ProgressView(value: model.progress).progressViewStyle(.linear)
                        .padding(.top, model.pageTop)
                        .ignoresSafeArea(edges: [.top, .horizontal])
                }
            }
            .overlay {
                if let error = model.loadError {
                    ContentUnavailableView {
                        Label("記事を読み込めませんでした", systemImage: "wifi.exclamationmark")
                    } description: {
                        Text(error.localizedDescription)
                    } actions: {
                        Button("再読み込み") { model.reload() }
                            .buttonStyle(.bordered)
                    }
                    .background(.background)
                }
            }
            .navigationBarTitleDisplayMode(.inline)
            // The entry, not the page shown, so that following links inside the
            // article still marks the row it was opened from.
            .onAppear { readHistory.markRead(model.entry.link) }
            .background(ScreenReader { state in
                // Turning the screen or folding it brings the bars back, the way
                // the bars come and go with the screen's layout anyway.
                if !state.isUnfolded || (screen.windowSize != nil && state.windowSize != screen.windowSize) {
                    hidesBars = false
                }
                screen = state
            })
            .onChange(of: hidesBars, initial: true) { model.extendsToBottomEdge(hidesBars) }
            .backButtonMoved(layout != .side)
            .verticalBarDisabled(layout == .fullScreen || hidesBars)
            .toolbar(layout == .fullScreen || hidesBars ? .hidden : .automatic, for: .navigationBar)
            // A bar along the bottom gets out of the way while the reader
            // scrolls down, uncovering the foot of the page and anything the
            // page keeps there. The vertical bar takes no height from the page.
            .toolbar(
                hidesBars || (layout != .side && model.isScrollingDown) ? .hidden : .automatic,
                for: .bottomBar
            )
            .animation(.default, value: model.isScrollingDown)
            .animation(.default, value: hidesBars)
            .statusBarHidden(layout == .fullScreen || hidesBars)
            .toolbar {
                switch layout {
                case .bottom, .fullScreen:
                    if #available(iOS 26.0, *) {
                        // Going back to the list is a glass of its own, apart
                        // from the page arrows, as the system's back button is.
                        ToolbarItem(placement: .bottomBar) {
                            BackButton()
                        }
                        ToolbarSpacer(.fixed, placement: .bottomBar)
                        ToolbarItemGroup(placement: .bottomBar) {
                            pageButtons
                        }
                        ToolbarSpacer(.flexible, placement: .bottomBar)
                        ToolbarItemGroup(placement: .bottomBar) {
                            entryButtons
                        }
                    } else {
                        ToolbarItemGroup(placement: .bottomBar) {
                            BackButton()
                            pageButtons
                            Spacer()
                            entryButtons
                        }
                    }
                case .side:
                    ToolbarItemGroup(placement: .topBarLeading) {
                        pageButtons
                    }
                    ToolbarItemGroup(placement: .bottomBar) {
                        entryButtons
                    }
                }
            }
            .sheet(isPresented: $isShowingComments) {
                NavigationStack {
                    EntryDetailView(entry: model.entry)
                        .toolbar {
                            ToolbarItem(placement: .cancellationAction) {
                                Button("閉じる", systemImage: "xmark") { isShowingComments = false }
                            }
                        }
                }
            }
            .sheet(isPresented: $isShowingBookmarkEditor) {
                NavigationStack {
                    // The entry, like the read mark: a link followed inside the
                    // article is not what the user is bookmarking from the list.
                    BookmarkEditorView(url: model.entry.link, title: model.entry.title)
                }
            }
    }

    @ViewBuilder
    private var pageButtons: some View {
        // Arrows, because the chevron is the button back to the list.
        Button("前のページ", systemImage: "arrow.backward") { model.goBack() }
            .disabled(!model.canGoBack)
        Button("次のページ", systemImage: "arrow.forward") { model.goForward() }
            .disabled(!model.canGoForward)
    }

    @ViewBuilder
    private var entryButtons: some View {
        Button("ブコメを見る", systemImage: "text.bubble") { isShowingComments = true }
        Button("ブクマする", systemImage: "bookmark") { isShowingBookmarkEditor = true }
        ShareLink(item: model.pageURL ?? model.entry.link) {
            Label("共有", systemImage: "square.and.arrow.up")
        }
        // The inner display is wide enough for one more button along the
        // bottom, upright or sideways; an iPhone's width is not.
        if screen.isUnfolded {
            Button("全画面", systemImage: "arrow.up.left.and.arrow.down.right") { hidesBars = true }
        }
    }
}

/// What the browser needs to know about the screen it is on.
private struct ScreenState: Equatable {
    var windowSize: CGSize?
    /// Whether iPhone Duo is open, so that the page is on its inner display.
    var isUnfolded = false
}

/// Reports the window's size and the state of iPhone Duo's hinge. The window,
/// not the page, because putting the bars away changes the page's size too.
/// The hinge, not the device's model, so that the inner display is told by
/// the context the page is in, as Apple's iPhone Duo guidance asks; other
/// iPhones have none, and count as folded.
private struct ScreenReader: UIViewRepresentable {
    let onChange: (ScreenState) -> Void

    func makeUIView(context: Context) -> Probe { Probe(onChange: onChange) }
    func updateUIView(_ uiView: Probe, context: Context) { uiView.onChange = onChange }

    final class Probe: UIView {
        var onChange: (ScreenState) -> Void
        private var state = ScreenState()

        init(onChange: @escaping (ScreenState) -> Void) {
            self.onChange = onChange
            super.init(frame: .zero)
            isUserInteractionEnabled = false
            if #available(iOS 27.1, *) {
                addInteraction(UIHingeInteraction { [weak self] _, update in
                    let status = update.hinge?.status
                    self?.change { $0.isUnfolded = status == .partiallyOpen || status == .fullyOpen }
                })
            }
        }

        @available(*, unavailable)
        required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

        override func didMoveToWindow() {
            super.didMoveToWindow()
            reportWindowSize()
        }

        override func layoutSubviews() {
            super.layoutSubviews()
            reportWindowSize()
        }

        private func reportWindowSize() {
            guard let window else { return }
            change { $0.windowSize = window.bounds.size }
        }

        private func change(_ update: (inout ScreenState) -> Void) {
            var next = state
            update(&next)
            guard next != state else { return }
            state = next
            // Not during layout, which SwiftUI may be in the middle of.
            DispatchQueue.main.async { [onChange] in onChange(next) }
        }
    }
}

private struct WebViewContainer: UIViewRepresentable {
    let webView: WKWebView

    func makeUIView(context: Context) -> WKWebView { webView }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}

/// Keeps pages clear of the top only where something covers them.
///
/// On iPhone Duo the navigation bar moves to the side column but leaves an
/// empty, zero-height bar at the top, and its slot still counts as safe area.
/// WebKit kept pages without `viewport-fit=cover` below that safe area, so
/// their fixed headers stood clear of the top while the article scrolled into
/// the gap above them. Here the top inset is only what really covers the page:
/// the screen's own safe area and a navigation bar that has a height.
final class ArticleWebView: WKWebView {
    init() {
        let configuration = WKWebViewConfiguration()
        // Lets a page's own full-screen button, such as a web comic viewer's,
        // take the whole screen. WebKit moves the web view into a window of its
        // own, over the bars, and back again on leaving. Without it the page
        // only fills the web view's frame, under the bars that cover it.
        configuration.preferences.isElementFullscreenEnabled = true
        super.init(frame: .zero, configuration: configuration)
        if #available(iOS 26.0, *) {
            // WebKit places fixed headers by the obscured insets, which it
            // takes from the scroll view's automatic insets unless they are set.
            scrollView.contentInsetAdjustmentBehavior = .never
            scrollView.automaticallyAdjustsScrollIndicatorInsets = false
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) has not been implemented") }

    // What pages with `viewport-fit=cover` read as the safe area, so that they
    // do not leave the empty bar's slot either.
    override var safeAreaInsets: UIEdgeInsets {
        var insets = super.safeAreaInsets
        if #available(iOS 26.0, *) {
            insets.top = min(insets.top, coveredTop)
            if extendsToBottomEdge {
                insets.bottom = 0
            }
        }
        return insets
    }

    /// Lets the page run to the screen's bottom edge, under the home
    /// indicator, while the bars are away. Kept clear of it, a page that
    /// fills its window, such as a web comic viewer, ended short of the edge,
    /// with a strip of the page's background below it.
    var extendsToBottomEdge = false {
        didSet {
            guard extendsToBottomEdge != oldValue else { return }
            setNeedsLayout()
        }
    }

    /// Called with the top of the part of the page nothing covers, whenever it moves.
    var onPageTopChange: ((CGFloat) -> Void)?
    private var reportedPageTop: CGFloat?
    private weak var observedBar: UINavigationBar?
    private var barObservation: NSKeyValueObservation?

    override func didMoveToWindow() {
        super.didMoveToWindow()
        setNeedsLayout()
    }

    override func layoutSubviews() {
        super.layoutSubviews()
        observeNavigationBar()
        let safeArea = safeAreaInsets
        if reportedPageTop != safeArea.top {
            reportedPageTop = safeArea.top
            // Not during layout, which SwiftUI may be in the middle of.
            DispatchQueue.main.async { [weak self, top = safeArea.top] in
                self?.onPageTopChange?(top)
            }
        }
        guard #available(iOS 26.0, *) else { return }
        // The bar's height only, not the home indicator's: the scroll view
        // lets the end of the page go past where WebKit keeps a fixed footer,
        // which then rose off the bottom edge with a strip of page below it.
        let insets = UIEdgeInsets(top: safeArea.top, left: safeArea.left, bottom: coveredBottom, right: safeArea.right)
        // At the screen's edge, as Safari puts it, past the vertical bar.
        let indicatorInsets = UIEdgeInsets(top: safeArea.top, left: 0, bottom: safeArea.bottom, right: 0)
        guard scrollView.contentInset != insets || scrollView.verticalScrollIndicatorInsets != indicatorInsets else { return }
        let wasAtTop = scrollView.contentOffset.y <= -scrollView.contentInset.top
        let wasAtLeading = scrollView.contentOffset.x <= -scrollView.contentInset.left
        // Not the bottom. Given a bottom inset, WebKit painted it over with
        // the colour of a page's fixed footer, such as an empty ad slot, so the
        // page stopped short of the bottom bar instead of running under it.
        obscuredContentInsets = UIEdgeInsets(top: insets.top, left: insets.left, bottom: 0, right: insets.right)
        scrollView.contentInset = insets
        scrollView.verticalScrollIndicatorInsets = indicatorInsets
        if wasAtTop {
            scrollView.contentOffset.y = -insets.top
        }
        if wasAtLeading {
            scrollView.contentOffset.x = -insets.left
        }
    }

    /// Lays the page out again when the navigation bar changes size. On iPhone
    /// Duo the bar shrinks to zero height after the web view takes its safe
    /// area, and the safe area itself stays the same, so nothing else would:
    /// the page kept the bar's old height above it until it reloaded.
    private func observeNavigationBar() {
        let bar = navigationBar
        guard bar !== observedBar else { return }
        observedBar = bar
        // The layer, because UIKit sizes views through it and the layer
        // reports its changes to observers.
        barObservation = bar?.layer.observe(\.bounds) { [weak self] _, _ in
            MainActor.assumeIsolated { self?.setNeedsLayout() }
        }
    }

    private var coveredTop: CGFloat {
        guard let window else { return super.safeAreaInsets.top }
        let screenTop = window.safeAreaInsets.top - convert(CGPoint.zero, to: window).y
        var top = max(0, screenTop)
        if let bar = navigationBar, !bar.isHidden, bar.bounds.height > 0 {
            top = max(top, bar.convert(bar.bounds, to: self).maxY)
        }
        return top
    }

    /// How far the bottom bar reaches above the screen's own safe area, so
    /// that the end of the page can scroll clear of the bar.
    private var coveredBottom: CGFloat {
        let bottom = safeAreaInsets.bottom
        guard let window else { return bottom }
        let screenBottom = convert(CGPoint(x: 0, y: bounds.maxY), to: window).y
            - (window.bounds.height - window.safeAreaInsets.bottom)
        return max(0, bottom - max(0, screenBottom))
    }

    private var navigationBar: UINavigationBar? {
        sequence(first: next, next: { $0?.next })
            .lazy
            .compactMap { ($0 as? UIViewController)?.navigationController }
            .first?
            .navigationBar
    }
}

/// Leaves going back to the list to a swipe from the screen's leading edge.
/// Since iOS 26 a navigation stack also goes back on a swipe that starts
/// anywhere, which takes the swipes of pages that use them themselves, such as
/// maps and carousels.
private struct EdgeOnlyPopGesture: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> Controller { Controller() }
    func updateUIViewController(_ uiViewController: Controller, context: Context) {}

    final class Controller: UIViewController {
        override func loadView() {
            view = UIView()
            view.isUserInteractionEnabled = false
        }

        override func viewWillAppear(_ animated: Bool) {
            super.viewWillAppear(animated)
            setContentPopEnabled(false)
        }

        override func viewDidAppear(_ animated: Bool) {
            super.viewDidAppear(animated)
            // The navigation controller may be reachable only once on screen.
            setContentPopEnabled(false)
        }

        override func viewWillDisappear(_ animated: Bool) {
            super.viewWillDisappear(animated)
            // The rest of the stack keeps the system's behaviour.
            setContentPopEnabled(true)
        }

        private func setContentPopEnabled(_ isEnabled: Bool) {
            guard #available(iOS 26.0, *) else { return }
            navigationController?.interactiveContentPopGestureRecognizer?.isEnabled = isEnabled
        }
    }
}
