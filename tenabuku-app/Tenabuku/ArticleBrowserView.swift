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
        let webView = WKWebView()
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
        webView.load(URLRequest(url: entry.link))
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
    @State private var isShowingBookmarkNotice = false

    init(entry: FeedItem) {
        _model = State(initialValue: ArticleBrowserModel(entry: entry))
    }

    var body: some View {
        // The toolbar goes through the navigation container, not a hand-made bar,
        // because that is what lets iPhone Duo lay it out vertically and leave the
        // height to the page. There is no title for the same reason: on iPhone Duo
        // it would take a bar of its own, and the page shows its title anyway.
        // Vertical only: left to WebKit, the sides depend on each page's viewport
        // and CSS, and many pages run under a toolbar on iPhone Duo's side.
        WebViewContainer(webView: model.webView)
            .ignoresSafeArea(edges: .vertical)
            .overlay(alignment: .top) {
                if model.isLoading {
                    ProgressView(value: model.progress).progressViewStyle(.linear)
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
            .toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    // Arrows, because the chevron is the system's button back to the list.
                    Button("前のページ", systemImage: "arrow.backward") { model.goBack() }
                        .disabled(!model.canGoBack)
                    Button("次のページ", systemImage: "arrow.forward") { model.goForward() }
                        .disabled(!model.canGoForward)
                    Spacer()
                    Button("ブコメを見る", systemImage: "text.bubble") { isShowingComments = true }
                    Button("ブクマする", systemImage: "bookmark") { isShowingBookmarkNotice = true }
                    ShareLink(item: model.pageURL ?? model.entry.link) {
                        Label("共有", systemImage: "square.and.arrow.up")
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
            .alert("ブックマークはまだ使えません", isPresented: $isShowingBookmarkNotice) {
                Button("OK", role: .cancel) {}
            } message: {
                Text("ログインとブックマークの機能を追加すると、ここからブックマークできるようになります。")
            }
    }
}

private struct WebViewContainer: UIViewRepresentable {
    let webView: WKWebView

    func makeUIView(context: Context) -> WKWebView { webView }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
}
