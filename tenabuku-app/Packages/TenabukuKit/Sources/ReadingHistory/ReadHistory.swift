import Foundation

/// The entries the user has opened, keyed by the entry's URL, with when each
/// was last opened. Only the last 30 days are kept: a hot entry falls off the
/// list well within that, and a month is long enough for feeds that move
/// slower. Older marks are dropped by `prune(now:)`.
public struct ReadHistory: Codable, Equatable, Sendable {
    public static let retention: TimeInterval = 30 * 24 * 60 * 60

    private var openedAt: [String: Date]

    public init() {
        openedAt = [:]
    }

    public var count: Int { openedAt.count }

    public func contains(_ url: URL) -> Bool {
        openedAt[url.absoluteString] != nil
    }

    /// Opening an entry again restarts its 30 days.
    public mutating func markRead(_ url: URL, at date: Date = Date()) {
        openedAt[url.absoluteString] = date
    }

    public mutating func prune(now: Date = Date()) {
        let cutoff = now.addingTimeInterval(-Self.retention)
        openedAt = openedAt.filter { $0.value >= cutoff }
    }

    public mutating func removeAll() {
        openedAt = [:]
    }
}

/// Keeps a `ReadHistory` in one JSON file on the device.
public struct ReadHistoryFile: Sendable {
    public let url: URL

    public init(url: URL) {
        self.url = url
    }

    /// An empty history when nothing has been saved yet.
    public func load() throws -> ReadHistory {
        guard FileManager.default.fileExists(atPath: url.path) else { return ReadHistory() }
        return try JSONDecoder().decode(ReadHistory.self, from: Data(contentsOf: url))
    }

    public func save(_ history: ReadHistory) throws {
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try JSONEncoder().encode(history).write(to: url, options: .atomic)
    }
}
