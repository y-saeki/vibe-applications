import Foundation
import HatenaShared

/// Hatena Bookmark's RSS feeds. None of them needs sign-in.
public struct FeedClient: Sendable {
    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The current hot entries of a category, 30 or so.
    public func hotEntries(_ category: HotEntryCategory) async throws -> [FeedItem] {
        let items = try RSS1Parser.parse(try await http.get(Self.hotEntryURL(category)), source: "feed.hotentry")
        // A hot entry list is never empty; an empty one means the feed changed.
        guard !items.isEmpty else {
            throw HatenaError.parseFailed(source: "feed.hotentry", detail: "no items for \(category.rawValue)")
        }
        return items
    }

    /// Entries matching a search, 40 per request.
    public func search(_ query: SearchQuery) async throws -> [FeedItem] {
        try RSS1Parser.parse(try await http.get(query.url), source: "feed.search")
    }

    /// A user's public bookmarks, newest first, 20 per page.
    public func bookmarks(of user: String, page: Int = 1, tag: String? = nil) async throws -> [FeedItem] {
        try RSS1Parser.parse(try await http.get(Self.userFeedURL(user, page: page, tag: tag)), source: "feed.user")
    }

    static func hotEntryURL(_ category: HotEntryCategory) -> URL {
        URL(string: "https://b.hatena.ne.jp/hotentry/\(category.rawValue).rss")!
    }

    static func userFeedURL(_ user: String, page: Int, tag: String?) -> URL {
        var query: [(String, String)] = []
        if page > 1 {
            query.append(("page", String(page)))
        }
        if let tag {
            query.append(("tag", tag))
        }
        let base = "https://b.hatena.ne.jp/\(PercentEncoding.encode(user))/bookmark.rss"
        return URL(string: query.isEmpty ? base : base + "?" + PercentEncoding.encodeForm(query))!
    }
}

/// A keyword search over all of Hatena Bookmark, which is how past hot
/// entries are found: sort by popularity, narrow to a period and a minimum
/// number of bookmarks.
public struct SearchQuery: Equatable, Sendable {
    public enum Sort: String, Sendable {
        case recent
        case popular
    }

    public var keyword: String
    public var sort: Sort
    /// First and last day, both inclusive, taken in Japan time.
    public var since: Date?
    public var until: Date?
    public var minimumBookmarks: Int?

    public init(keyword: String, sort: Sort = .recent, since: Date? = nil, until: Date? = nil, minimumBookmarks: Int? = nil) {
        self.keyword = keyword
        self.sort = sort
        self.since = since
        self.until = until
        self.minimumBookmarks = minimumBookmarks
    }

    public var url: URL {
        var query: [(String, String)] = [("mode", "rss"), ("sort", sort.rawValue)]
        if let since {
            query.append(("date_begin", HatenaDate.day(since)))
        }
        if let until {
            query.append(("date_end", HatenaDate.day(until)))
        }
        if let minimumBookmarks {
            query.append(("users", String(minimumBookmarks)))
        }
        let base = "https://b.hatena.ne.jp/q/" + PercentEncoding.encode(keyword)
        return URL(string: base + "?" + PercentEncoding.encodeForm(query))!
    }
}
