import Foundation
import HatenaFeed
import Muting
import os

/// The user's mute rules, shared by the list that hides entries and the
/// settings that edit the rules. Saved to Application Support on every change.
@MainActor
@Observable
final class MuteStore {
    private(set) var list: MuteList
    /// Observed too, so that a list asking `isMuted` redraws when rules change.
    private var filter: MuteFilter
    @ObservationIgnored private let file: MuteListFile
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "mute")

    init(file: MuteListFile = MuteStore.defaultFile) {
        self.file = file
        let loaded: MuteList
        do {
            loaded = try file.load()
        } catch {
            // A file that cannot be read is started over rather than blocking the app.
            Self.logger.error("mute rules unreadable: \(error.localizedDescription, privacy: .public)")
            loaded = MuteList()
        }
        list = loaded
        filter = MuteFilter(rules: loaded.rules)
    }

    var rules: [MuteRule] { list.rules }

    func isMuted(_ item: FeedItem) -> Bool {
        filter.isMuted(title: item.title, url: item.link)
    }

    func save(_ rule: MuteRule) {
        list.upsert(rule)
        changed()
    }

    func remove(id: MuteRule.ID) {
        list.remove(id: id)
        changed()
    }

    private func changed() {
        filter = MuteFilter(rules: list.rules)
        do {
            try file.save(list)
        } catch {
            Self.logger.error("mute rules not saved: \(error.localizedDescription, privacy: .public)")
        }
    }

    private nonisolated static var defaultFile: MuteListFile {
        let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return MuteListFile(url: directory.appendingPathComponent("MuteRules.json"))
    }
}
