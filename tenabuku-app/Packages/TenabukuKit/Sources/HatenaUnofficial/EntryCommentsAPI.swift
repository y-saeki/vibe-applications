import Foundation
import HatenaShared

/// The comments on an entry, through the JSON the website's own entry page
/// loads. Undocumented: it is the only way to get the "popular" ranking, and
/// it may change without notice. Shapes were taken from responses in 2026-10.
public struct EntryCommentsAPI: Sendable {
    public enum Sort: String, Sendable {
        case popular
        case recent
    }

    static let source = "unofficial.entry-bookmarks"

    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// One page of comments. Pass the previous page's `nextCursor` to continue.
    public func comments(for url: URL, sort: Sort, cursor: String? = nil) async throws -> CommentPage {
        let data = try await http.get(Self.url(for: url, sort: sort, cursor: cursor))
        return try JSONDecoder.hatena(CommentPage.self, from: data, source: Self.source)
    }

    static func url(for url: URL, sort: Sort, cursor: String?) -> URL {
        var query = [("sort", sort.rawValue)]
        if let cursor {
            query.append(("cursor", cursor))
        }
        let base = "https://b.hatena.ne.jp/api/entry/" + PercentEncoding.encode(url.absoluteString) + "/bookmarks"
        return URL(string: base + "?" + PercentEncoding.encodeForm(query))!
    }
}

public struct CommentPage: Equatable, Sendable {
    public let comments: [EntryComment]
    public let nextCursor: String?
}

public struct EntryComment: Equatable, Sendable, Identifiable {
    public var id: String { user }
    public let user: String
    public let iconURL: URL?
    public let comment: String
    public let tags: [String]
    public let date: Date?
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
        nextCursor = try container.decodeIfPresent(String.self, forKey: .cursor)
    }
}
