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

public struct UserTag: Equatable, Sendable, Identifiable, Decodable {
    public var id: String { name }
    public let name: String
    /// How many of the user's bookmarks carry the tag.
    public let count: Int

    public init(name: String, count: Int) {
        self.name = name
        self.count = count
    }

    private enum CodingKeys: String, CodingKey {
        case name = "tag"
        case count
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        name = try container.decode(String.self, forKey: .name)
        count = try container.decodeIfPresent(LooseInt.self, forKey: .count)?.value ?? 0
    }

    /// The tags to offer while the user types `input`, leaving out those in
    /// `excluded`. Tags that start with the input come before those that only
    /// contain it, and the more used come first within each. With no input,
    /// every tag is offered, most used first. Letter case is ignored.
    public static func suggestions(from tags: [UserTag], matching input: String, excluding excluded: [String]) -> [UserTag] {
        let needle = input.trimmingCharacters(in: .whitespacesAndNewlines)
        let candidates = tags.filter { !excluded.contains($0.name) }
        let ranked: [(tag: UserTag, rank: Int)] = candidates.compactMap { tag in
            if needle.isEmpty {
                return (tag, 0)
            }
            guard let range = tag.name.range(of: needle, options: [.caseInsensitive, .widthInsensitive]) else {
                return nil
            }
            return (tag, range.lowerBound == tag.name.startIndex ? 0 : 1)
        }
        return ranked.sorted { lhs, rhs in
            if lhs.rank != rhs.rank { return lhs.rank < rhs.rank }
            if lhs.tag.count != rhs.tag.count { return lhs.tag.count > rhs.tag.count }
            return lhs.tag.name < rhs.tag.name
        }.map(\.tag)
    }
}

/// The documentation says the tag API returns an array of tags; the API is
/// also known to wrap it as `{"tags": [...]}`. Both are accepted until a real
/// response has been seen.
struct UserTagList: Decodable {
    let tags: [UserTag]

    private enum CodingKeys: String, CodingKey {
        case tags
    }

    init(from decoder: Decoder) throws {
        if let container = try? decoder.container(keyedBy: CodingKeys.self) {
            tags = try container.decode([UserTag].self, forKey: .tags)
        } else {
            tags = try decoder.singleValueContainer().decode([UserTag].self)
        }
    }
}

public struct MyBookmarkSearchPage: Equatable, Sendable {
    /// Matches in total, of which `hits` is one page.
    public let total: Int
    public let hits: [MyBookmarkSearchHit]
}

public struct MyBookmarkSearchHit: Equatable, Sendable, Identifiable, Codable {
    public var id: String { url.absoluteString }
    public let title: String
    public let url: URL
    public let bookmarkCount: Int
    public let snippet: String
    public let comment: String
    public let isPrivate: Bool
    public let bookmarkedAt: Date?
    /// The entry's ID, which names the bookmark to Hatena Star. Nil in
    /// timelines saved before it was kept.
    public let eid: String?

    public init(
        title: String,
        url: URL,
        bookmarkCount: Int,
        snippet: String,
        comment: String,
        isPrivate: Bool,
        bookmarkedAt: Date?,
        eid: String? = nil
    ) {
        self.title = title
        self.url = url
        self.bookmarkCount = bookmarkCount
        self.snippet = snippet
        self.comment = comment
        self.isPrivate = isPrivate
        self.bookmarkedAt = bookmarkedAt
        self.eid = eid
    }

    /// The tags the search writes at the start of `comment`, as `[tag]`.
    public var tags: [String] { Self.split(comment).tags }

    /// `comment` without its leading tags.
    public var commentBody: String { Self.split(comment).body }

    /// Splits off the `[tag]`s a comment starts with. A bracket that is not
    /// closed, or an empty pair, ends the tags and stays in the body.
    static func split(_ comment: String) -> (tags: [String], body: String) {
        var tags: [String] = []
        var rest = Substring(comment)
        while rest.first == "[", let close = rest.firstIndex(of: "]") {
            let tag = rest[rest.index(after: rest.startIndex)..<close]
            guard !tag.isEmpty, !tag.contains("[") else { break }
            tags.append(String(tag))
            rest = rest[rest.index(after: close)...]
        }
        return (tags, String(rest))
    }

    /// The comment as the search writes it, from the tags and body the REST
    /// API keeps apart.
    static func comment(tags: [String], body: String) -> String {
        tags.map { "[\($0)]" }.joined() + body
    }
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
            let eid: LooseString?
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
                bookmarkedAt: hit.timestamp.map { Date(timeIntervalSince1970: TimeInterval($0.value)) },
                eid: hit.entry.eid.map(\.value).flatMap { $0.isEmpty ? nil : $0 }
            )
        }
    }
}
