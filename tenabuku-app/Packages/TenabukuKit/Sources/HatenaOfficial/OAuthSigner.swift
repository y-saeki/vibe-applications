import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

public struct OAuthConsumer: Equatable, Sendable {
    public let key: String
    public let secret: String

    public init(key: String, secret: String) {
        self.key = key
        self.secret = secret
    }
}

/// A request token or an access token, with its secret.
public struct OAuthToken: Equatable, Sendable, Codable {
    public let token: String
    public let secret: String

    public init(token: String, secret: String) {
        self.token = token
        self.secret = secret
    }
}

/// Signs requests with OAuth 1.0a HMAC-SHA1 (RFC 5849), as Hatena requires.
public struct OAuthSigner: Sendable {
    public let consumer: OAuthConsumer
    private let nonce: @Sendable () -> String
    private let now: @Sendable () -> Date

    public init(
        consumer: OAuthConsumer,
        nonce: @escaping @Sendable () -> String = { UUID().uuidString.replacingOccurrences(of: "-", with: "") },
        now: @escaping @Sendable () -> Date = { Date() }
    ) {
        self.consumer = consumer
        self.nonce = nonce
        self.now = now
    }

    /// A signed request. Parameters of a GET or DELETE go in `url`'s query;
    /// `form` is the body of a POST. Both are covered by the signature.
    public func request(
        method: String,
        url: URL,
        form: [(String, String)] = [],
        token: OAuthToken?,
        oauthExtras: [(String, String)] = []
    ) -> URLRequest {
        var oauth: [(String, String)] = [
            ("oauth_consumer_key", consumer.key),
            ("oauth_nonce", nonce()),
            ("oauth_signature_method", "HMAC-SHA1"),
            ("oauth_timestamp", String(Int(now().timeIntervalSince1970))),
            ("oauth_version", "1.0"),
        ]
        if let token {
            oauth.append(("oauth_token", token.token))
        }
        oauth += oauthExtras

        let base = Self.signatureBaseString(method: method, url: url, parameters: oauth + form)
        let signature = Self.signature(baseString: base, consumerSecret: consumer.secret, tokenSecret: token?.secret)
        oauth.append(("oauth_signature", signature))

        var request = URLRequest(url: url)
        request.httpMethod = method
        request.setValue(Self.authorizationHeader(oauth), forHTTPHeaderField: "Authorization")
        if !form.isEmpty {
            request.httpBody = Data(PercentEncoding.encodeForm(form).utf8)
            request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        }
        return request
    }

    /// RFC 5849 section 3.4.1. `parameters` are the oauth_* parameters and the
    /// form body; the query is read from `url`.
    public static func signatureBaseString(method: String, url: URL, parameters: [(String, String)]) -> String {
        let components = URLComponents(url: url, resolvingAgainstBaseURL: false)
        let query = components?.percentEncodedQuery.map(PercentEncoding.decodeForm) ?? []
        let normalized = (query + parameters)
            .map { (PercentEncoding.encode($0.0), PercentEncoding.encode($0.1)) }
            .sorted { $0.0 == $1.0 ? $0.1 < $1.1 : $0.0 < $1.0 }
            .map { "\($0.0)=\($0.1)" }
            .joined(separator: "&")
        return [method.uppercased(), baseURI(components), normalized]
            .map(PercentEncoding.encode)
            .joined(separator: "&")
    }

    public static func signature(baseString: String, consumerSecret: String, tokenSecret: String?) -> String {
        let key = PercentEncoding.encode(consumerSecret) + "&" + PercentEncoding.encode(tokenSecret ?? "")
        let digest = HMACSHA1.authenticate(Array(baseString.utf8), key: Array(key.utf8))
        return Data(digest).base64EncodedString()
    }

    private static func baseURI(_ components: URLComponents?) -> String {
        let scheme = components?.scheme?.lowercased() ?? ""
        let host = components?.host?.lowercased() ?? ""
        var uri = "\(scheme)://\(host)"
        if let port = components?.port, !(scheme == "http" && port == 80), !(scheme == "https" && port == 443) {
            uri += ":\(port)"
        }
        let path = components?.percentEncodedPath ?? ""
        return uri + (path.isEmpty ? "/" : path)
    }

    private static func authorizationHeader(_ parameters: [(String, String)]) -> String {
        let fields = parameters.map { "\(PercentEncoding.encode($0.0))=\"\(PercentEncoding.encode($0.1))\"" }
        return "OAuth realm=\"\", " + fields.joined(separator: ", ")
    }
}
