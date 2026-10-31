import Foundation
import HatenaShared

// Shapes follow
// https://developer.hatena.ne.jp/ja/documents/bookmark/apis/rest/datatype/
// Responses that need sign-in have not been seen first hand yet, so every
// field the documentation does not insist on is optional here.

public struct MyBookmark: Equatable, Sendable {
    public let user: String
    public let comment: String
    public let tags: [String]
    public let isPrivate: Bool
    public let createdAt: Date?
    public let permalink: URL?

    public init(user: String, comment: String, tags: [String], isPrivate: Bool, createdAt: Date?, permalink: URL?) {
        self.user = user
        self.comment = comment
        self.tags = tags
        self.isPrivate = isPrivate
        self.createdAt = createdAt
        self.permalink = permalink
    }
}

extension MyBookmark: Decodable {
    private enum CodingKeys: String, CodingKey {
        case user, comment, tags, permalink
        case isPrivate = "private"
        case createdEpoch = "created_epoch"
        case createdDatetime = "created_datetime"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        user = try container.decode(String.self, forKey: .user)
        comment = try container.decodeIfPresent(String.self, forKey: .comment) ?? ""
        tags = try container.decodeIfPresent([String].self, forKey: .tags) ?? []
        isPrivate = try container.decodeIfPresent(LooseBool.self, forKey: .isPrivate)?.value ?? false
        permalink = try container.decodeIfPresent(String.self, forKey: .permalink).flatMap(URL.init(string:))
        if let epoch = try container.decodeIfPresent(LooseInt.self, forKey: .createdEpoch) {
            createdAt = Date(timeIntervalSince1970: TimeInterval(epoch.value))
        } else {
            createdAt = try container.decodeIfPresent(String.self, forKey: .createdDatetime)
                .flatMap(HatenaDate.parseISO8601)
        }
    }
}

public struct HatenaUser: Equatable, Sendable {
    public let name: String
    public let isPlusUser: Bool
    /// Whether the user keeps all of their bookmarks private.
    public let isPrivate: Bool
}

extension HatenaUser: Decodable {
    private enum CodingKeys: String, CodingKey {
        case name
        case isPlusUser = "plususer"
        case isPrivate = "private"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        isPlusUser = try container.decodeIfPresent(LooseBool.self, forKey: .isPlusUser)?.value ?? false
        isPrivate = try container.decodeIfPresent(LooseBool.self, forKey: .isPrivate)?.value ?? false
    }
}

public struct MyBookmarkSearchPage: Equatable, Sendable {
    /// Matches in total, of which `hits` is one page.
    public let total: Int
    public let hits: [MyBookmarkSearchHit]
}

public struct MyBookmarkSearchHit: Equatable, Sendable, Identifiable {
    public var id: String { url.absoluteString }
    public let title: String
    public let url: URL
    public let bookmarkCount: Int
    public let snippet: String
    public let comment: String
    public let isPrivate: Bool
    public let bookmarkedAt: Date?
}

extension MyBookmarkSearchPage: Decodable {
    private struct Meta: Decodable {
        let total: LooseInt
    }

    private struct Hit: Decodable {
        struct Entry: Decodable {
            let title: String
            let url: String
            let count: LooseInt?
            let snippet: String?
        }

        let entry: Entry
        let timestamp: LooseInt?
        let comment: String?
        let is_private: LooseBool?
    }

    private enum CodingKeys: String, CodingKey {
        case bookmarks, meta
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        total = try container.decode(Meta.self, forKey: .meta).total.value
        hits = try container.decode([Hit].self, forKey: .bookmarks).compactMap { hit in
            guard let url = URL(string: hit.entry.url) else { return nil }
            return MyBookmarkSearchHit(
                title: hit.entry.title,
                url: url,
                bookmarkCount: hit.entry.count?.value ?? 0,
                snippet: hit.entry.snippet ?? "",
                comment: hit.comment ?? "",
                isPrivate: hit.is_private?.value ?? false,
                bookmarkedAt: hit.timestamp.map { Date(timeIntervalSince1970: TimeInterval($0.value)) }
            )
        }
    }
}
