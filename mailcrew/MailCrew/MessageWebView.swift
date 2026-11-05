import AppKit
import SwiftUI
import WebKit

/// A message's sanitised HTML, under the rules in `MessageFrame`.
///
/// The view never scrolls: it is made as tall as the message, and the card
/// body around it does the scrolling, so a sideways swipe over a card always
/// reaches the strip. A message laid out wider than the card is scaled down to
/// fit rather than left to overflow.
struct MessageWebView: NSViewRepresentable {
    let html: String
    let allowRemoteImages: Bool
    /// Hands a clicked link on; only links `MessageFrame.decide` lets out get here.
    let openLink: (URL) -> Void
    @Binding var height: CGFloat

    func makeNSView(context: Context) -> MessageFrameView {
        MessageFrameView(openLink: openLink) { height = $0 }
    }

    func updateNSView(_ view: MessageFrameView, context: Context) {
        view.openLink = openLink
        view.onHeight = { height = $0 }
        view.show(html: html, allowRemoteImages: allowRemoteImages)
    }

    static func dismantleNSView(_ view: MessageFrameView, coordinator: ()) {
        view.tearDown()
    }
}

/// Holds the web view and keeps it as wide as the card and as tall as the message.
final class MessageFrameView: NSView, WKNavigationDelegate, WKUIDelegate {
    var openLink: (URL) -> Void
    var onHeight: (CGFloat) -> Void

    private let webView: MessageWKWebView
    /// What was last handed to the web view.
    private var shown: (html: String, allowRemoteImages: Bool)?
    private var loaded = false
    private var fittedWidth: CGFloat = 0
    /// Bumped on every load, so that a rule list or a measurement that comes
    /// back for an older one is dropped.
    private var generation = 0

    /// The app's own scripts run here, out of the message's reach.
    private static let world = WKContentWorld.world(name: "MailCrewFit")

    init(openLink: @escaping (URL) -> Void, onHeight: @escaping (CGFloat) -> Void) {
        self.openLink = openLink
        self.onHeight = onHeight
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        // Nothing an allowed image sets, such as a cookie, outlives the app.
        configuration.websiteDataStore = .nonPersistent()
        configuration.mediaTypesRequiringUserActionForPlayback = .all
        webView = MessageWKWebView(frame: .zero, configuration: configuration)
        super.init(frame: .zero)
        // A force click would otherwise load the link in a preview window.
        webView.allowsLinkPreview = false
        webView.allowsBackForwardNavigationGestures = false
        webView.allowsMagnification = false
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.setAccessibilityLabel("メール本文")
        addSubview(webView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override var isFlipped: Bool { true }

    override func layout() {
        super.layout()
        webView.frame.size.width = bounds.width
        if loaded && abs(bounds.width - fittedWidth) > 0.5 { fit() }
    }

    func show(html: String, allowRemoteImages: Bool) {
        if let shown, shown.html == html, shown.allowRemoteImages == allowRemoteImages { return }
        shown = (html, allowRemoteImages)
        loaded = false
        generation += 1
        let current = generation
        Task { @MainActor in
            let rules: WKContentRuleList
            do {
                rules = try await MessageRules.list(allowRemoteImages: allowRemoteImages)
            } catch {
                // Without the rules nothing is shown; the CSP alone is not relied on.
                NSLog("mailcrew: could not compile the content rules: \(error)")
                return
            }
            guard current == generation else { return }
            let controller = webView.configuration.userContentController
            controller.removeAllContentRuleLists()
            controller.add(rules)
            webView.loadHTMLString(
                MessageFrame.document(html: html, allowRemoteImages: allowRemoteImages),
                baseURL: nil
            )
        }
    }

    func tearDown() {
        webView.stopLoading()
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
    }

    /// Scales the message to the width and makes the view as tall as it.
    private func fit() {
        let current = generation
        let width = bounds.width
        fittedWidth = width
        webView.callAsyncJavaScript(MessageFrame.fitScript, arguments: [:], in: nil, in: Self.world) { [weak self] result in
            guard let self, current == generation, case .success(let value) = result else { return }
            let height = max(1, CGFloat((value as? NSNumber)?.doubleValue ?? 0))
            webView.frame.size.height = height
            onHeight(height)
        }
    }

    // MARK: WKNavigationDelegate

    func webView(
        _ webView: WKWebView,
        decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        switch MessageFrame.decide(
            url: action.request.url,
            linkActivated: action.navigationType == .linkActivated,
            isOpenable: AppModel.isOpenableLink
        ) {
        case .allow:
            decisionHandler(.allow)
        case .openOutside(let url):
            decisionHandler(.cancel)
            openLink(url)
        case .refuse:
            decisionHandler(.cancel)
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        loaded = true
        fit()
    }

    // MARK: WKUIDelegate

    /// A link with a target opens no window; `decidePolicyFor` has already
    /// opened it outside.
    func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        nil
    }
}

/// A web view that never scrolls: every scroll goes on to the card body, or
/// through it to the strip.
final class MessageWKWebView: WKWebView {
    override func scrollWheel(with event: NSEvent) {
        nextResponder?.scrollWheel(with: event)
    }
}

/// The compiled `WKContentRuleList`s, one for each image setting.
@MainActor
enum MessageRules {
    private static var lists: [Bool: WKContentRuleList] = [:]

    static func list(allowRemoteImages: Bool) async throws -> WKContentRuleList {
        if let list = lists[allowRemoteImages] { return list }
        let identifier = allowRemoteImages ? "mailcrew-images-allowed" : "mailcrew-blocked"
        guard let store = WKContentRuleListStore.default() else {
            throw CocoaError(.featureUnsupported)
        }
        let list = try await store.compileContentRuleList(
            forIdentifier: identifier,
            encodedContentRuleList: MessageFrame.ruleList(allowRemoteImages: allowRemoteImages)
        )
        guard let list else { throw CocoaError(.featureUnsupported) }
        lists[allowRemoteImages] = list
        return list
    }
}
