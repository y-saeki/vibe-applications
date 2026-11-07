import Foundation
import HatenaShared

/// The site icon shown beside an entry, from the favicon service the Hatena
/// Bookmark website itself uses: `cdn-ak2.favicon.st-hatena.com/{size}?url=`
/// with the entry's URL. Asking Hatena rather than the site, or a third
/// party, means only Hatena learns which entries are on screen, and it
/// already does. Undocumented and may change without notice; taken from the
/// hot entry pages in 2026-11. A site it has no icon for answers 404 with a
/// placeholder image.
public enum Favicon {
    static let host = "https://cdn-ak2.favicon.st-hatena.com"
    /// The size the website asks for, which stays sharp at 16 points on a
    /// 3x screen.
    static let size = 64

    /// The icon of the site `page` belongs to, or nil for a URL that is not
    /// on the web.
    public static func url(for page: URL) -> URL? {
        guard let scheme = page.scheme?.lowercased(), scheme == "http" || scheme == "https",
              page.host() != nil
        else { return nil }
        return URL(string: "\(host)/\(size)?url=\(PercentEncoding.encode(page.absoluteString))")
    }
}
