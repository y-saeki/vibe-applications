import Foundation
import HatenaFeed
import HatenaOfficial
import HatenaShared
import HatenaUnofficial
import os

/// The clients the screens use, all sharing one HTTP client and User-Agent.
enum AppServices {
    static let http = HatenaHTTPClient(userAgent: UserAgent.tenabuku(version: appVersion))
    static let feed = FeedClient(http: http)
    static let entryInfo = EntryInfoAPI(http: http)
    static let entryComments = EntryCommentsAPI(http: http)

    private static var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
    }
}

/// Where failures are written, so that a breakage can be traced afterwards in
/// Console.app (subsystem io.github.y-saeki.tenabuku).
enum FailureLog {
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "fetch")

    static func record(_ error: Error) {
        switch error as? HatenaError {
        case let .parseFailed(source, detail)?:
            logger.error("breakage in \(source, privacy: .public): \(detail, privacy: .public)")
        case let other?:
            logger.notice("fetch failed: \(String(describing: other), privacy: .public)")
        case nil:
            logger.notice("fetch failed: \(error.localizedDescription, privacy: .public)")
        }
    }
}
