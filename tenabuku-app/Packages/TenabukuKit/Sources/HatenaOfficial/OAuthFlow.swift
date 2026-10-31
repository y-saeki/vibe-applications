import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

public enum OAuthScope: String, Sendable, CaseIterable {
    case readPublic = "read_public"
    case writePublic = "write_public"
    case readPrivate = "read_private"
    case writePrivate = "write_private"
}

/// What a completed sign-in yields.
public struct AccessGrant: Equatable, Sendable {
    public let token: OAuthToken
    /// The Hatena ID.
    public let urlName: String
    public let displayName: String
}

/// The three-legged OAuth 1.0a sign-in, following
/// https://developer.hatena.ne.jp/ja/documents/auth/apis/oauth/consumer
///
/// 1. `requestToken` gets a request token.
/// 2. The user approves it at `authorizationURL`; Hatena redirects to the
///    callback with `oauth_verifier`.
/// 3. `accessToken` trades the two for an access token.
public struct OAuthFlow: Sendable {
    public static let initiateURL = URL(string: "https://www.hatena.com/oauth/initiate")!
    public static let tokenURL = URL(string: "https://www.hatena.com/oauth/token")!
    /// The smartphone variant of the approval page.
    public static let authorizeURL = URL(string: "https://www.hatena.ne.jp/touch/oauth/authorize")!

    private let http: HatenaHTTPClient
    private let signer: OAuthSigner

    public init(http: HatenaHTTPClient, signer: OAuthSigner) {
        self.http = http
        self.signer = signer
    }

    public func requestToken(callback: String, scopes: [OAuthScope]) async throws -> OAuthToken {
        let request = signer.request(
            method: "POST",
            url: Self.initiateURL,
            form: [("scope", scopes.map(\.rawValue).joined(separator: ","))],
            token: nil,
            oauthExtras: [("oauth_callback", callback)]
        )
        let fields = try await formResponse(request)
        guard let token = fields["oauth_token"], let secret = fields["oauth_token_secret"] else {
            throw HatenaError.parseFailed(source: "oauth.initiate", detail: "no oauth_token in \(fields.keys.sorted())")
        }
        return OAuthToken(token: token, secret: secret)
    }

    public func authorizationURL(for requestToken: OAuthToken) -> URL {
        URL(string: Self.authorizeURL.absoluteString + "?oauth_token=" + PercentEncoding.encode(requestToken.token))!
    }

    public func accessToken(for requestToken: OAuthToken, verifier: String) async throws -> AccessGrant {
        let request = signer.request(
            method: "POST",
            url: Self.tokenURL,
            token: requestToken,
            oauthExtras: [("oauth_verifier", verifier)]
        )
        let fields = try await formResponse(request)
        guard let token = fields["oauth_token"], let secret = fields["oauth_token_secret"],
              let urlName = fields["url_name"]
        else {
            throw HatenaError.parseFailed(source: "oauth.token", detail: "missing fields in \(fields.keys.sorted())")
        }
        return AccessGrant(
            token: OAuthToken(token: token, secret: secret),
            urlName: urlName,
            displayName: fields["display_name"] ?? urlName
        )
    }

    private func formResponse(_ request: URLRequest) async throws -> [String: String] {
        let data = try await http.data(for: request)
        let pairs = PercentEncoding.decodeForm(String(decoding: data, as: UTF8.self))
        return Dictionary(pairs, uniquingKeysWith: { first, _ in first })
    }
}
