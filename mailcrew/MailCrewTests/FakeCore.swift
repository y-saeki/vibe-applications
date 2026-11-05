import Foundation
import MailCrewCore

/// The Rust side as the tests want it to answer. Only what `AppModel` calls
/// so far is scripted; anything else fails the test that reaches it.
final class FakeCore: MailCrewProtocol, @unchecked Sendable {
    var accounts: [String] = []
    var accountsError: MailCrewError?
    /// Answers to `listInbox`, one per call, in order.
    var inboxAnswers: [Result<InboxPage, MailCrewError>] = []
    var startSignInError: MailCrewError = .Internal(message: "not scripted")
    private(set) var inboxCalls: [(accounts: [String], mailbox: Mailbox, query: String)] = []
    private(set) var cancelledSignIns = 0
    /// Answers to `getMessage`, by message id. One that is missing fails.
    var messages: [String: Result<MessageContent, MailCrewError>] = [:]
    private(set) var messageCalls: [(account: String, messageId: String)] = []
    var imageSenders: [String] = []
    var imageSendersError: MailCrewError?
    private(set) var savedImageSenders: [[String]] = []
    /// What the card actions were asked to do, in order.
    private(set) var actions: [Action] = []
    /// Thrown by the next card action, then cleared.
    var actionError: MailCrewError?
    /// Each account's labels. One that is missing fails.
    var labels: [String: [MailCrewCore.Label]] = [:]
    private(set) var labelCalls: [String] = []

    enum Action: Equatable {
        case archive(account: String, messageIds: [String], mailbox: Mailbox)
        case setAside(account: String, messageIds: [String])
        case createLabel(account: String, name: String)
        case applyLabel(account: String, messageIds: [String], labelId: String, mailbox: Mailbox)
        case setStarred(account: String, messageId: String, messageIds: [String], starred: Bool)
    }

    func accountEmails() async throws -> [String] {
        if let accountsError { throw accountsError }
        return accounts
    }

    func listInbox(accounts: [String], mailbox: Mailbox, query: String) async throws -> InboxPage {
        inboxCalls.append((accounts, mailbox, query))
        guard !inboxAnswers.isEmpty else { return InboxPage(cards: [], failures: []) }
        return try inboxAnswers.removeFirst().get()
    }

    func startSignIn(clientId: String?, clientSecret: String?) async throws -> SignIn {
        // A real `SignIn` waits on Google, which the tests cannot reach.
        throw startSignInError
    }

    func cancelSignIn() async { cancelledSignIns += 1 }

    func getMessage(account: String, messageId: String) async throws -> MessageContent {
        messageCalls.append((account, messageId))
        guard let answer = messages[messageId] else { throw MailCrewError.Network(message: "not scripted") }
        return try answer.get()
    }

    func listImageSenders() async throws -> [String] {
        if let imageSendersError { throw imageSendersError }
        return imageSenders
    }

    /// Stores them as the Rust side does: trimmed, in lower case, once each.
    func saveImageSenders(senders: [String]) async throws -> [String] {
        savedImageSenders.append(senders)
        var kept: [String] = []
        for sender in senders.map({ $0.trimmingCharacters(in: .whitespaces).lowercased() }) where !kept.contains(sender) {
            kept.append(sender)
        }
        imageSenders = kept
        return kept
    }

    private func act(_ action: Action) throws {
        actions.append(action)
        if let error = actionError {
            actionError = nil
            throw error
        }
    }

    func archive(account: String, messageIds: [String], mailbox: Mailbox) async throws {
        try act(.archive(account: account, messageIds: messageIds, mailbox: mailbox))
    }

    func setAside(account: String, messageIds: [String]) async throws {
        try act(.setAside(account: account, messageIds: messageIds))
    }

    func applyLabel(account: String, messageIds: [String], labelId: String, mailbox: Mailbox) async throws {
        try act(.applyLabel(account: account, messageIds: messageIds, labelId: labelId, mailbox: mailbox))
    }

    /// Makes the label with an id of its name, as Gmail would with one of its own.
    func createLabel(account: String, name: String) async throws -> MailCrewCore.Label {
        try act(.createLabel(account: account, name: name))
        return MailCrewCore.Label(id: "Label_\(name)", name: name)
    }

    func setStarred(account: String, messageId: String, messageIds: [String], starred: Bool) async throws {
        try act(.setStarred(account: account, messageId: messageId, messageIds: messageIds, starred: starred))
    }

    func listLabels(account: String) async throws -> [MailCrewCore.Label] {
        labelCalls.append(account)
        guard let labels = labels[account] else { throw MailCrewError.Network(message: "not scripted") }
        return labels
    }

    // Not called by AppModel yet.
    func attachmentToOpen(account: String, messageId: String, attachmentId: String, openedDir: String) async throws -> String { unscripted() }
    func getBrand(account: String, messageId: String) async throws -> Brand? { unscripted() }
    func listViews() async throws -> [View] { unscripted() }
    func preferences() async throws -> Preferences { unscripted() }
    func saveAttachment(account: String, messageId: String, attachmentId: String, dir: String) async throws -> String { unscripted() }
    func savePreferences(preferences: Preferences) async throws { unscripted() }
    func saveViews(views: [View]) async throws { unscripted() }
    func signOut(account: String) async throws { unscripted() }

    private func unscripted(function: StaticString = #function) -> Never {
        fatalError("FakeCore.\(function) is not scripted")
    }
}

func card(
    _ id: String, account: String = "a@example.com", subject: String = "件名",
    from: String = "from@example.com", messageId: String? = nil, messageIds: [String]? = nil,
    starred: Bool = false
) -> Card {
    let shown = messageId ?? "m-\(id)"
    return Card(
        id: id, account: account, subject: subject,
        from: Person(name: "差出人", email: from),
        date: 0, messageCount: UInt32(messageIds?.count ?? 1), unread: true, starred: starred,
        messageId: shown, messageIds: messageIds ?? [shown], snippet: "本文の抜粋"
    )
}

func content(_ body: CardBody) -> MessageContent {
    MessageContent(body: body, attachments: [], authentication: Authentication(spf: nil, dkim: nil, dmarc: nil))
}

func html(_ markup: String = "<p>本文</p>", remoteImages: Bool = false) -> MessageContent {
    content(.html(html: markup, remoteImages: remoteImages))
}
