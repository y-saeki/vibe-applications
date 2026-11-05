import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The "新着コメント" (recent comments) of an entry, newest first, through the
/// JSON the website's entry page loads as the reader scrolls.
///
/// The documented entry info API (`/entry/jsonlite/`) has the same comments,
/// but its responses are cached for three hours on Hatena's CDN, so the
/// newest comments can be hours late. This endpoint is cached for three
/// minutes. Undocumented and may change without notice. The shape was taken
/// from responses in 2026-11.
public struct RecentCommentsAPI: Sendable {
    static let source = "unofficial.entry-bookmarks"
    /// The most the endpoint returns at once.
    static let pageSize = 100

    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// One page of comments. Pass the previous page's `nextCursor` to continue.
    public func comments(for url: URL, cursor: String? = nil) async throws -> CommentPage {
        var request = URLRequest(url: Self.url(for: url, cursor: cursor))
        // The response says it may be kept for minutes; reading again should
        // show what Hatena has now.
        request.cachePolicy = .reloadIgnoringLocalCacheData
        let data = try await http.data(for: request)
        return try JSONDecoder.hatena(CommentPage.self, from: Self.escapingControlCharacters(data), source: Self.source)
    }

    static func url(for url: URL, cursor: String?) -> URL {
        var query = [("commented_only", "1"), ("limit", String(pageSize))]
        if let cursor {
            query.append(("cursor", cursor))
        }
        let base = "https://b.hatena.ne.jp/api/entry/" + PercentEncoding.encode(url.absoluteString) + "/bookmarks"
        return URL(string: base + "?" + PercentEncoding.encodeForm(query))!
    }

    /// Hatena sometimes leaves a control character from a comment unescaped,
    /// which JSON does not allow and the decoder rejects. Each becomes a
    /// space: still valid between tokens, and harmless inside a comment. UTF-8
    /// never uses these bytes inside a multi-byte character.
    static func escapingControlCharacters(_ data: Data) -> Data {
        guard data.contains(where: { $0 < 0x20 }) else { return data }
        return Data(data.map { $0 < 0x20 ? 0x20 : $0 })
    }
}

public struct CommentPage: Equatable, Sendable {
    public let comments: [EntryComment]
    /// Nil on the last page.
    public let nextCursor: String?
}

extension CommentPage: Decodable {
    private struct Bookmark: Decodable {
        struct User: Decodable {
            struct Image: Decodable {
                let image_url: String?
            }

            let name: String
            let image: Image?
        }

        let user: User
        let comment: String?
        let tags: [String]?
        let created: String?
    }

    private enum CodingKeys: String, CodingKey {
        case bookmarks, cursor
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        comments = try container.decode([Bookmark].self, forKey: .bookmarks).map {
            EntryComment(
                user: $0.user.name,
                iconURL: $0.user.image?.image_url.flatMap(URL.init(string:)),
                comment: $0.comment ?? "",
                tags: $0.tags ?? [],
                date: $0.created.flatMap(HatenaDate.parseISO8601)
            )
        }
        // The last page has no cursor, or an empty one.
        nextCursor = try container.decodeIfPresent(String.self, forKey: .cursor).flatMap { $0.isEmpty ? nil : $0 }
    }
}
