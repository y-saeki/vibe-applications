import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The image Hatena shows for an entry, the one the hot entry feeds carry as
/// `hatena:imageurl`, for a page known only by its URL.
///
/// None of the documented APIs has it: the entry info API's `screenshot` is a
/// placeholder for every page. This reads the JSON the website's entry page
/// loads, which needs no sign-in. Undocumented and may change without notice.
/// The shape was taken from responses in 2026-11.
public struct EntryImageAPI: Sendable {
    static let source = "unofficial.entry-info"

    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The entry's image, or nil when it has none or nobody has bookmarked
    /// the page.
    public func imageURL(for url: URL) async throws -> URL? {
        let (data, response) = try await http.send(URLRequest(url: Self.url(for: url)))
        // An unbookmarked URL is answered with 404.
        if response.statusCode == 404 {
            return nil
        }
        try HatenaHTTPClient.validate(response)
        let info = try JSONDecoder.hatena(Info.self, from: data, source: Self.source)
        return info.image.flatMap { $0.isEmpty ? nil : URL(string: $0) }
    }

    static func url(for url: URL) -> URL {
        URL(string: "https://b.hatena.ne.jp/api/entry/info?" + PercentEncoding.encodeForm([("url", url.absoluteString)]))!
    }

    private struct Info: Decodable {
        let image: String?
    }
}
