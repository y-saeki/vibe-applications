import Foundation

/// Every failure the clients report.
///
/// `parseFailed` is the one that means Hatena changed something the app relies
/// on. The app shows it prominently and logs it, so that a breakage is noticed
/// the first time it happens rather than looking like an empty list.
public enum HatenaError: Error, Equatable, Sendable {
    /// The request did not complete (offline, timeout, TLS failure, ...).
    case transport(String)
    /// The server answered 401: the OAuth token is missing, revoked or lacks a scope.
    case unauthorized
    /// Any other status outside 2xx.
    case httpStatus(Int)
    /// The response arrived but did not have the expected shape.
    /// `source` names the endpoint, e.g. `feed.hotentry`.
    case parseFailed(source: String, detail: String)

    public var isBreakage: Bool {
        if case .parseFailed = self { return true }
        return false
    }
}
