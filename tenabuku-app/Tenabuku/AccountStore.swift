import Foundation
import HatenaOfficial
import HatenaShared
import os
import Security

/// The signed-in Hatena account, shared by every screen that needs one. The
/// access token is kept in the Keychain and read back at launch.
@MainActor
@Observable
final class AccountStore {
    struct Account: Codable, Equatable {
        let token: OAuthToken
        /// The Hatena ID.
        let urlName: String
        let displayName: String
    }

    /// Where Hatena sends the browser once the user approves. Hatena only
    /// accepts http(s) callbacks, so this is a host that can never resolve: the
    /// sign-in web view catches the navigation before it is sent.
    static let callbackURL = URL(string: "https://tenabuku.invalid/oauth-callback")!

    /// A request token waiting for the user's approval.
    struct PendingSignIn: Identifiable {
        let requestToken: OAuthToken
        /// Hatena's approval page.
        let approvalURL: URL

        var id: String { requestToken.token }
    }

    enum SignInOutcome {
        case signedIn
        /// The user turned the request down on Hatena's page.
        case declined
    }

    private(set) var account: Account?
    /// Set when Hatena refused the saved token, so that screens ask the user to
    /// sign in again instead of failing on every request.
    private(set) var needsSignInAgain = false

    @ObservationIgnored private let keychain: KeychainItem
    private static let logger = Logger(subsystem: "io.github.y-saeki.tenabuku", category: "account")

    init(keychain: KeychainItem = KeychainItem(service: "io.github.y-saeki.tenabuku", account: "hatena")) {
        self.keychain = keychain
        do {
            account = try keychain.read().map { try JSONDecoder().decode(Account.self, from: $0) }
        } catch {
            // An unreadable item only costs a sign-in; it must not stop the app.
            Self.logger.error("saved account unreadable: \(error.localizedDescription, privacy: .public)")
        }
    }

    /// False when the build carries no consumer key, so signing in cannot work.
    var canSignIn: Bool {
        AppServices.oauthSigner != nil
    }

    /// The account's bookmarks, or nil when nobody is signed in or the token
    /// has been refused.
    var bookmarkAPI: BookmarkAPI? {
        guard let account, !needsSignInAgain, let signer = AppServices.oauthSigner else { return nil }
        return BookmarkAPI(http: AppServices.http, signer: signer, token: account.token)
    }

    /// The first leg of OAuth: a request token and the page where the user approves it.
    func beginSignIn() async throws -> PendingSignIn? {
        guard let signer = AppServices.oauthSigner else { return nil }
        let flow = OAuthFlow(http: AppServices.http, signer: signer)
        let requestToken = try await flow.requestToken(callback: Self.callbackURL.absoluteString, scopes: OAuthScope.allCases)
        return PendingSignIn(requestToken: requestToken, approvalURL: flow.authorizationURL(for: requestToken))
    }

    static func isCallback(_ url: URL) -> Bool {
        url.host() == callbackURL.host() && url.path() == callbackURL.path()
    }

    /// The last leg: trades the verifier Hatena put on `callback` for an
    /// access token, and keeps it in the Keychain.
    func finishSignIn(_ pending: PendingSignIn, callback: URL) async throws -> SignInOutcome {
        guard let signer = AppServices.oauthSigner else { return .declined }
        let query = URLComponents(url: callback, resolvingAgainstBaseURL: false)?.queryItems ?? []
        // Hatena documents only the redirect that carries a verifier; a
        // redirect without one is read as the user refusing.
        guard let verifier = query.first(where: { $0.name == "oauth_verifier" })?.value else {
            Self.logger.notice("callback without oauth_verifier: \(query.map(\.name), privacy: .public)")
            return .declined
        }
        let flow = OAuthFlow(http: AppServices.http, signer: signer)
        let grant = try await flow.accessToken(for: pending.requestToken, verifier: verifier)
        let account = Account(token: grant.token, urlName: grant.urlName, displayName: grant.displayName)
        try keychain.write(JSONEncoder().encode(account))
        self.account = account
        needsSignInAgain = false
        return .signedIn
    }

    func signOut() {
        do {
            try keychain.delete()
        } catch {
            Self.logger.error("saved account not deleted: \(error.localizedDescription, privacy: .public)")
        }
        account = nil
        needsSignInAgain = false
    }

    /// Records a failure from an API call made with the account. A 401 means
    /// the token no longer works.
    func record(_ error: Error) {
        FailureLog.record(error)
        if error as? HatenaError == .unauthorized {
            needsSignInAgain = true
        }
    }
}

/// One generic password in the Keychain, readable once the device has been
/// unlocked after a restart and never synced to other devices.
struct KeychainItem {
    let service: String
    let account: String

    struct Failure: LocalizedError {
        let status: OSStatus

        var errorDescription: String? {
            SecCopyErrorMessageString(status, nil) as String? ?? "Keychain error \(status)"
        }
    }

    private var query: [String: Any] {
        [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
        ]
    }

    func read() throws -> Data? {
        var query = query
        query[kSecReturnData as String] = true
        query[kSecMatchLimit as String] = kSecMatchLimitOne
        var result: CFTypeRef?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        if status == errSecItemNotFound {
            return nil
        }
        guard status == errSecSuccess else { throw Failure(status: status) }
        return result as? Data
    }

    func write(_ data: Data) throws {
        let status = SecItemUpdate(query as CFDictionary, [kSecValueData as String: data] as CFDictionary)
        if status == errSecItemNotFound {
            var item = query
            item[kSecValueData as String] = data
            item[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            let added = SecItemAdd(item as CFDictionary, nil)
            guard added == errSecSuccess else { throw Failure(status: added) }
        } else if status != errSecSuccess {
            throw Failure(status: status)
        }
    }

    func delete() throws {
        let status = SecItemDelete(query as CFDictionary)
        guard status == errSecSuccess || status == errSecItemNotFound else { throw Failure(status: status) }
    }
}
