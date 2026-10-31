import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The signed-in user's own bookmarks, through the documented OAuth APIs:
/// https://developer.hatena.ne.jp/ja/documents/bookmark/apis/rest/
/// and the full-text search of one's own bookmarks:
/// https://developer.hatena.ne.jp/ja/documents/bookmark/apis/fulltext_search
public struct BookmarkAPI: Sendable {
    static let restBase = "https://bookmark.hatenaapis.com/rest/1"
    static let searchURL = "https://b.hatena.ne.jp/my/search/json"

    private let http: HatenaHTTPClient
    private let signer: OAuthSigner
    private let token: OAuthToken

    public init(http: HatenaHTTPClient, signer: OAuthSigner, token: OAuthToken) {
        self.http = http
        self.signer = signer
        self.token = token
    }

    /// The user's bookmark of `url`, or nil when there is none.
    public func bookmark(for url: URL) async throws -> MyBookmark? {
        let request = signed("GET", Self.url(Self.restBase + "/my/bookmark", [("url", url.absoluteString)]))
        let (data, response) = try await http.send(request)
        if response.statusCode == 404 {
            return nil
        }
        try HatenaHTTPClient.validate(response)
        return try JSONDecoder.hatena(MyBookmark.self, from: data, source: "official.my-bookmark")
    }

    /// Adds the bookmark, or replaces it when `url` is already bookmarked.
    @discardableResult
    public func save(_ draft: BookmarkDraft) async throws -> MyBookmark {
        var form: [(String, String)] = [("url", draft.url.absoluteString), ("comment", draft.comment)]
        form += draft.tags.map { ("tags", $0) }
        form.append(("private", draft.isPrivate ? "1" : "0"))
        let request = signed("POST", URL(string: Self.restBase + "/my/bookmark")!, form: form)
        let data = try await http.data(for: request)
        return try JSONDecoder.hatena(MyBookmark.self, from: data, source: "official.my-bookmark")
    }

    /// Deletes the bookmark. Deleting one that does not exist is not an error.
    public func delete(_ url: URL) async throws {
        let request = signed("DELETE", Self.url(Self.restBase + "/my/bookmark", [("url", url.absoluteString)]))
        let (_, response) = try await http.send(request)
        if response.statusCode == 404 {
            return
        }
        try HatenaHTTPClient.validate(response)
    }

    public func me() async throws -> HatenaUser {
        let data = try await http.data(for: signed("GET", URL(string: Self.restBase + "/my")!))
        return try JSONDecoder.hatena(HatenaUser.self, from: data, source: "official.my")
    }

    /// Full-text search over the user's bookmarks (title, body, URL, comment).
    /// `limit` is at most 100.
    public func search(_ query: String, offset: Int = 0, limit: Int = 20) async throws -> MyBookmarkSearchPage {
        let url = Self.url(Self.searchURL, [("q", query), ("of", String(offset)), ("limit", String(min(limit, 100)))])
        let data = try await http.data(for: signed("GET", url))
        return try JSONDecoder.hatena(MyBookmarkSearchPage.self, from: data, source: "official.my-search")
    }

    private func signed(_ method: String, _ url: URL, form: [(String, String)] = []) -> URLRequest {
        signer.request(method: method, url: url, form: form, token: token)
    }

    static func url(_ base: String, _ query: [(String, String)]) -> URL {
        URL(string: base + "?" + PercentEncoding.encodeForm(query))!
    }
}

public struct BookmarkDraft: Equatable, Sendable {
    public var url: URL
    public var comment: String
    /// At most 10, which the API enforces.
    public var tags: [String]
    public var isPrivate: Bool

    public init(url: URL, comment: String = "", tags: [String] = [], isPrivate: Bool = false) {
        self.url = url
        self.comment = comment
        self.tags = tags
        self.isPrivate = isPrivate
    }
}
