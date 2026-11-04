import Foundation

/// One page of a search over the user's bookmarks, to be read next.
public struct MyBookmarkSearchRequest: Equatable, Sendable {
    public let query: String
    public let offset: Int

    public init(query: String, offset: Int) {
        self.query = query
        self.offset = offset
    }
}

/// A list of the user's bookmarks built from pages of search hits, read on
/// one page at a time as it is scrolled.
public protocol MyBookmarkListing: Sendable {
    /// What to show, in order, without repeats.
    var hits: [MyBookmarkSearchHit] { get }
    /// The page to read next, or nil when there is nothing more to show.
    var nextRequest: MyBookmarkSearchRequest? { get }
    /// Takes in the answer to `request`. A page asked for under an earlier
    /// state of the listing is dropped.
    mutating func receive(_ page: MyBookmarkSearchPage, for request: MyBookmarkSearchRequest)
    /// Reflects a bookmark the user edited from the list, which the search
    /// index only catches up with later.
    mutating func update(_ url: URL, with bookmark: MyBookmark)
    /// Drops a bookmark the user deleted from the list.
    mutating func remove(_ url: URL)
}

extension MyBookmarkListing {
    public var hasMore: Bool { nextRequest != nil }
}

/// The pages of one search over the user's bookmarks read so far.
public struct MyBookmarkPages: MyBookmarkListing, Equatable {
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

    /// A page that comes back empty ends the list even when `total` says
    /// there are more, so that a count the index has not caught up with
    /// cannot ask for pages forever. Hits past `searchDepth` cannot be read.
    public var nextRequest: MyBookmarkSearchRequest? {
        if let total, reachedEnd || nextOffset >= total { return nil }
        guard nextOffset < BookmarkAPI.searchDepth else { return nil }
        return MyBookmarkSearchRequest(query: query, offset: nextOffset)
    }

    public mutating func receive(_ page: MyBookmarkSearchPage, for request: MyBookmarkSearchRequest) {
        guard request == nextRequest else { return }
        append(page)
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

    public mutating func update(_ url: URL, with bookmark: MyBookmark) {
        guard let index = hits.firstIndex(where: { $0.url == url }) else { return }
        hits[index] = hits[index].updated(with: bookmark)
    }

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
