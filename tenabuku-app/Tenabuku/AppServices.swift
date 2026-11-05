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
    static let popularComments = PopularCommentsAPI(http: http)
    static let recentComments = RecentCommentsAPI(http: http)
    static let myBookmarks = MyBookmarkTimelineStore(
        url: FileManager.default.urls(for: .cachesDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("MyBookmarks.json")
    )

    /// Signs with the consumer key the build carries in Info.plist, or nil when
    /// Secrets.xcconfig left it empty.
    static let oauthSigner: OAuthSigner? = {
        let info = Bundle.main.infoDictionary ?? [:]
        guard let key = info["HatenaConsumerKey"] as? String, !key.isEmpty,
              let secret = info["HatenaConsumerSecret"] as? String, !secret.isEmpty
        else { return nil }
        return OAuthSigner(consumer: OAuthConsumer(key: key, secret: secret))
    }()

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
