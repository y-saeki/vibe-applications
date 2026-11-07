import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// A service the site icon beside an entry can come from. None of them is
/// documented, and each may change without notice; their behaviour was taken
/// in 2026-11.
public enum FaviconSource: String, Codable, CaseIterable, Sendable {
    /// The service the Hatena Bookmark website itself uses, asked with the
    /// entry's URL. Hatena already knows which entries are listed.
    case hatena
    /// DuckDuckGo's icon service, asked with the host only, so it gets the
    /// site-wide icon rather than one a page declares for itself.
    case duckDuckGo
    /// Google's icon service, asked with the entry's URL. It honours the
    /// page's own icon and converts SVG icons, which Hatena cannot.
    case google

    /// The size asked for, which stays sharp at 13 points on a 3x screen.
    static let size = 64

    /// Where to ask for the icon of `page`, or nil for a URL that is not on
    /// the web.
    public func url(for page: URL) -> URL? {
        guard let scheme = page.scheme?.lowercased(), scheme == "http" || scheme == "https",
              let host = page.host()?.lowercased()
        else { return nil }
        let pageURL = PercentEncoding.encode(page.absoluteString)
        switch self {
        case .hatena:
            return URL(string: "https://cdn-ak2.favicon.st-hatena.com/\(Self.size)?url=\(pageURL)")
        case .duckDuckGo:
            return URL(string: "https://icons.duckduckgo.com/ip3/\(PercentEncoding.encode(host)).ico")
        case .google:
            return URL(string: "https://www.google.com/s2/favicons?domain_url=\(pageURL)&sz=\(Self.size)")
        }
    }

    /// Whether an answer carries an icon. Every service sends a stand-in
    /// image with a 404. Hatena also sends its stand-in with a 200 when it
    /// found an icon it could not convert (an SVG icon, say), so for Hatena
    /// the bytes are compared too.
    func isIcon(_ data: Data, status: Int) -> Bool {
        guard (200..<300).contains(status), !data.isEmpty else { return false }
        return self != .hatena || data != Self.hatenaPlaceholder
    }

    /// The grey page Hatena sends instead of an icon, a 16 x 16 PNG.
    static let hatenaPlaceholder = Data(base64Encoded: """
        iVBORw0KGgoAAAANSUhEUgAAABAAAAAQEAYAAABPYyMiAAAABGdBTUEAAK/INwWK6QAAAAZiS0dE////////CVj33AAAAAlwSFlzAAAASAAAAEgA\
        RslrPgAAAAl2cEFnAAAAEAAAABAAXMatwwAAAMNJREFUSMe1jtFmQ1EUBe8nR5RQSh9KKaVEVf+mn1GlhJKHUkoJETFhWC+p7TrszMu45zrrzIRM\
        U/wq0O3zd+J/By8CRxn3+f18x7MBzwIHGXd1P+dxGbAW2Mu45+7nfxnwJLCTcef+nMuAR4E/Gfeb1M5+GfAg8Cv9zn4ZcC/wI/3OfhlwJ/At/c5+\
        GXArsJV+Z78MuBH4kn5nvwy4FthIv7NfBqwEPqXf2S8DrgQ+pN/ZLwOWAu/S7+yXAQuBSzvvnQAm/poVaYyJawAAACJ6VFh0U29mdHdhcmUAAHja\
        c0zJT0pV8MxNTE8NSk1MqQQAL5wF1K4MqU0AAAAASUVORK5CYII=
        """)!
}

/// Which services to ask for site icons, and in what order: the setting the
/// user changes.
public struct FaviconPreferences: Codable, Equatable, Sendable {
    public struct Entry: Codable, Equatable, Sendable {
        public var source: FaviconSource
        public var isEnabled: Bool

        public init(source: FaviconSource, isEnabled: Bool) {
            self.source = source
            self.isEnabled = isEnabled
        }
    }

    /// Every source exactly once, in the order they are asked.
    public private(set) var entries: [Entry]

    /// Hatena first; DuckDuckGo listed but off, since it gives the site-wide
    /// icon where Google gives the page's own; Google last.
    public static let `default` = FaviconPreferences(entries: [
        Entry(source: .hatena, isEnabled: true),
        Entry(source: .duckDuckGo, isEnabled: false),
        Entry(source: .google, isEnabled: true),
    ])

    /// Keeps the first entry for each source and appends, switched off, any
    /// source missing from `entries` (one added in a later version, say).
    public init(entries: [Entry]) {
        var seen = Set<FaviconSource>()
        var kept = entries.filter { seen.insert($0.source).inserted }
        for source in FaviconSource.allCases where !seen.contains(source) {
            kept.append(Entry(source: source, isEnabled: false))
        }
        self.entries = kept
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        // A source this version does not know is skipped, not a failure.
        let raw = try container.decode([LossyEntry].self, forKey: .entries)
        self.init(entries: raw.compactMap(\.entry))
    }

    private struct LossyEntry: Decodable {
        let entry: Entry?
        init(from decoder: Decoder) throws {
            entry = try? Entry(from: decoder)
        }
    }

    /// The sources to ask, in order.
    public var enabledSources: [FaviconSource] {
        entries.filter(\.isEnabled).map(\.source)
    }

    public mutating func setEnabled(_ isEnabled: Bool, for source: FaviconSource) {
        guard let index = entries.firstIndex(where: { $0.source == source }) else { return }
        entries[index].isEnabled = isEnabled
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
}

/// Fetches the site icon of an entry, asking each source in turn.
public struct FaviconClient: Sendable {
    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The icon's image data, in whatever format the service sent (PNG,
    /// ICO, JPEG), from the first of `sources` that has one; nil when none
    /// has or none could be reached. Never throws: a missing icon is not
    /// worth an error.
    public func icon(for page: URL, from sources: [FaviconSource]) async -> Data? {
        for source in sources {
            guard let url = source.url(for: page),
                  let (data, response) = try? await http.send(URLRequest(url: url))
            else { continue }
            if source.isIcon(data, status: response.statusCode) {
                return data
            }
        }
        return nil
    }
}
