import Foundation
import HatenaUnofficial
import os

/// Which services site icons come from, and in what order. Kept in
/// UserDefaults; changing it forgets the icons fetched so far, so that every
/// row asks again under the new order.
@MainActor
@Observable
final class FaviconSettingsStore {
    private(set) var preferences: FaviconPreferences

    @ObservationIgnored private let defaults: UserDefaults
    private static let key = "faviconPreferences"
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "favicon")

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        if let data = defaults.data(forKey: Self.key) {
            do {
                preferences = try JSONDecoder().decode(FaviconPreferences.self, from: data)
            } catch {
                Self.logger.error("favicon settings unreadable: \(error.localizedDescription, privacy: .public)")
                preferences = .default
            }
        } else {
            preferences = .default
        }
    }

    var enabledSources: [FaviconSource] { preferences.enabledSources }

    func setEnabled(_ isEnabled: Bool, for source: FaviconSource) {
        update { $0.setEnabled(isEnabled, for: source) }
    }

    func move(fromOffsets source: IndexSet, toOffset destination: Int) {
        update { $0.move(fromOffsets: source, toOffset: destination) }
    }

    func reset() {
        update { $0 = .default }
    }

    private func update(_ change: (inout FaviconPreferences) -> Void) {
        var changed = preferences
        change(&changed)
        guard changed != preferences else { return }
        preferences = changed
        FaviconCache.shared.removeAll()
        do {
            defaults.set(try JSONEncoder().encode(changed), forKey: Self.key)
        } catch {
            Self.logger.error("favicon settings not saved: \(error.localizedDescription, privacy: .public)")
        }
    }
}

extension FaviconSource {
    /// The name shown in Settings.
    var displayName: String {
        switch self {
        case .hatena: "はてな"
        case .duckDuckGo: "DuckDuckGo"
        case .google: "Google"
        }
    }

    /// What is sent, in a line under the name.
    var sentDescription: String {
        switch self {
        case .hatena: "記事の URL を送ります"
        case .duckDuckGo: "ドメインだけを送ります。サイト全体のアイコンになります"
        case .google: "記事の URL を送ります。ページごとのアイコンや SVG のアイコンも出ます"
        }
    }
}
