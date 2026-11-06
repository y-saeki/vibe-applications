import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The Hatena Stars on bookmark comments: how many each has, and adding one.
///
/// A comment is named to Hatena Star by its permalink,
/// `https://b.hatena.ne.jp/{user}/{yyyyMMdd}#bookmark-{eid}`, the day being
/// when the bookmark was made, in Japan time. This is what the website's star
/// widget sends; the entry page or the entry URL get no stars.
///
/// Reading needs no sign-in. Adding has no OAuth API: like the widget, it
/// sends the website's login cookies and the `rks` token that `entries.json`
/// hands a signed-in browser. Undocumented and may change without notice. The
/// shapes were taken from the widget's script and responses in 2026-11.
public struct HatenaStarAPI: Sendable {
    static let source = "unofficial.star"
    static let host = "https://s.hatena.ne.jp"
    /// Comments asked about in one request, which keeps the URL well under
    /// the 2,048 characters the widget stays within.
    static let batchSize = 20

    private let http: HatenaHTTPClient
    private let cookies: [HTTPCookie]

    /// `cookies` are the website's, as a browser holds them. Reading works
    /// without them; adding does not.
    public init(http: HatenaHTTPClient, cookies: [HTTPCookie] = []) {
        self.http = http
        self.cookies = cookies
    }

    /// The permalink Hatena Star knows the bookmark of `user` by.
    public static func uri(user: String, eid: String, date: Date) -> URL? {
        guard !user.isEmpty, !eid.isEmpty else { return nil }
        return URL(string: "https://b.hatena.ne.jp/\(user)/\(HatenaDate.compactDay(date))#bookmark-\(eid)")
    }

    /// How many stars each of `uris` has, by colour. A comment nobody has
    /// starred is left out of Hatena's answer and has none.
    public func counts(for uris: [URL]) async throws -> [URL: StarCount] {
        let batches = stride(from: 0, to: uris.count, by: Self.batchSize).map {
            Array(uris[$0..<min($0 + Self.batchSize, uris.count)])
        }
        return try await withThrowingTaskGroup(of: Entries.self) { group in
            for batch in batches {
                group.addTask { try await entries(for: batch, withCookies: false) }
            }
            var counts = Dictionary(uris.map { ($0, StarCount.none) }, uniquingKeysWith: { first, _ in first })
            for try await entries in group {
                for entry in entries.entries {
                    guard let uri = URL(string: entry.uri), counts[uri] != nil else { continue }
                    counts[uri] = entry.starCount
                }
            }
            return counts
        }
    }

    /// Adds one yellow star to `uri`, as the signed-in user. Throws
    /// `HatenaError.unauthorized` when the website's login is missing or has
    /// expired, and `HatenaError.rejected` when Hatena refuses the star.
    public func addStar(to uri: URL) async throws {
        guard WebCookies.header(for: Self.entriesURL(for: [uri]), from: cookies) != nil else {
            throw HatenaError.unauthorized
        }
        // The token Hatena checks the request against. It is handed out only
        // with the login cookies, and null without them.
        guard let rks = try await entries(for: [uri], withCookies: true).rks, !rks.isEmpty else {
            throw HatenaError.unauthorized
        }
        let url = Self.addURL(for: uri, rks: rks)
        let (data, response) = try await http.send(request(url, withCookies: true))
        try HatenaHTTPClient.validate(response)
        let result = try JSONDecoder.hatena(AddResult.self, from: data, source: Self.source)
        if result.is_guest?.value == true {
            throw HatenaError.unauthorized
        }
        if let errors = result.errors, !errors.isEmpty {
            throw HatenaError.rejected(errors.joined(separator: " "))
        }
    }

    static func entriesURL(for uris: [URL]) -> URL {
        let query = uris.map { ("uri", $0.absoluteString) } + [("no_comments", "1")]
        return URL(string: host + "/entries.json?" + PercentEncoding.encodeForm(query))!
    }

    static func addURL(for uri: URL, rks: String) -> URL {
        URL(string: host + "/star.add.json?" + PercentEncoding.encodeForm([("uri", uri.absoluteString), ("rks", rks)]))!
    }

    private func entries(for uris: [URL], withCookies: Bool) async throws -> Entries {
        var request = request(Self.entriesURL(for: uris), withCookies: withCookies)
        // A star added a moment ago should show when the list is read again.
        request.cachePolicy = .reloadIgnoringLocalCacheData
        let data = try await http.data(for: request)
        return try JSONDecoder.hatena(Entries.self, from: data, source: Self.source)
    }

    private func request(_ url: URL, withCookies: Bool) -> URLRequest {
        if withCookies, let header = WebCookies.header(for: url, from: cookies) {
            return WebCookies.request(url, header: header)
        }
        var request = URLRequest(url: url)
        // Reading never sends what the session might hold.
        request.httpShouldHandleCookies = false
        return request
    }
}

/// The stars on one comment: the ordinary yellow ones, which anyone can
/// give, and the coloured ones, which are bought or earned.
public struct StarCount: Equatable, Sendable {
    public struct Colored: Equatable, Sendable {
        /// Hatena's name for the colour: `green`, `red`, `blue` or `purple`.
        public let color: String
        public var count: Int

        public init(color: String, count: Int) {
            self.color = color
            self.count = count
        }
    }

    public var yellow: Int
    /// In Hatena's order, without colours that have no star.
    public var colored: [Colored]

    public static let none = StarCount(yellow: 0, colored: [])

    public init(yellow: Int, colored: [Colored]) {
        self.yellow = yellow
        self.colored = colored
    }

    public var total: Int {
        colored.reduce(yellow) { $0 + $1.count }
    }
}

private struct Entries: Decodable {
    struct Entry: Decodable {
        let uri: String
        let stars: [Star]?
        let colored_stars: [Colored]?

        struct Colored: Decodable {
            let color: String
            let stars: [Star]
        }

        var starCount: StarCount {
            // Hatena lists each colour once; the order it gives is kept.
            var colored: [StarCount.Colored] = []
            for group in colored_stars ?? [] {
                let count = group.stars.reduce(0) { $0 + $1.count }
                if let index = colored.firstIndex(where: { $0.color == group.color }) {
                    colored[index].count += count
                } else if count > 0 {
                    colored.append(.init(color: group.color, count: count))
                }
            }
            return StarCount(yellow: (stars ?? []).reduce(0) { $0 + $1.count }, colored: colored)
        }
    }

    let entries: [Entry]
    let rks: String?
}

/// One element of a star list: a star of one user, given `count` times, or
/// a bare number standing for that many stars left out of a long list.
private struct Star: Decodable {
    let count: Int

    private enum CodingKeys: String, CodingKey {
        case name, count
    }

    init(from decoder: Decoder) throws {
        if let omitted = try? decoder.singleValueContainer().decode(Int.self) {
            count = omitted
            return
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        _ = try container.decode(String.self, forKey: .name)
        count = try container.decodeIfPresent(LooseInt.self, forKey: .count)?.value ?? 1
    }
}

private struct AddResult: Decodable {
    let is_guest: LooseBool?
    /// Why Hatena refused the star. Only ever read as a message, so a list
    /// that is not of strings still counts as a refusal.
    let errors: [String]?

    private enum CodingKeys: String, CodingKey {
        case is_guest, errors
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        is_guest = try container.decodeIfPresent(LooseBool.self, forKey: .is_guest)
        if (try? container.decodeNil(forKey: .errors)) ?? true {
            errors = nil
        } else {
            errors = (try? container.decode([String].self, forKey: .errors)) ?? ["(unreadable)"]
        }
    }
}
