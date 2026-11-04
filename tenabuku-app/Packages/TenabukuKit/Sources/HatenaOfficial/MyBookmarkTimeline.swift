import Foundation

/// All of the user's bookmarks, private ones included, newest first.
///
/// No documented API lists them, and the full-text search answers an empty
/// query with nothing. A search for "https" comes back newest first and holds
/// nearly all of them, but misses those whose URL starts with "http://", so
/// the timeline reads a search for each scheme side by side and interleaves
/// them by when they were bookmarked, showing a bookmark both hold once.
///
/// A search can only be read `BookmarkAPI.searchDepth` hits deep. Once one of
/// them runs into that, the other's older hits cannot be placed among the
/// ones it can no longer read, so the timeline ends there. A search that runs
/// out of hits instead lets the other carry on alone.
public struct MyBookmarkTimeline: MyBookmarkListing, Equatable {
    public static let queries = ["https", "http"]

    public private(set) var hits: [MyBookmarkSearchHit] = []
    /// Whether the timeline stopped short of the user's oldest bookmarks.
    public private(set) var isCutShort = false
    private var sources: [Source]
    private var shown: Set<String> = []

    public init() {
        sources = Self.queries.map { Source(query: $0) }
    }

    private struct Source: Equatable, Sendable {
        let query: String
        /// Hits read but not yet shown, newest first.
        var waiting: [MyBookmarkSearchHit] = []
        /// Everything this search has returned, to pull its offset back when
        /// one of them is deleted.
        var read: Set<String> = []
        var nextOffset = 0
        var total: Int?
        var reachedEnd = false

        var isExhausted: Bool {
            guard let total else { return false }
            return reachedEnd || nextOffset >= total
        }

        var isTooDeep: Bool {
            !isExhausted && nextOffset >= BookmarkAPI.searchDepth
        }

        var needsPage: Bool {
            waiting.isEmpty && !isExhausted && !isTooDeep
        }
    }

    public var nextRequest: MyBookmarkSearchRequest? {
        guard !isCutShort, let source = sources.first(where: \.needsPage) else { return nil }
        return MyBookmarkSearchRequest(query: source.query, offset: source.nextOffset)
    }

    public mutating func receive(_ page: MyBookmarkSearchPage, for request: MyBookmarkSearchRequest) {
        guard request == nextRequest,
              let index = sources.firstIndex(where: { $0.query == request.query })
        else { return }
        sources[index].total = page.total
        sources[index].nextOffset += page.hits.count
        sources[index].reachedEnd = page.hits.isEmpty
        sources[index].waiting += page.hits
        sources[index].read.formUnion(page.hits.map(\.id))
        merge()
    }

    /// Moves waiting hits into `hits`, newest first, for as long as every
    /// search either has some waiting or has none left to give.
    private mutating func merge() {
        while !sources.contains(where: \.needsPage) {
            if sources.contains(where: { $0.waiting.isEmpty && $0.isTooDeep }) {
                isCutShort = true
                return
            }
            let candidates = sources.indices.filter { !sources[$0].waiting.isEmpty }
            guard let newest = candidates.max(by: { date(of: $0) < date(of: $1) }) else { return }
            let hit = sources[newest].waiting.removeFirst()
            if shown.insert(hit.id).inserted {
                hits.append(hit)
            }
        }
    }

    private func date(of source: Int) -> Date {
        sources[source].waiting.first?.bookmarkedAt ?? .distantPast
    }

    public mutating func update(_ url: URL, with bookmark: MyBookmark) {
        if let index = hits.firstIndex(where: { $0.url == url }) {
            hits[index] = hits[index].updated(with: bookmark)
        }
        for source in sources.indices {
            sources[source].waiting = sources[source].waiting.map { $0.url == url ? $0.updated(with: bookmark) : $0 }
        }
    }

    public mutating func remove(_ url: URL) {
        hits.removeAll { $0.url == url }
        for source in sources.indices where sources[source].read.contains(url.absoluteString) {
            sources[source].waiting.removeAll { $0.url == url }
            sources[source].nextOffset = max(sources[source].nextOffset - 1, 0)
            sources[source].total = sources[source].total.map { max($0 - 1, 0) }
        }
    }
}
