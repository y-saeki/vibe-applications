import Foundation
import HatenaFeed

/// One shelf of the home screen: a hot entry category, or the user's own
/// My hot entries.
public enum HomeShelf: Hashable, Sendable {
    case hotEntry(HotEntryCategory)
    case myHotEntry

    /// Every shelf, in the order a new install shows them.
    public static let allCases: [HomeShelf] = HotEntryCategory.allCases.map(Self.hotEntry) + [.myHotEntry]

    /// How the shelf is written to disk: the category's own name in its feed
    /// URL, so that a renamed case does not lose the user's order.
    public var storageKey: String {
        switch self {
        case let .hotEntry(category): category.rawValue
        case .myHotEntry: "myhotentry"
        }
    }

    public init?(storageKey: String) {
        if storageKey == "myhotentry" {
            self = .myHotEntry
        } else if let category = HotEntryCategory(rawValue: storageKey) {
            self = .hotEntry(category)
        } else {
            return nil
        }
    }
}

/// Which shelves the home screen shows, and in what order.
public struct ShelfPreferences: Codable, Equatable, Sendable {
    public struct Entry: Equatable, Sendable {
        public var shelf: HomeShelf
        public var isShown: Bool

        public init(shelf: HomeShelf, isShown: Bool) {
            self.shelf = shelf
            self.isShown = isShown
        }
    }

    /// Every shelf exactly once, in the order they are shown.
    public private(set) var entries: [Entry]

    /// Every shelf, shown, in the order of Hatena's own category menu.
    public static let `default` = ShelfPreferences(entries: HomeShelf.allCases.map { Entry(shelf: $0, isShown: true) })

    /// Keeps the first entry for each shelf and appends, shown, any shelf
    /// missing from `entries`: one a later version added should not stay
    /// out of sight until the user goes looking for it.
    public init(entries: [Entry]) {
        var seen = Set<HomeShelf>()
        var kept = entries.filter { seen.insert($0.shelf).inserted }
        for shelf in HomeShelf.allCases where !seen.contains(shelf) {
            kept.append(Entry(shelf: shelf, isShown: true))
        }
        self.entries = kept
    }

    /// The shelves to show, in order.
    public var shownShelves: [HomeShelf] {
        entries.filter(\.isShown).map(\.shelf)
    }

    public mutating func setShown(_ isShown: Bool, for shelf: HomeShelf) {
        guard let index = entries.firstIndex(where: { $0.shelf == shelf }) else { return }
        entries[index].isShown = isShown
    }

    /// Same contract as `Array.move(fromOffsets:toOffset:)`, which SwiftUI's
    /// `onMove` hands over.
    public mutating func move(fromOffsets source: IndexSet, toOffset destination: Int) {
        let moving = source.sorted().map { entries[$0] }
        var rest = entries.enumerated().filter { !source.contains($0.offset) }.map(\.element)
        let insertAt = destination - source.filter { $0 < destination }.count
        rest.insert(contentsOf: moving, at: insertAt)
        entries = rest
    }

    // MARK: Codable

    private enum CodingKeys: String, CodingKey {
        case entries
    }

    private struct StoredEntry: Codable {
        let shelf: String
        let isShown: Bool
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let stored = try container.decode([StoredEntry].self, forKey: .entries)
        // A shelf this version does not know is skipped, not a failure.
        self.init(entries: stored.compactMap { entry in
            HomeShelf(storageKey: entry.shelf).map { Entry(shelf: $0, isShown: entry.isShown) }
        })
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(entries.map { StoredEntry(shelf: $0.shelf.storageKey, isShown: $0.isShown) }, forKey: .entries)
    }
}
