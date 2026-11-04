import Foundation
import HatenaShared
import HatenaUnofficial
import WebKit

/// The login to Hatena's website, as opposed to the OAuth token in
/// `AccountStore`. Some lists, such as My hot entries, are only served to the
/// website and need its login cookies.
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

    func didSignIn() {
        isSignedIn = true
    }

    /// Forgets the website login by deleting Hatena's cookies. The OAuth token
    /// is untouched.
    func signOut() async {
        for cookie in await cookieStore.allCookies() where Self.isHatena(cookie) {
            await cookieStore.deleteCookie(cookie)
        }
        isSignedIn = false
    }

    private static func isHatena(_ cookie: HTTPCookie) -> Bool {
        let domain = cookie.domain.lowercased().trimmingCharacters(in: CharacterSet(charactersIn: "."))
        return domain == "hatena.ne.jp" || domain.hasSuffix(".hatena.ne.jp")
    }
}
