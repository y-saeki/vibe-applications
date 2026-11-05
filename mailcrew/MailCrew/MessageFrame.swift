import Foundation

/// The rules a message's HTML is shown under, apart from the view that shows
/// it (`MessageWebView`), so that the tests can hold them.
///
/// This is the second of two layers; the first is `core/src/sanitize.rs`,
/// which strips anything that could run before the HTML reaches Swift.
///
/// - JavaScript from the message never runs: the web view is made with
///   `allowsContentJavaScript` off. Only the app's own measuring script runs,
///   in a content world of its own (`MessageWebView`).
/// - Nothing loads from the network until the user lets it in, and then only
///   images. Two locks hold this, either of which is enough: a
///   Content-Security-Policy in the document, and a `WKContentRuleList` that
///   blocks every load but the document itself and `data:` URLs.
/// - `no-referrer`, so even an allowed image does not say where it was seen.
/// - The view never navigates. Its own document is loaded with
///   `loadHTMLString`, which WebKit asks about as `about:blank`; every other
///   navigation is turned away, and a clicked link that passes
///   `isOpenableLink` (http, https, mailto) is opened by the OS instead.
enum MessageFrame {
    /// Inline styles every message starts from: the Tauri app's
    /// `BASE_STYLE` (`src/message-frame.ts`).
    static let baseStyle = """
    html { color-scheme: light; background: #fff; overflow: hidden; }
    body {
      margin: 16px;
      color: #1d1d1f;
      font: 14px/1.6 -apple-system, BlinkMacSystemFont, "Hiragino Sans", sans-serif;
      overflow-wrap: anywhere;
    }
    pre { white-space: pre-wrap; }
    a { color: #1f5fbf; }
    """

    /// Marks where the message ends, so that its height can be read whatever
    /// the message does to `html` and `body` (a `height: 100%` would otherwise
    /// make it as tall as the view). Its own inline `!important` styles win
    /// over anything the message's CSS says about it.
    static let endMarkerID = "mailcrew-end"
    private static let endMarker = """
    <mailcrew-end id="\(endMarkerID)" style="display: block !important; clear: both !important; \
    height: 0 !important; margin: 0 !important; padding: 0 !important; border: 0 !important; \
    position: static !important; float: none !important;"></mailcrew-end>
    """

    /// The Content-Security-Policy a message runs under.
    static func policy(allowRemoteImages: Bool) -> String {
        let images = allowRemoteImages ? "data: https: http:" : "data:"
        return "default-src 'none'; style-src 'unsafe-inline'; img-src \(images)"
    }

    /// The whole document the view is given, around the sanitised body.
    static func document(html: String, allowRemoteImages: Bool) -> String {
        [
            "<!doctype html><html><head><meta charset=\"utf-8\">",
            "<meta http-equiv=\"Content-Security-Policy\" content=\"\(escapeAttribute(policy(allowRemoteImages: allowRemoteImages)))\">",
            "<meta name=\"referrer\" content=\"no-referrer\">",
            "<style>\(baseStyle)</style>",
            "</head><body>",
            html,
            endMarker,
            "</body></html>",
        ].joined()
    }

    private static func escapeAttribute(_ value: String) -> String {
        value
            .replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "\"", with: "&quot;")
            .replacingOccurrences(of: "<", with: "&lt;")
    }

    /// The `WKContentRuleList` source: block everything, then let the
    /// document (`about:`), `data:` URLs and, once allowed, images from the
    /// web through. Later rules win over earlier ones.
    static func ruleList(allowRemoteImages: Bool) -> String {
        var rules: [[String: Any]] = [
            ["trigger": ["url-filter": ".*"], "action": ["type": "block"]],
            // The rules' regular expressions have no alternation (`|`).
            ["trigger": ["url-filter": "^about:"], "action": ["type": "ignore-previous-rules"]],
            ["trigger": ["url-filter": "^data:"], "action": ["type": "ignore-previous-rules"]],
        ]
        if allowRemoteImages {
            rules.append([
                "trigger": ["url-filter": "^https?://", "resource-type": ["image"]],
                "action": ["type": "ignore-previous-rules"],
            ])
        }
        let data = try! JSONSerialization.data(withJSONObject: rules, options: [.sortedKeys])
        return String(decoding: data, as: UTF8.self)
    }

    enum Navigation: Equatable {
        /// The document the app gave the view.
        case allow
        /// A link the user clicked: opened by the OS, never in the view.
        case openOutside(URL)
        case refuse
    }

    /// What to do with a navigation the web view asks about. Only the app's
    /// own `loadHTMLString` (`about:blank`, not from a click) stays.
    static func decide(url: URL?, linkActivated: Bool, isOpenable: (String) -> Bool) -> Navigation {
        guard let url else { return .refuse }
        if !linkActivated && url.absoluteString == "about:blank" { return .allow }
        if linkActivated && isOpenable(url.absoluteString) { return .openOutside(url) }
        return .refuse
    }

    /// Scales the message to the view's width when it is laid out wider, as
    /// newsletters built for a 600-pixel column are, and says how tall it is
    /// then, in points. Runs in the app's own content world.
    static let fitScript = """
    const root = document.documentElement;
    const body = document.body;
    const end = document.getElementById("\(endMarkerID)");
    if (!root || !body || !end) return 0;
    root.style.removeProperty("zoom");
    const available = root.clientWidth;
    const wanted = root.scrollWidth;
    const zoom = available > 0 && wanted > available + 1 ? available / wanted : 1;
    if (zoom < 1) root.style.setProperty("zoom", String(zoom));
    const style = getComputedStyle(body);
    const below = parseFloat(style.paddingBottom) + parseFloat(style.borderBottomWidth)
        + parseFloat(style.marginBottom);
    // WebKit reports the boxes inside a zoomed root, and the body's
    // computed lengths, with the zoom already applied.
    return Math.ceil(end.getBoundingClientRect().bottom + window.scrollY + below);
    """
}

/// Plain text with its web addresses made into links, as the Tauri app's
/// `renderText` does.
enum MessageText {
    private static let urlPattern = try! NSRegularExpression(pattern: #"\bhttps?://[^\s<>"'）」】]+"#)

    static func attributed(_ text: String, isOpenable: (String) -> Bool) -> AttributedString {
        var result = AttributedString()
        let ns = text as NSString
        var last = 0
        for match in urlPattern.matches(in: text, range: NSRange(location: 0, length: ns.length)) {
            var link = ns.substring(with: match.range)
            while let tail = link.last, ".,;:!?)".contains(tail) { link.removeLast() }
            let range = NSRange(location: match.range.location, length: (link as NSString).length)
            result += AttributedString(ns.substring(with: NSRange(location: last, length: range.location - last)))
            var piece = AttributedString(link)
            if isOpenable(link), let url = URL(string: link) { piece.link = url }
            result += piece
            last = range.location + range.length
        }
        result += AttributedString(ns.substring(from: last))
        return result
    }
}
