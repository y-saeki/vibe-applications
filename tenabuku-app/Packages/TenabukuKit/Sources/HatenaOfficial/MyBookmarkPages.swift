import Foundation

/// The pages of one search over the user's bookmarks read so far, so that a
/// list can read on as it is scrolled.
public struct MyBookmarkPages: Equatable, Sendable {
    public let query: String
    public private(set) var hits: [MyBookmarkSearchHit] = []
    /// Matches in total as of the last page, or nil before the first.
    public private(set) var total: Int?
    /// Where the next page starts. It advances by what each page held rather
    /// than by `hits`, which leaves out repeats.
    public private(set) var nextOffset = 0
    private var reachedEnd = false

    public init(query: String) {
        self.query = query
    }

    /// Whether another page is worth asking for. A page that comes back empty
    /// ends the list even when `total` says there are more, so that a count
    /// the index has not caught up with cannot ask for pages forever.
    public var hasMore: Bool {
        guard let total else { return true }
        return !reachedEnd && nextOffset < total
    }

    public mutating func append(_ page: MyBookmarkSearchPage) {
        total = page.total
        nextOffset += page.hits.count
        reachedEnd = page.hits.isEmpty
        // Bookmarks added while reading shift the offsets, so a page can
        // repeat hits an earlier one held.
        var seen = Set(hits.map(\.id))
        hits += page.hits.filter { seen.insert($0.id).inserted }
    }

    /// Reflects a bookmark the user edited from the list, which the search
    /// index only catches up with later.
    public mutating func update(_ url: URL, with bookmark: MyBookmark) {
        guard let index = hits.firstIndex(where: { $0.url == url }) else { return }
        hits[index] = hits[index].updated(with: bookmark)
    }

    /// Drops a bookmark the user deleted from the list.
    public mutating func remove(_ url: URL) {
        guard let index = hits.firstIndex(where: { $0.url == url }) else { return }
        hits.remove(at: index)
        nextOffset = max(nextOffset - 1, 0)
        total = total.map { max($0 - 1, 0) }
    }
}

extension MyBookmarkSearchHit {
    /// The hit with the comment and privacy of `bookmark`, as saved just now.
    public func updated(with bookmark: MyBookmark) -> MyBookmarkSearchHit {
        MyBookmarkSearchHit(
            title: title,
            url: url,
            bookmarkCount: bookmarkCount,
            snippet: snippet,
            comment: bookmark.comment,
            isPrivate: bookmark.isPrivate,
            bookmarkedAt: bookmarkedAt
        )
    }
}
