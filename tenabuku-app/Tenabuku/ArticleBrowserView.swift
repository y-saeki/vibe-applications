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
    /// Whether the buttons have gone to a bar along the top, in place of the
    /// vertical bar, on iPhone Duo's inner display held sideways.
    @State private var usesTopBar = false
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
        /// Along the top, on iPhone Duo's inner display held sideways, so that
        /// the page takes the whole width and its middle is the screen's.
        case top
    }

    var body: some View {
        VerticalBarReader { barsMayBeVertical in
            browser(layout: usesTopBar ? .top : barsMayBeVertical ? .side : .bottom)
                .onChange(of: [barsMayBeVertical, isWide], initial: true) {
                    // Turning the vertical bar off also tells this screen there
                    // is none, so the top bar holds until the screen turns
                    // upright or folds, rather than handing back to the side.
                    if !isWide {
                        usesTopBar = false
                    } else if barsMayBeVertical {
                        usesTopBar = true
                    }
                }
        }
    }

    private func browser(layout: BarLayout) -> some View {
        // The toolbar goes through the navigation container, not a hand-made bar,
        // because that is what lets iPhone Duo lay it out vertically and leave the
        // height to the page. There is no title for the same reason: on iPhone Duo
        // it would take a bar of its own, and the page shows its title anyway.
        // Vertical only: left to WebKit, the sides depend on each page's viewport
        // and CSS, and many pages run under a toolbar on iPhone Duo's side.
        WebViewContainer(webView: model.webView)
            .ignoresSafeArea(edges: .vertical)
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
            .backButtonMoved(layout == .bottom)
            .verticalBarDisabled(layout == .top)
            .toolbar {
                switch layout {
                case .bottom:
                    ToolbarItemGroup(placement: .bottomBar) {
                        BackButton()
                        pageButtons
                        Spacer()
                        entryButtons
                    }
                case .side:
                    ToolbarItemGroup(placement: .topBarLeading) {
                        pageButtons
                    }
                    ToolbarItemGroup(placement: .bottomBar) {
                        entryButtons
                    }
                case .top:
                    ToolbarItemGroup(placement: .topBarLeading) {
                        pageButtons
                    }
                    ToolbarItemGroup(placement: .topBarTrailing) {
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
                .presentationDetents([.medium, .large])
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
        super.init(frame: .zero, configuration: WKWebViewConfiguration())
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
        }
        return insets
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
        let insets = UIEdgeInsets(top: safeArea.top, left: 0, bottom: safeArea.bottom, right: 0)
        guard obscuredContentInsets != insets else { return }
        let wasAtTop = scrollView.contentOffset.y <= -scrollView.contentInset.top
        obscuredContentInsets = insets
        scrollView.contentInset = insets
        scrollView.verticalScrollIndicatorInsets = insets
        if wasAtTop {
            scrollView.contentOffset.y = -insets.top
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
