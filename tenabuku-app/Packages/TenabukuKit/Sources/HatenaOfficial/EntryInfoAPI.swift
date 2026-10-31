import Foundation
import HatenaShared

/// An entry and everyone's bookmarks of it, newest first, through the
/// documented entry info API (jsonlite variant), which needs no sign-in:
/// https://developer.hatena.ne.jp/ja/documents/bookmark/apis/getinfo/
///
/// The list can be shorter than `count`: bookmarks that are private or were
/// hidden are left out.
public struct EntryInfoAPI: Sendable {
    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The entry for `url`, or nil when nobody has bookmarked it.
    public func entry(for url: URL) async throws -> EntryInfo? {
        let data = try await http.get(Self.url(for: url))
        // An unbookmarked URL is answered with a bare `null`.
        if String(decoding: data, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines) == "null" {
            return nil
        }
        return try JSONDecoder.hatena(EntryInfo.self, from: data, source: "official.entry-jsonlite")
    }

    static func url(for url: URL) -> URL {
        URL(string: "https://b.hatena.ne.jp/entry/jsonlite/?url=" + PercentEncoding.encode(url.absoluteString))!
    }
}

public struct EntryInfo: Equatable, Sendable {
    public let eid: String
    public let title: String
    public let url: URL
    public let entryURL: URL?
    public let count: Int
    public let bookmarks: [EntryBookmark]
}

public struct EntryBookmark: Equatable, Sendable, Identifiable {
    public var id: String { user }
    public let user: String
    public let comment: String
    public let tags: [String]
    public let date: Date?
}

extension EntryInfo: Decodable {
    private struct Bookmark: Decodable {
        let user: String
        let comment: String?
        let tags: [String]?
        let timestamp: String?
    }

    private enum CodingKeys: String, CodingKey {
        case eid, title, url, count, bookmarks
        case entryURL = "entry_url"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        eid = try container.decode(LooseString.self, forKey: .eid).value
        title = try container.decode(String.self, forKey: .title)
        let urlString = try container.decode(String.self, forKey: .url)
        guard let url = URL(string: urlString) else {
            throw DecodingError.dataCorruptedError(forKey: .url, in: container, debugDescription: "bad URL \(urlString)")
        }
        self.url = url
        entryURL = try container.decodeIfPresent(String.self, forKey: .entryURL).flatMap(URL.init(string:))
        count = try container.decode(LooseInt.self, forKey: .count).value
        bookmarks = try container.decode([Bookmark].self, forKey: .bookmarks).map {
            EntryBookmark(
                user: $0.user,
                comment: $0.comment ?? "",
                tags: $0.tags ?? [],
                date: $0.timestamp.flatMap(HatenaDate.parseSlashed)
            )
        }
    }
}
