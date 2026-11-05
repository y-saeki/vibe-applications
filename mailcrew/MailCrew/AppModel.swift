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

    /// The bodies fetched so far, by `Card.bodyKey`. A card without one shows
    /// its snippet.
    private(set) var bodies: [String: MessageContent] = [:]
    /// Senders whose images from the network show without asking, in lower case.
    private(set) var imageSenders: [String] = []
    /// The messages, by `Card.bodyKey`, whose images the user let in with 表示する.
    private(set) var remoteImagesShown: Set<String> = []

    @ObservationIgnored private let core: MailCrewProtocol
    @ObservationIgnored private let openURL: (URL) -> Void
    /// Set when a load is asked for while one is running, so the newer one wins.
    @ObservationIgnored private var loadAgain = false
    /// The bodies on their way, by `Card.bodyKey`.
    @ObservationIgnored private var fetching: Set<String> = []

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
            await loadImageSenders()
            await load()
        }
    }

    /// Reads who the user chose to see images from. When it cannot be read,
    /// every message asks.
    private func loadImageSenders() async {
        do {
            imageSenders = try await core.listImageSenders()
        } catch {
            notice = Self.message(of: error)
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
            await loadImageSenders()
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
        bodies = [:]
        remoteImagesShown = []
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
                forgetBodiesOffTheStrip()
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
            forgetBodiesOffTheStrip()
            notice = message
        }
    }

    // MARK: Bodies

    /// How many cards on either side of one that comes into view have their
    /// bodies fetched too: about the Tauri app's 1,200 pixels.
    static let preloadCards = 2

    /// A card came into view: fetch its body and its neighbours'. A body
    /// costs Gmail quota several times what the card's headers do, so the
    /// bodies of cards nobody scrolls to are never fetched.
    func cardAppeared(_ card: Card) {
        Task { await fetchBodies(near: card) }
    }

    /// Fetches the bodies of `card` and its neighbours, the card itself first
    /// and then outwards, one at a time.
    func fetchBodies(near card: Card) async {
        guard let index = cards.firstIndex(where: { $0.key == card.key }) else { return }
        let offsets = [0] + (1...Self.preloadCards).flatMap { [$0, -$0] }
        let near = offsets.compactMap { cards.indices.contains(index + $0) ? cards[index + $0] : nil }
        for neighbour in near {
            await fetchBody(neighbour)
        }
    }

    /// Fetches one card's body, unless it is here or on its way. On a failure
    /// the snippet stays, and the card is tried again when it next comes into view.
    func fetchBody(_ card: Card) async {
        let key = card.bodyKey
        guard bodies[key] == nil, !fetching.contains(key) else { return }
        fetching.insert(key)
        defer { fetching.remove(key) }
        do {
            let content = try await core.getMessage(account: card.account, messageId: card.messageId)
            // The strip may have been loaded again while the body was on its way.
            guard cards.contains(where: { $0.bodyKey == key }) else { return }
            bodies[key] = content
        } catch {
            await fail(error)
        }
    }

    /// Keeps the bodies, and the images let in, of the messages still on the strip.
    private func forgetBodiesOffTheStrip() {
        let keys = Set(cards.map(\.bodyKey))
        bodies = bodies.filter { keys.contains($0.key) }
        remoteImagesShown.formIntersection(keys)
    }

    /// Whether a card's message loads its images from the network.
    func showsRemoteImages(_ card: Card) -> Bool {
        remoteImagesShown.contains(card.bodyKey) || trustsImages(from: card.from.email)
    }

    /// Whether a card has images from the network the user has not let in yet.
    func asksForRemoteImages(_ card: Card) -> Bool {
        guard case .html(_, let remote) = bodies[card.bodyKey]?.body else { return false }
        return remote && !showsRemoteImages(card)
    }

    private func trustsImages(from email: String) -> Bool {
        let email = email.trimmingCharacters(in: .whitespaces).lowercased()
        return !email.isEmpty && imageSenders.contains(email)
    }

    /// 表示する: loads the images in this one message.
    func showRemoteImages(_ card: Card) {
        remoteImagesShown.insert(card.bodyKey)
    }

    /// この差出人は常に表示: remembers the sender, which shows the images in
    /// every card from them. The choice is kept per address, never per domain.
    func alwaysShowImages(from card: Card) async {
        let email = card.from.email
        guard !email.isEmpty, !trustsImages(from: email) else { return }
        do {
            imageSenders = try await core.saveImageSenders(senders: imageSenders + [email])
        } catch {
            notice = Self.message(of: error)
        }
    }

    // MARK: Links

    /// Opens a link from a message outside Mail Crew: http, https and mailto
    /// only, decided by the Rust side.
    func openLink(_ url: URL) {
        guard Self.isOpenableLink(url.absoluteString) else { return }
        openURL(url)
    }

    nonisolated static func isOpenableLink(_ url: String) -> Bool {
        MailCrewCore.isOpenableLink(url: url)
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
typealias MessageContent = MailCrewCore.MessageContent
typealias CardBody = MailCrewCore.CardBody

extension Card {
    /// Thread ids are only unique within one account.
    var key: String { "\(account)\n\(id)" }
    /// The message the card shows. A reply that arrives changes it, and with
    /// it the body to fetch.
    var bodyKey: String { "\(account)\n\(messageId)" }
}
