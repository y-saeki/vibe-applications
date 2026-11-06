import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The "注目コメント" (popular comments) of an entry, read from the website's
/// entry page.
///
/// No API returns this ranking: `/api/entry/{url}/bookmarks` ignores its
/// `sort` parameter and always answers newest first. The page has the list in
/// its HTML, already ranked by Hatena, and needs no sign-in. Undocumented and
/// may change without notice. The markup was taken from pages in 2026-11.
public struct PopularCommentsAPI: Sendable {
    static let source = "unofficial.entry-page-popular"

    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The popular comments, in Hatena's order. Empty when the entry has none
    /// yet, or nobody has bookmarked the URL.
    public func comments(for url: URL) async throws -> [EntryComment] {
        let (data, response) = try await http.send(URLRequest(url: Self.url(for: url)))
        try HatenaHTTPClient.validate(response)
        // An unbookmarked URL is redirected away from the entry page, to the
        // site's list of entries.
        if let final = response.url, !Self.isEntryPage(final) {
            return []
        }
        return try Self.parse(String(decoding: data, as: UTF8.self))
    }

    static func url(for url: URL) -> URL {
        URL(string: "https://b.hatena.ne.jp/entry?url=" + PercentEncoding.encode(url.absoluteString))!
    }

    static func isEntryPage(_ url: URL) -> Bool {
        url.path == "/entry" || url.path.hasPrefix("/entry/")
    }

    static func parse(_ html: String) throws -> [EntryComment] {
        guard let start = html.range(of: "js-bookmarks-popular\"") else {
            throw HatenaError.parseFailed(source: source, detail: "no popular comments section")
        }
        // The popular panel is followed by the recent one.
        let end = html.range(of: "data-sort=\"recent\"", range: start.upperBound..<html.endIndex)?.lowerBound ?? html.endIndex
        let section = html[start.upperBound..<end]
        let items = section.components(separatedBy: "<div class=\"entry-comment-contents ").dropFirst()
        return try items.map { item in
            guard let user = HTML.firstMatch(#"data-user-name="([^"]+)""#, in: item) else {
                throw HatenaError.parseFailed(source: source, detail: "comment without a user name")
            }
            guard let text = HTML.firstMatch(#"class="entry-comment-text[^"]*"[^>]*>(.*?)</span>"#, in: item) else {
                throw HatenaError.parseFailed(source: source, detail: "comment of \(user) without text")
            }
            let tagList = HTML.firstMatch(#"<ul class="entry-comment-tags">(.*?)</ul>"#, in: item) ?? ""
            return EntryComment(
                user: HTML.decodeEntities(user),
                iconURL: HTML.firstMatch(#"class="entry-user-icon"[^>]*><img src="([^"]+)""#, in: item)
                    .flatMap { URL(string: HTML.decodeEntities($0)) },
                comment: HTML.text(text),
                tags: HTML.allMatches(#"<a [^>]*>(.*?)</a>"#, in: tagList).map(HTML.text),
                date: HTML.firstMatch(#"itemprop="datePublished" content="([^"]+)""#, in: item)
                    .flatMap(HatenaDate.parseISO8601),
                // The page links each comment's date to its permalink.
                starURI: HTML.firstMatch(#"class="js-bookmark-anchor-path"[^>]*href="([^"]+)""#, in: item)
                    .flatMap { URL(string: HTML.decodeEntities($0)) }
            )
        }
    }
}

public struct EntryComment: Equatable, Sendable, Identifiable {
    public var id: String { user }
    public let user: String
    public let iconURL: URL?
    public let comment: String
    public let tags: [String]
    public let date: Date?
    /// The bookmark's permalink, which Hatena Star names it by. Nil when the
    /// response did not say enough to build it.
    public let starURI: URL?
}

/// Just enough HTML handling for the entry page, without a parser dependency.
enum HTML {
    /// The first capture group of the first match of `pattern`, which may span lines.
    static func firstMatch(_ pattern: String, in string: String) -> String? {
        allMatches(pattern, in: string).first
    }

    static func allMatches(_ pattern: String, in string: String) -> [String] {
        let regex = try! NSRegularExpression(pattern: pattern, options: [.dotMatchesLineSeparators])
        let range = NSRange(string.startIndex..., in: string)
        return regex.matches(in: string, range: range).compactMap { match in
            Range(match.range(at: 1), in: string).map { String(string[$0]) }
        }
    }

    /// The text of an HTML fragment: tags dropped, entities decoded, ends trimmed.
    static func text(_ fragment: String) -> String {
        let stripped = fragment.replacingOccurrences(of: "<[^>]*>", with: "", options: .regularExpression)
        return decodeEntities(stripped).trimmingCharacters(in: .whitespacesAndNewlines)
    }

    static func decodeEntities(_ string: String) -> String {
        guard string.contains("&") else { return string }
        let named = ["amp": "&", "lt": "<", "gt": ">", "quot": "\"", "apos": "'", "nbsp": "\u{00A0}"]
        let regex = try! NSRegularExpression(pattern: "&(#[0-9]+|#[xX][0-9a-fA-F]+|[a-zA-Z]+);")
        var result = ""
        var last = string.startIndex
        for match in regex.matches(in: string, range: NSRange(string.startIndex..., in: string)) {
            guard let whole = Range(match.range, in: string), let name = Range(match.range(at: 1), in: string) else { continue }
            result += string[last..<whole.lowerBound]
            let body = string[name]
            if let value = named[String(body)] {
                result += value
            } else if body.hasPrefix("#x") || body.hasPrefix("#X"),
                      let code = UInt32(body.dropFirst(2), radix: 16), let scalar = Unicode.Scalar(code) {
                result.unicodeScalars.append(scalar)
            } else if body.hasPrefix("#"), let code = UInt32(body.dropFirst()), let scalar = Unicode.Scalar(code) {
                result.unicodeScalars.append(scalar)
            } else {
                result += string[whole]
            }
            last = whole.upperBound
        }
        return result + string[last...]
    }
}
