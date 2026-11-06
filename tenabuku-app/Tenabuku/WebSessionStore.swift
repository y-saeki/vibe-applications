import Foundation
import HatenaShared
import HatenaUnofficial
import WebKit

/// The login to Hatena's website, as opposed to the OAuth token in
/// `AccountStore`. Some lists, such as My hot entries and the hidden users,
/// are only served to the website and need its login cookies, and so does
/// adding a star.
///
/// The cookies live in WebKit's default store, the one every web view in the
/// app shares, so a login made on Hatena's OAuth approval page or in the
/// article browser counts too.
@MainActor
@Observable
final class WebSessionStore {
    /// Whether the last thing the app learned says the website login works:
    /// set when the login sheet finishes or a request succeeds, cleared when
    /// Hatena asks for a login or the user signs out. Remembered across
    /// launches so that Settings can say so without asking Hatena.
    private(set) var isSignedIn: Bool {
        didSet { UserDefaults.standard.set(isSignedIn, forKey: Self.signedInKey) }
    }

    /// Where the login sheet starts. Hatena sends the browser on to `location`
    /// once the user has signed in.
    static let signInURL = URL(string: "https://www.hatena.ne.jp/login?location=https%3A%2F%2Fb.hatena.ne.jp%2F")!
    /// Reaching this host in the login sheet means the login is done.
    static let landingHost = "b.hatena.ne.jp"

    private static let signedInKey = "webSessionSignedIn"
    @ObservationIgnored private let cookieStore = WKWebsiteDataStore.default().httpCookieStore
    /// The hidden users last read from Hatena, and when.
    @ObservationIgnored private var ignoredUsersCache: (users: Set<String>, readAt: Date)?
    /// How long the hidden users are reused before Hatena is asked again.
    /// Someone hidden on the website shows up within this, or at once when
    /// the comment list is pulled to refresh.
    private static let ignoredUsersLifetime: TimeInterval = 10 * 60

    init() {
        isSignedIn = UserDefaults.standard.bool(forKey: Self.signedInKey)
    }

    func myHotEntries() async throws -> [MyHotEntry] {
        let cookies = await cookieStore.allCookies()
        do {
            let entries = try await MyHotEntryAPI(http: AppServices.http, cookies: cookies).entries()
            isSignedIn = true
            return entries
        } catch HatenaError.unauthorized {
            isSignedIn = false
            throw HatenaError.unauthorized
        }
    }

    /// The users hidden on Hatena's website, whose comments are left out of
    /// the comment lists. Empty without the website login. A failure to read
    /// them does not stop the comments: the last list read is used, or none.
    func ignoredUsers(refresh: Bool = false) async -> Set<String> {
        if !refresh, let cache = ignoredUsersCache, Date().timeIntervalSince(cache.readAt) < Self.ignoredUsersLifetime {
            return cache.users
        }
        let cookies = await cookieStore.allCookies()
        do {
            let users = try await IgnoredUsersAPI(http: AppServices.http, cookies: cookies).users()
            isSignedIn = true
            ignoredUsersCache = (users, Date())
            return users
        } catch HatenaError.unauthorized {
            isSignedIn = false
            ignoredUsersCache = ([], Date())
            return []
        } catch {
            FailureLog.record(error)
            return ignoredUsersCache?.users ?? []
        }
    }

    /// Adds a star to a comment as the signed-in user. Throws
    /// `HatenaError.unauthorized` without the website login.
    func addStar(to uri: URL) async throws {
        let cookies = await cookieStore.allCookies()
        do {
            try await HatenaStarAPI(http: AppServices.http, cookies: cookies).addStar(to: uri)
            isSignedIn = true
        } catch HatenaError.unauthorized {
            isSignedIn = false
            throw HatenaError.unauthorized
        }
    }

    func didSignIn() {
        isSignedIn = true
        ignoredUsersCache = nil
    }

    /// Forgets the website login by deleting Hatena's cookies. The OAuth token
    /// is untouched.
    func signOut() async {
        for cookie in await cookieStore.allCookies() where Self.isHatena(cookie) {
            await cookieStore.deleteCookie(cookie)
        }
        isSignedIn = false
        ignoredUsersCache = nil
    }

    private static func isHatena(_ cookie: HTTPCookie) -> Bool {
        let domain = cookie.domain.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        return domain == "hatena.ne.jp" || domain.hasSuffix(".hatena.ne.jp")
    }
}
