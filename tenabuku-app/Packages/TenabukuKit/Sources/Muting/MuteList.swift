import Foundation

/// The user's mute rules, in the order they were added. There is no cap on
/// how many: each is compiled once by `MuteFilter`, and asking a few dozen
/// entries against even hundreds of rules costs little.
public struct MuteList: Codable, Equatable, Sendable {
    public var rules: [MuteRule]

    public init(rules: [MuteRule] = []) {
        self.rules = rules
    }

    /// Replaces the rule with the same id, or adds it at the end.
    public mutating func upsert(_ rule: MuteRule) {
        if let index = rules.firstIndex(where: { $0.id == rule.id }) {
            rules[index] = rule
        } else {
            rules.append(rule)
        }
    }

    public mutating func remove(id: MuteRule.ID) {
        rules.removeAll { $0.id == id }
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
}
