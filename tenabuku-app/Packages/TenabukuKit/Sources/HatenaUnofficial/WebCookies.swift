import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// The website's login cookies, for the endpoints that only answer a
/// signed-in browser.
enum WebCookies {
    /// The `Cookie` header a browser would send to `url`, or nil when none of
    /// `cookies` applies.
    static func header(for url: URL, from cookies: [HTTPCookie], now: Date = Date()) -> String? {
        guard let host = url.host?.lowercased() else { return nil }
        let path = url.path.isEmpty ? "/" : url.path
        let applicable = cookies.filter { cookie in
            let domain = cookie.domain.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
            guard host == domain || host.hasSuffix("." + domain) else { return false }
            guard path.hasPrefix(cookie.path) else { return false }
            if let expires = cookie.expiresDate, expires <= now { return false }
            return true
        }
        guard !applicable.isEmpty else { return nil }
        return applicable.map { "\($0.name)=\($0.value)" }.joined(separator: "; ")
    }

    /// A request carrying only `header`. The session's own cookie handling is
    /// off: letting it add what it holds could send another account's.
    static func request(_ url: URL, header: String) -> URLRequest {
        var request = URLRequest(url: url)
        request.setValue(header, forHTTPHeaderField: "Cookie")
        request.httpShouldHandleCookies = false
        return request
    }
}
