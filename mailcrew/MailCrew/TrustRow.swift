import AppKit
import SwiftUI
import WebKit

/// The row under a card's subject: Gmail's SPF, DKIM and DMARC, then Mail
/// Crew's BIMI and mark certificate checks. Each is a word with a dot; the
/// dot's colour says how it went and the tooltip says the rest. The row keeps
/// its height before the body comes in.
struct TrustRow: View {
    let items: [TrustItem]

    var body: some View {
        HStack(spacing: 12) {
            ForEach(items, id: \.key) { item in
                TrustBadge(item: item)
            }
        }
        .frame(height: 14, alignment: .leading)
        .accessibilityElement(children: .contain)
        .accessibilityLabel("送信元の検証")
    }
}

private struct TrustBadge: View {
    let item: TrustItem

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(dot)
                .frame(width: 6, height: 6)
            Text(item.label)
                .font(.system(size: 10, weight: .semibold))
                .tracking(0.6)
        }
        .foregroundStyle(item.tone == .fail ? TrustColor.danger : TrustColor.muted)
        .help(item.detail)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(item.detail)
    }

    /// As the Tauri app drew it: quiet unless the check passed or failed.
    private var dot: Color {
        switch item.tone {
        case .pass: TrustColor.pass
        case .fail: TrustColor.danger
        case .neutral: TrustColor.muted.opacity(0.35)
        case .pending: TrustColor.muted.opacity(0.2)
        }
    }
}

/// The colours the Tauri app used for passed, failed and no result, light and dark.
private enum TrustColor {
    static let pass = adaptive(light: 0x248A3D, dark: 0x30D158)
    static let danger = adaptive(light: 0xD70015, dark: 0xFF453A)
    static let muted = adaptive(light: 0x86868B, dark: 0x98989D)

    private static func adaptive(light: UInt32, dark: UInt32) -> Color {
        Color(nsColor: NSColor(name: nil) { appearance in
            let hex = appearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua ? dark : light
            return NSColor(
                srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
                green: CGFloat((hex >> 8) & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255,
                alpha: 1
            )
        })
    }
}

/// The sender's BIMI logo on a white tile, as BIMI logos are drawn for one.
struct BrandLogoView: View {
    let logo: BrandLogo

    var body: some View {
        LogoWebView(url: logo.url)
            .frame(width: 20, height: 20)
            .clipShape(RoundedRectangle(cornerRadius: 5))
            .overlay {
                RoundedRectangle(cornerRadius: 5)
                    .strokeBorder(Color.primary.opacity(0.1), lineWidth: 0.5)
            }
            // Over the web view, which takes no mouse events, so that the tooltip shows.
            .overlay { Color.clear.contentShape(Rectangle()).help(logo.title) }
            .accessibilityElement()
            .accessibilityLabel(logo.title)
    }
}

/// Draws an SVG logo. AppKit's documentation does not list SVG among the
/// formats `NSImage(data:)` reads, so WebKit draws it, under the same locks
/// as a message: no JavaScript, and a content rule list and a CSP that let
/// nothing but the document and `data:` load (`LogoFrame`).
private struct LogoWebView: NSViewRepresentable {
    let url: String

    func makeNSView(context: Context) -> LogoFrameView { LogoFrameView() }

    func updateNSView(_ view: LogoFrameView, context: Context) { view.show(url) }

    static func dismantleNSView(_ view: LogoFrameView, coordinator: ()) { view.tearDown() }
}

final class LogoFrameView: NSView, WKNavigationDelegate {
    private let webView: WKWebView
    private var shown: String?

    init() {
        let configuration = WKWebViewConfiguration()
        configuration.defaultWebpagePreferences.allowsContentJavaScript = false
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.websiteDataStore = .nonPersistent()
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init(frame: .zero)
        webView.allowsLinkPreview = false
        webView.allowsMagnification = false
        webView.navigationDelegate = self
        addSubview(webView)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not used") }

    override func layout() {
        super.layout()
        webView.frame = bounds
    }

    /// The logo is a picture: clicks and scrolls go to what is under it.
    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    func show(_ url: String) {
        guard url != shown else { return }
        shown = url
        Task { @MainActor in
            // Without the rules nothing is shown; the CSP alone is not relied on.
            guard let rules = try? await MessageRules.list(allowRemoteImages: false), shown == url else { return }
            let controller = webView.configuration.userContentController
            controller.removeAllContentRuleLists()
            controller.add(rules)
            webView.loadHTMLString(LogoFrame.document(logo: url), baseURL: nil)
        }
    }

    func tearDown() {
        webView.stopLoading()
        webView.navigationDelegate = nil
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor action: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        decisionHandler(LogoFrame.allows(action.request.url) ? .allow : .cancel)
    }
}

/// The document a logo is drawn in, apart from the view so that the tests can hold it.
enum LogoFrame {
    /// Only the logo itself, as a `data:` URL, and the document's own style.
    static let policy = "default-src 'none'; img-src data:; style-src 'unsafe-inline'"

    static func document(logo: String) -> String {
        [
            "<!doctype html><html><head><meta charset=\"utf-8\">",
            "<meta http-equiv=\"Content-Security-Policy\" content=\"\(policy)\">",
            "<meta name=\"referrer\" content=\"no-referrer\">",
            "<style>html, body { margin: 0; width: 100%; height: 100%; background: #fff; overflow: hidden; }",
            " img { display: block; width: 100%; height: 100%; object-fit: contain; }</style>",
            "</head><body><img alt=\"\" src=\"\(escapeAttribute(logo))\"></body></html>",
        ].joined()
    }

    /// Only the app's own document loads; the logo cannot navigate anywhere.
    static func allows(_ url: URL?) -> Bool {
        url?.absoluteString == "about:blank"
    }

    private static func escapeAttribute(_ value: String) -> String {
        value
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "<", with: "&lt;")
    }
}
