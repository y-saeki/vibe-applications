import Foundation

/// Keeps the user's bookmark timeline on disk, so that the list can be shown
/// at once instead of after the seconds each search takes, and read on from
/// where it was. One account's timeline at a time.
///
/// It holds private bookmarks, so it is thrown away on signing out. It lives
/// in Caches, which the system may empty; the list then reads from Hatena
/// again.
public actor MyBookmarkTimelineStore {
    public struct Saved: Equatable, Sendable, Codable {
        /// The Hatena ID the timeline belongs to.
        public let user: String
        public let savedAt: Date
        public let timeline: MyBookmarkTimeline
        /// What the file was written as. A timeline saved by an earlier
        /// version lacks what the list now shows, so it is read again
        /// rather than shown.
        let format: Int?

        public init(user: String, savedAt: Date, timeline: MyBookmarkTimeline) {
            self.user = user
            self.savedAt = savedAt
            self.timeline = timeline
            format = Self.currentFormat
        }

        /// 2 keeps each bookmark's entry ID, which its stars are read by.
        static let currentFormat = 2
    }

    private let url: URL

    public init(url: URL) {
        self.url = url
    }

    /// The saved timeline of `user`, or nil when there is none, it belongs to
    /// someone else, it was saved in an earlier format, or it cannot be read.
    public func load(user: String) -> Saved? {
        guard let data = try? Data(contentsOf: url),
              let saved = try? JSONDecoder().decode(Saved.self, from: data),
              saved.user == user,
              saved.format == Saved.currentFormat
        else { return nil }
        return saved
    }

    public func save(_ saved: Saved) throws {
        try FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        #if canImport(Darwin)
        // Private bookmarks stay unreadable while the device is locked.
        let options: Data.WritingOptions = [.atomic, .completeFileProtection]
        #else
        let options: Data.WritingOptions = [.atomic]
        #endif
        try JSONEncoder().encode(saved).write(to: url, options: options)
    }

    public func clear() throws {
        guard FileManager.default.fileExists(atPath: url.path) else { return }
        try FileManager.default.removeItem(at: url)
    }
}
