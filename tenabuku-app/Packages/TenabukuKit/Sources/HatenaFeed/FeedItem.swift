import Foundation

/// One item of a Hatena Bookmark RSS feed. Hot entries and search results
/// describe an entry; a user's feed describes that user's bookmark of it.
public struct FeedItem: Hashable, Sendable, Identifiable {
    public var id: URL { link }
    public let title: String
    public let link: URL
    public let summary: String
    public let date: Date?
    /// `dc:subject`. On hot entries the first one is the category name.
    public let subjects: [String]
    public let bookmarkCount: Int
    public let imageURL: URL?
    /// `dc:creator`, set on a user's own feed.
    public let creator: String?

    public init(
        title: String,
        link: URL,
        summary: String = "",
        date: Date? = nil,
        subjects: [String] = [],
        bookmarkCount: Int = 0,
        imageURL: URL? = nil,
        creator: String? = nil
    ) {
        self.title = title
        self.link = link
        self.summary = summary
        self.date = date
        self.subjects = subjects
        self.bookmarkCount = bookmarkCount
        self.imageURL = imageURL
        self.creator = creator
    }
}

public enum HotEntryCategory: String, CaseIterable, Identifiable, Sendable {
    case all
    case general
    case social
    case economics
    case life
    case knowledge
    case it
    case fun
    case entertainment
    case game

    public var id: String { rawValue }

    /// The name Hatena Bookmark shows for the category.
    public var displayName: String {
        switch self {
        case .all: "総合"
        case .general: "一般"
        case .social: "世の中"
        case .economics: "政治と経済"
        case .life: "暮らし"
        case .knowledge: "学び"
        case .it: "テクノロジー"
        case .fun: "おもしろ"
        case .entertainment: "エンタメ"
        case .game: "アニメとゲーム"
        }
    }
}
