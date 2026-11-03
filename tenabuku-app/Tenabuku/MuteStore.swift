import EntryMute
import Foundation
import os

/// The user's mute rules, shared by the list that hides entries and the
/// screens that edit the rules. Saved to Application Support on every change.
@MainActor
@Observable
final class MuteStore {
    private(set) var list: MuteList
    /// Observed too, so that a view asking `isMuted` redraws when the rules change.
    private var filter: MuteFilter
    @ObservationIgnored private let file: MuteListFile
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "mute")

    init(file: MuteListFile = MuteStore.defaultFile) {
        self.file = file
        let loaded: MuteList
        do {
            loaded = try file.load()
        } catch {
            // Unlike the read history, the rules are the user's own work, so the
            // file is kept aside instead of being overwritten by the next save.
            Self.logger.error("mute rules unreadable: \(error.localizedDescription, privacy: .public)")
            do {
                let aside = try file.setAside()
                Self.logger.error("unreadable mute rules moved to \(aside.path, privacy: .public)")
            } catch {
                Self.logger.error("unreadable mute rules not moved: \(error.localizedDescription, privacy: .public)")
            }
            loaded = MuteList()
        }
        list = loaded
        filter = MuteFilter(loaded)
    }

    var rules: [MuteRule] { list.rules }

    func isMuted(title: String, url: URL) -> Bool {
        filter.isMuted(title: title, url: url)
    }

    func save(_ rule: MuteRule) {
        list.save(rule)
        persist()
    }

    func remove(id: UUID) {
        list.remove(id: id)
        persist()
    }

    private func persist() {
        filter = MuteFilter(list)
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
