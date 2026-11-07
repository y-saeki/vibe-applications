import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// Where the site icon beside an entry comes from.
///
/// First the favicon service the Hatena Bookmark website itself uses,
/// `cdn-ak2.favicon.st-hatena.com/{size}?url=` with the entry's URL, so that
/// for most entries only Hatena learns what is on screen, and it already
/// does. When Hatena has no icon, DuckDuckGo's icon service is asked for the
/// site's host (never the full URL). Both are undocumented and may change
/// without notice; their behaviour was taken in 2026-11.
public enum Favicon {
    static let hatenaHost = "https://cdn-ak2.favicon.st-hatena.com"
    static let duckDuckGoHost = "https://icons.duckduckgo.com"
    /// The size the Hatena website asks for, which stays sharp at 13 points
    /// on a 3x screen.
    static let size = 64

    /// Hatena's icon for the site `page` belongs to, or nil for a URL that is
    /// not on the web.
    public static func url(for page: URL) -> URL? {
        guard isWeb(page) else { return nil }
        return URL(string: "\(hatenaHost)/\(size)?url=\(PercentEncoding.encode(page.absoluteString))")
    }

    /// DuckDuckGo's icon for the host of `page`.
    public static func fallbackURL(for page: URL) -> URL? {
        guard isWeb(page), let host = page.host()?.lowercased() else { return nil }
        return URL(string: "\(duckDuckGoHost)/ip3/\(PercentEncoding.encode(host)).ico")
    }

    private static func isWeb(_ page: URL) -> Bool {
        guard let scheme = page.scheme?.lowercased() else { return false }
        return (scheme == "http" || scheme == "https") && page.host() != nil
    }

    /// The grey page Hatena sends instead of an icon. It comes with 404 when
    /// Hatena finds no icon, but with 200 when it found one it could not
    /// convert (an SVG favicon, say), so the bytes are what tell it apart.
    static let hatenaPlaceholder = Data(base64Encoded: """
        iVBORw0KGgoAAAANSUhEUgAAABAAAAAQEAYAAABPYyMiAAAABGdBTUEAAK/INwWK6QAAAAZiS0dE////////CVj33AAAAAlwSFlzAAAASAAAAEgA\
        RslrPgAAAAl2cEFnAAAAEAAAABAAXMatwwAAAMNJREFUSMe1jtFmQ1EUBe8nR5RQSh9KKaVEVf+mn1GlhJKHUkoJETFhWC+p7TrszMu45zrrzIRM\
        U/wq0O3zd+J/By8CRxn3+f18x7MBzwIHGXd1P+dxGbAW2Mu45+7nfxnwJLCTcef+nMuAR4E/Gfeb1M5+GfAg8Cv9zn4ZcC/wI/3OfhlwJ/At/c5+\
        GXArsJV+Z78MuBH4kn5nvwy4FthIv7NfBqwEPqXf2S8DrgQ+pN/ZLwOWAu/S7+yXAQuBSzvvnQAm/poVaYyJawAAACJ6VFh0U29mdHdhcmUAAHja\
        c0zJT0pV8MxNTE8NSk1MqQQAL5wF1K4MqU0AAAAASUVORK5CYII=
        """)!
}

/// Fetches the site icon of an entry, from Hatena and then DuckDuckGo.
public struct FaviconClient: Sendable {
    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The icon's image data, in whatever format the service sent (PNG,
    /// ICO, JPEG), or nil when neither service has one or neither could be
    /// reached. Never throws: a missing icon is not worth an error.
    public func icon(for page: URL) async -> Data? {
        if let url = Favicon.url(for: page),
           let data = await fetch(url),
           data != Favicon.hatenaPlaceholder {
            return data
        }
        if let url = Favicon.fallbackURL(for: page) {
            return await fetch(url)
        }
        return nil
    }

    /// The body of a 2xx answer. DuckDuckGo, like Hatena, sends a stand-in
    /// image with its 404, which is dropped here.
    private func fetch(_ url: URL) async -> Data? {
        guard let (data, response) = try? await http.send(URLRequest(url: url)),
              (200..<300).contains(response.statusCode),
              !data.isEmpty
        else { return nil }
        return data
    }
}
