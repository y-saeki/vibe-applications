import Foundation
import HatenaFeed
import HomeShelves
import os

/// Which shelves the home screen shows, and in what order. Kept in
/// UserDefaults, like the site icon settings.
@MainActor
@Observable
final class ShelfSettingsStore {
    private(set) var preferences: ShelfPreferences

    @ObservationIgnored private let defaults: UserDefaults
    private static let key = "shelfPreferences"
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "shelves")

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: Self.key) {
            do {
                preferences = try JSONDecoder().decode(ShelfPreferences.self, from: data)
            } catch {
                Self.logger.error("shelf settings unreadable: \(error.localizedDescription, privacy: .public)")
                preferences = .default
            }
        } else {
            preferences = .default
        }
    }

    var shownShelves: [HomeShelf] { preferences.shownShelves }

    func setShown(_ isShown: Bool, for shelf: HomeShelf) {
        update { $0.setShown(isShown, for: shelf) }
    }

    func move(fromOffsets source: IndexSet, toOffset destination: Int) {
        update { $0.move(fromOffsets: source, toOffset: destination) }
    }

    func reset() {
        update { $0 = .default }
    }

    private func update(_ change: (inout ShelfPreferences) -> Void) {
        var changed = preferences
        change(&changed)
        guard changed != preferences else { return }
        preferences = changed
        do {
            defaults.set(try JSONEncoder().encode(changed), forKey: Self.key)
        } catch {
            Self.logger.error("shelf settings not saved: \(error.localizedDescription, privacy: .public)")
        }
    }
}

extension HomeShelf {
    /// The name shown over the shelf.
    var displayName: String {
        switch self {
        case let .hotEntry(category): category.displayName
        case .myHotEntry: "マイホットエントリー"
        }
    }
}
