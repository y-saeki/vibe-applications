import Foundation

/// The user's mute rules, in the order they were added. There is no upper
/// limit: every rule is compiled once into a `MuteFilter`, and testing a list
/// of hot entries against a few hundred of them stays well within a frame.
public struct MuteList: Codable, Equatable, Sendable {
    public private(set) var rules: [MuteRule]

    public init(rules: [MuteRule] = []) {
        self.rules = rules
    }

    /// Replaces the rule with the same id, or adds it at the end.
    public mutating func save(_ rule: MuteRule) {
        if let index = rules.firstIndex(where: { $0.id == rule.id }) {
            rules[index] = rule
        } else {
            rules.append(rule)
        }
    }

    public mutating func remove(id: UUID) {
        rules.removeAll { $0.id == id }
    }

    public mutating func removeAll() {
        rules = []
    }
}

/// Tests entries against every usable rule of a list. A rule that cannot be
/// used, such as a regular expression saved before it stopped compiling, is
/// left out rather than hiding everything or nothing.
public struct MuteFilter {
    private let matchers: [MuteMatcher]

    public init(_ list: MuteList) {
        matchers = list.rules.compactMap { try? MuteMatcher($0) }
    }

    public func isMuted(title: String, url: URL) -> Bool {
        matchers.contains { $0.matches(title: title, url: url) }
    }
}

/// Keeps a `MuteList` in one JSON file on the device.
public struct MuteListFile: Sendable {
    public let url: URL

    public init(url: URL) {
        self.url = url
    }

    /// An empty list when nothing has been saved yet.
    public func load() throws -> MuteList {
        guard FileManager.default.fileExists(atPath: url.path) else { return MuteList() }
        return try JSONDecoder().decode(MuteList.self, from: Data(contentsOf: url))
    }

    public func save(_ list: MuteList) throws {
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        try JSONEncoder().encode(list).write(to: url, options: .atomic)
    }

    /// Moves a file that could not be read out of the way, so that saving a
    /// new list does not overwrite rules that might still be recovered by hand.
    /// Returns where it went.
    @discardableResult
    public func setAside() throws -> URL {
        let destination = url.deletingPathExtension().appendingPathExtension("unreadable.json")
        if FileManager.default.fileExists(atPath: destination.path) {
            try FileManager.default.removeItem(at: destination)
        }
        try FileManager.default.moveItem(at: url, to: destination)
        return destination
    }
}
