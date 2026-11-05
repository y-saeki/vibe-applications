import AppKit
import Foundation
import MailCrewCore
import Observation

/// What the window shows and everything it asks of the Rust side. The views
/// read it and call its methods; nothing else talks to `MailCrew`.
@MainActor
@Observable
final class AppModel {
    enum Screen: Equatable {
        /// Asking who is signed in, before anything is shown.
        case starting
        case signIn
        case inbox
    }

    private(set) var screen: Screen = .starting
    /// The signed-in addresses, in the order they were added.
    private(set) var accounts: [String] = []

    /// Set while a sign-in waits for the browser.
    private(set) var signingIn = false
    /// Why the last sign-in failed, or why the app came back to the sign-in.
    private(set) var signInError: String?

    /// The threads on the strip, newest first.
    private(set) var cards: [Card] = []
    private(set) var loading = false
    /// A message for the user that does not stop anything, such as an
    /// account that did not answer. Cleared by the user.
    var notice: String?

    @ObservationIgnored private let core: MailCrewProtocol
    @ObservationIgnored private let openURL: (URL) -> Void
    /// Set when a load is asked for while one is running, so the newer one wins.
    @ObservationIgnored private var loadAgain = false

    init(core: MailCrewProtocol, openURL: @escaping (URL) -> Void = { NSWorkspace.shared.open($0) }) {
        self.core = core
        self.openURL = openURL
    }

    /// The model over the real Rust side.
    static func live() -> AppModel {
        AppModel(core: MailCrew(configDir: configDirectory().path))
    }

    /// Where the views, the senders whose images are shown and the
    /// preferences are kept: the Tauri app's folder, so they carry over.
    static func configDirectory() -> URL {
        FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("com.ysaeki.mailcrew", isDirectory: true)
    }

    /// A card says which account it is in only when there is more than one.
    var showsAccounts: Bool { accounts.count > 1 }

    /// Decides the first screen: the inbox when someone is signed in.
    func start() async {
        do {
            accounts = try await core.accountEmails()
        } catch {
            showSignIn(message: Self.message(of: error))
            return
        }
        if accounts.isEmpty {
            showSignIn(message: nil)
        } else {
            screen = .inbox
            await load()
        }
    }

    // MARK: Sign-in

    /// Opens Google's sign-in in the browser and waits for it to come back.
    /// Empty fields use the client of the first account, when there is one.
    func signIn(clientId: String, clientSecret: String) async {
        signInError = nil
        signingIn = true
        defer { signingIn = false }
        do {
            let pending = try await core.startSignIn(clientId: clientId, clientSecret: clientSecret)
            guard let url = URL(string: pending.url()) else {
                signInError = "ログインのページを開けませんでした"
                await core.cancelSignIn()
                return
            }
            openURL(url)
            _ = try await pending.finish()
            accounts = try await core.accountEmails()
            screen = .inbox
            await load()
        } catch MailCrewError.Cancelled {
            // What the user asked for; nothing to report.
        } catch {
            signInError = Self.message(of: error)
        }
    }

    /// Gives up the sign-in that is waiting for the browser.
    func cancelSignIn() {
        Task { await core.cancelSignIn() }
    }

    private func showSignIn(message: String?) {
        cards = []
        signInError = message
        screen = .signIn
    }

    // MARK: Inbox

    /// Fetches the newest threads of every account.
    func load() async {
        if loading {
            loadAgain = true
            return
        }
        loading = true
        defer { loading = false }
        repeat {
            loadAgain = false
            do {
                let page = try await core.listInbox(accounts: [], mailbox: .inbox, query: "")
                if loadAgain { continue }
                cards = page.cards
                await report(page.failures)
            } catch {
                await fail(error)
            }
        } while loadAgain
    }

    /// Says which accounts did not answer; the others' cards are shown all the same.
    private func report(_ failures: [AccountFailure]) async {
        guard !failures.isEmpty else { return }
        let message = failures.map { "\($0.account): \($0.message)" }.joined(separator: "\n")
        // An account Google no longer accepts has been removed on the Rust side.
        if failures.contains(where: { $0.kind == .signedOut }) {
            await signedOut(message: message)
        } else {
            notice = message
        }
    }

    private func fail(_ error: Error) async {
        let message = Self.message(of: error)
        if case MailCrewError.SignedOut = error {
            await signedOut(message: message)
        } else {
            notice = message
        }
    }

    /// Google no longer accepts an account. Back to the sign-in when nobody
    /// is left; otherwise the cards of the accounts that went leave the strip.
    private func signedOut(message: String) async {
        do {
            accounts = try await core.accountEmails()
        } catch {
            notice = Self.message(of: error)
            return
        }
        if accounts.isEmpty {
            showSignIn(message: message)
        } else {
            cards.removeAll { !accounts.contains($0.account) }
            notice = message
        }
    }

    /// The text to show for a failure. The Rust side writes it in Japanese.
    static func message(of error: Error) -> String {
        guard let error = error as? MailCrewError else { return error.localizedDescription }
        switch error {
        case .SignedOut(let message), .Cancelled(let message), .Network(let message),
             .Google(let message), .Keychain(let message), .Internal(let message):
            return message
        }
    }
}

// The records the screens show, by the names they have in the Rust side.
// The screens do not import MailCrewCore, whose `View` and `Label` would
// shadow SwiftUI's.
typealias Card = MailCrewCore.Card

extension Card {
    /// Thread ids are only unique within one account.
    var key: String { "\(account)\n\(id)" }
}
