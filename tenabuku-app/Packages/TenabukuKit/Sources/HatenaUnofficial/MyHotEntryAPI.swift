import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The signed-in user's "My hot entries": entries popular among the users they
/// follow and entries matching their interest words.
///
/// There is no official API or feed for it. This reads the JSON the website's
/// own `/{id}/hotentry` page loads when it appends earlier days, which needs
/// the website's login cookies rather than an OAuth token. Undocumented and
/// may change without notice. The shape was taken from responses in 2026-11.
public struct MyHotEntryAPI: Sendable {
    static let source = "unofficial.my-hotentry"
    static let url = URL(string: "https://b.hatena.ne.jp/api/my/feed_digest")!

    private let http: HatenaHTTPClient
    private let cookieHeader: String?

    /// `cookies` are the website's, as a browser holds them; those that would
    /// not be sent to the endpoint are left out.
    public init(http: HatenaHTTPClient, cookies: [HTTPCookie]) {
        self.http = http
        cookieHeader = WebCookies.header(for: Self.url, from: cookies)
    }

    /// Today's entries, in Hatena's order (highest score first). Empty when the
    /// user follows nobody and has no interest words, or nothing matched yet.
    /// Throws `HatenaError.unauthorized` when the website's login is missing or
    /// has expired.
    public func entries() async throws -> [MyHotEntry] {
        // Without a cookie Hatena can only answer "login required".
        guard let cookieHeader else { throw HatenaError.unauthorized }
        let (data, response) = try await http.send(WebCookies.request(Self.url, header: cookieHeader))
        // Signed out, the endpoint answers 403 {"reason":"login required"}.
        if response.statusCode == 403 {
            throw HatenaError.unauthorized
        }
        try HatenaHTTPClient.validate(response)
        return try JSONDecoder.hatena(Digest.self, from: data, source: Self.source).entries
    }
}

public struct MyHotEntry: Equatable, Sendable {
    public let title: String
    public let url: URL
    public let summary: String
    /// When the entry was first bookmarked.
    public let date: Date?
    public let bookmarkCount: Int
    public let imageURL: URL?
}

private struct Digest: Decodable {
    struct Item: Decodable {
        struct Entry: Decodable {
            let url: String
            let title: String
            let count: LooseInt
            let content: String?
            let image_url: String?
            let created: String?
        }

        let entry: Entry
    }

    let item: [Item]

    var entries: [MyHotEntry] {
        item.compactMap { item in
            let entry = item.entry
            guard let url = URL(string: entry.url) else { return nil }
            return MyHotEntry(
                title: entry.title,
                url: url,
                summary: entry.content ?? "",
                date: entry.created.flatMap(HatenaDate.parseISO8601),
                bookmarkCount: entry.count.value,
                imageURL: entry.image_url.flatMap(URL.init(string:))
            )
        }
    }
}
