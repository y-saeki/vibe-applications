import Foundation
import os
import ReadingHistory

/// Which entries the user has opened, shared by the list that dims them and the
/// browser that marks them. Saved to Application Support on every change, and
/// trimmed to the last 30 days at launch.
@MainActor
@Observable
final class ReadHistoryStore {
    private(set) var history: ReadHistory
    @ObservationIgnored private let file: ReadHistoryFile
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "read-history")

    init(file: ReadHistoryFile = ReadHistoryStore.defaultFile) {
        self.file = file
        do {
            history = try file.load()
        } catch {
            // A file that cannot be read is started over rather than blocking the app.
            Self.logger.error("read history unreadable: \(error.localizedDescription, privacy: .public)")
            history = ReadHistory()
        }
        history.prune()
        save()
    }

    func isRead(_ url: URL) -> Bool {
        history.contains(url)
    }

    func markRead(_ url: URL) {
        history.markRead(url)
        save()
    }

    func removeAll() {
        history.removeAll()
        save()
    }

    private func save() {
        do {
            try file.save(history)
        } catch {
            Self.logger.error("read history not saved: \(error.localizedDescription, privacy: .public)")
        }
    }

    private nonisolated static var defaultFile: ReadHistoryFile {
        let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return ReadHistoryFile(url: directory.appendingPathComponent("ReadHistory.json"))
    }
}
