import Foundation
import Observation
import HatenaFeed
import HatenaShared
import HomeShelves

/// What one shelf has loaded so far.
struct ShelfFeed {
    var items: [FeedItem] = []
    var error: Error?
    var isLoading = false
    /// Whether `items` came back at least once, which tells an empty
    /// My hot entries apart from one still on its way.
    var hasLoaded = false
    /// Set when My hot entries needs the website login.
    var needsWebSignIn = false
}

/// The entries of every shelf, kept for as long as the app runs. A shelf is
/// read the first time it comes on screen and kept from then on, so that
/// scrolling back to it shows it at once; pulling the home screen down reads
/// again every shelf already read. A reload keeps the entries on screen
/// until the new ones arrive, and keeps them when it fails.
@MainActor
@Observable
final class ShelfFeedStore {
    private(set) var feeds: [HomeShelf: ShelfFeed] = [:]

    subscript(shelf: HomeShelf) -> ShelfFeed {
        feeds[shelf] ?? ShelfFeed()
    }

    /// Reads the shelf unless it has been read, or is being read, already.
    func loadIfNeeded(_ shelf: HomeShelf, webSession: WebSessionStore) async {
        let feed = self[shelf]
        guard !feed.hasLoaded, !feed.isLoading else { return }
        await load(shelf, webSession: webSession)
    }

    /// Reads again every shelf that has been read, all at once.
    func reloadLoaded(webSession: WebSessionStore) async {
        let shelves = feeds.filter { $0.value.hasLoaded }.map(\.key)
        await withTaskGroup(of: Void.self) { group in
            for shelf in shelves {
                group.addTask { await self.load(shelf, webSession: webSession) }
            }
        }
    }

    func load(_ shelf: HomeShelf, webSession: WebSessionStore) async {
        guard !self[shelf].isLoading else { return }
        feeds[shelf, default: ShelfFeed()].isLoading = true
        defer { feeds[shelf]?.isLoading = false }
        do {
            let items = try await Self.fetch(shelf, webSession: webSession)
            feeds[shelf]?.items = items
            feeds[shelf]?.error = nil
            feeds[shelf]?.needsWebSignIn = false
        } catch HatenaError.unauthorized where shelf == .myHotEntry {
            feeds[shelf]?.items = []
            feeds[shelf]?.error = nil
            feeds[shelf]?.needsWebSignIn = true
        } catch {
            FailureLog.record(error)
            feeds[shelf]?.error = error
            feeds[shelf]?.needsWebSignIn = false
        }
        feeds[shelf]?.hasLoaded = true
    }

    private static func fetch(_ shelf: HomeShelf, webSession: WebSessionStore) async throws -> [FeedItem] {
        switch shelf {
        case let .hotEntry(category):
            return try await AppServices.feed.hotEntries(category)
        case .myHotEntry:
            return try await webSession.myHotEntries().map { entry in
                FeedItem(
                    title: entry.title,
                    link: entry.url,
                    summary: entry.summary,
                    date: entry.date,
                    bookmarkCount: entry.bookmarkCount,
                    imageURL: entry.imageURL
                )
            }
        }
    }
}
