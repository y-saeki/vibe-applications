import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The users the signed-in user has hidden ("非表示ユーザー"), whose comments
/// the website leaves out of an entry's comment list.
///
/// There is no official API for the list. This reads the JSON behind the
/// website's settings page, which needs the website's login cookies.
/// Undocumented and may change without notice. The shape was taken from
/// responses in 2026-11.
public struct IgnoredUsersAPI: Sendable {
    static let source = "unofficial.ignore-users"
    static let url = URL(string: "https://b.hatena.ne.jp/api/my/ignore_users")!
    /// Hatena returns the whole list at once when asked for this many; the
    /// cursor is still followed in case a list is longer.
    static let pageSize = 10_000
    /// Stops a cursor that never ends from looping forever.
    static let maxPages = 20

    private let http: HatenaHTTPClient
    private let cookieHeader: String?

    /// `cookies` are the website's, as a browser holds them; those that would
    /// not be sent to the endpoint are left out.
    public init(http: HatenaHTTPClient, cookies: [HTTPCookie]) {
        self.http = http
        cookieHeader = WebCookies.header(for: Self.url, from: cookies)
    }

    /// The hidden users' Hatena IDs. Empty when the user has hidden nobody.
    /// Throws `HatenaError.unauthorized` when the website's login is missing or
    /// has expired.
    public func users() async throws -> Set<String> {
        // Without a cookie Hatena can only answer "login required".
        guard let cookieHeader else { throw HatenaError.unauthorized }
        var users: Set<String> = []
        var cursor: String?
        for _ in 0..<Self.maxPages {
            var request = WebCookies.request(Self.url(cursor: cursor), header: cookieHeader)
            // The list changes when the user hides someone on the website.
            request.cachePolicy = .reloadIgnoringLocalCacheData
            let (data, response) = try await http.send(request)
            // Signed out, the endpoint answers 403 "login required".
            if response.statusCode == 403 {
                throw HatenaError.unauthorized
            }
            try HatenaHTTPClient.validate(response)
            let page = try JSONDecoder.hatena(Page.self, from: data, source: Self.source)
            users.formUnion(page.users)
            // The last page has no cursor.
            guard let next = page.cursor, !next.isEmpty, next != cursor, !page.users.isEmpty else { break }
            cursor = next
        }
        return users
    }

    static func url(cursor: String?) -> URL {
        var query = [("limit", String(pageSize))]
        if let cursor {
            query.append(("cursor", cursor))
        }
        return URL(string: url.absoluteString + "?" + PercentEncoding.encodeForm(query))!
    }
}

private struct Page: Decodable {
    let users: [String]
    let cursor: String?
}
