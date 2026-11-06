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

    /// The pile the cards on the strip came from; every action takes a card out of it.
    private(set) var mailbox: Mailbox = .inbox
    /// Each account's labels, as last fetched. Label ids are only unique
    /// within one account, so the picker shows the card's account's alone.
    private(set) var labels: [String: [TagLabel]] = [:]
    /// The cards, by `Card.key`, whose star is on its way to Gmail.
    private(set) var starring: Set<String> = []

    /// The sender's BIMI checks, by `Card.bodyKey`. A message without an
    /// entry shows Gmail's checks alone: BIMI is off, or the body is not here.
    private(set) var brands: [String: BrandCheck] = [:]
    /// Whether the user lets Mail Crew look up BIMI; on by default, as the
    /// Rust side has it.
    private(set) var looksUpBrands = true

    /// The custom views, in the order the sidebar lists them.
    private(set) var views: [SavedView] = []
    /// The sidebar's tab the strip shows: `inboxView`, `setAsideView` or a view's id.
    private(set) var activeView = AppModel.inboxView
    /// What the cards on the strip were fetched for; it says what an empty strip means.
    private(set) var shownFilter: InboxFilter? = .inbox

    /// Set while まとめて完了 waits for Gmail.
    private(set) var archivingAll = false

    /// Set while the settings' sign-in waits for the browser.
    private(set) var addingAccount = false
    /// Why the settings' last sign-in, or removing an account, failed.
    private(set) var accountError: String?
    /// The account the settings are removing.
    private(set) var removingAccount: String?
    /// Why the views could not be saved.
    private(set) var viewError: String?
    /// Why a sender could not be removed.
    private(set) var imageSenderError: String?
    /// Why the BIMI setting could not be saved.
    private(set) var brandSettingError: String?
    /// Set while the BIMI setting is on its way to the file.
    private(set) var savingBrandSetting = false

    @ObservationIgnored private let core: MailCrewProtocol
    @ObservationIgnored private let openURL: (URL) -> Void
    @ObservationIgnored private let files: Files
    /// The BIMI lookups on their way, by `Card.bodyKey`.
    @ObservationIgnored private var brandLookups: [String: Task<Void, Never>] = [:]
    /// Set when a load is asked for while one is running, so the newer one wins.
    @ObservationIgnored private var loadAgain = false
    /// The bodies on their way, by `Card.bodyKey`.
    @ObservationIgnored private var fetching: Set<String> = []

    /// Where attachments go, and how a file is handed to the OS.
    struct Files {
        /// Where 保存 writes.
        var downloads: URL
        /// Where 開く writes what it hands to the OS, each file in a folder of its own.
        var opened: URL
        /// Opens a file in the app the OS picks for it; false when it could not.
        var open: (URL) -> Bool

        static func live() -> Files {
            let temporary = FileManager.default.temporaryDirectory.path
            return Files(
                downloads: FileManager.default.urls(for: .downloadsDirectory, in: .userDomainMask)[0],
                opened: URL(fileURLWithPath: openedAttachmentsDir(tempDir: temporary), isDirectory: true),
                open: { NSWorkspace.shared.open($0) }
            )
        }
    }

    init(
        core: MailCrewProtocol,
        openURL: @escaping (URL) -> Void = { NSWorkspace.shared.open($0) },
        files: Files = .live()
    ) {
        self.core = core
        self.openURL = openURL
        self.files = files
    }

    /// The model over the real Rust side.
    static func live() -> AppModel {
        let files = Files.live()
        // What was opened last time is not needed any more. A file a viewer
        // still holds open stays, and goes next time.
        try? FileManager.default.removeItem(at: files.opened)
        return AppModel(core: MailCrew(configDir: configDirectory().path), files: files)
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
            await loadSettings()
            await load()
        }
    }

    private func loadSettings() async {
        await loadViews()
        await loadImageSenders()
        await loadPreferences()
    }

    /// Reads the custom views. When they cannot be read, the sidebar has the
    /// inbox and あとで読む alone.
    private func loadViews() async {
        do {
            views = try await core.listViews()
        } catch {
            notice = Self.message(of: error)
        }
        if !Self.tabExists(activeView, views: views) { activeView = Self.inboxView }
    }

    /// Reads whether BIMI is looked up. When it cannot be read, the default
    /// holds; the Rust side reads the file again before every lookup.
    private func loadPreferences() async {
        if let preferences = try? await core.preferences() {
            looksUpBrands = preferences.brandLogos
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
            await loadSettings()
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
        activeView = Self.inboxView
        bodies = [:]
        remoteImagesShown = []
        brands = [:]
        labels = [:]
        signInError = message
        screen = .signIn
    }

    // MARK: Inbox

    /// Fetches the newest threads of what the active tab shows.
    func load() async {
        if loading {
            loadAgain = true
            return
        }
        loading = true
        defer { loading = false }
        repeat {
            loadAgain = false
            let filter = Self.filter(for: activeView, views: views, accounts: accounts)
            do {
                // A view whose accounts have all been removed shows nothing.
                let page = if let filter {
                    try await core.listInbox(accounts: filter.accounts, mailbox: filter.mailbox, query: filter.query)
                } else {
                    InboxPage(cards: [], failures: [])
                }
                if loadAgain { continue }
                cards = page.cards
                shownFilter = filter
                mailbox = filter?.mailbox ?? .inbox
                forgetBodiesOffTheStrip()
                refreshLabels()
                await report(page.failures)
            } catch {
                await fail(error)
            }
        } while loadAgain
    }

    // MARK: Views

    /// The tab for the inbox of every account.
    nonisolated static let inboxView = ""
    /// The tab for what あとで読む set aside. No saved view can have this id.
    nonisolated static let setAsideView = "@set-aside"

    /// The sidebar's tabs: the inbox, あとで読む, then each custom view.
    var tabs: [ViewTab] {
        [
            ViewTab(id: Self.inboxView, name: "受信トレイ", systemImage: "tray"),
            ViewTab(id: Self.setAsideView, name: "あとで読む", systemImage: "clock"),
        ] + views.map { ViewTab(id: $0.id, name: $0.name, systemImage: "line.3.horizontal.decrease.circle") }
    }

    /// The window's title: the name of the tab the strip shows.
    var title: String {
        tabs.first { $0.id == activeView }?.name ?? "受信トレイ"
    }

    /// A tab was picked in the sidebar: the strip shows what it holds.
    func select(view id: String) async {
        guard id != activeView, Self.tabExists(id, views: views) else { return }
        activeView = id
        await load()
    }

    /// What the strip fetches for the tab `id`, given the views and who is
    /// signed in now; nil when every account the view was made for has gone.
    nonisolated static func filter(for id: String, views: [SavedView], accounts: [String]) -> InboxFilter? {
        if id == setAsideView { return InboxFilter(mailbox: .setAside, accounts: [], query: "", custom: false) }
        guard let view = views.first(where: { $0.id == id }) else { return .inbox }
        let chosen = view.accounts.filter { accounts.contains($0) }
        if !view.accounts.isEmpty && chosen.isEmpty { return nil }
        return InboxFilter(
            mailbox: .inbox, accounts: chosen,
            query: view.query.trimmingCharacters(in: .whitespacesAndNewlines), custom: true
        )
    }

    nonisolated static func tabExists(_ id: String, views: [SavedView]) -> Bool {
        id == inboxView || id == setAsideView || views.contains { $0.id == id }
    }

    /// One line on what a view shows, for the settings.
    nonisolated static func describe(_ view: SavedView) -> String {
        let accounts = view.accounts.isEmpty ? "すべてのアカウント" : view.accounts.joined(separator: "、")
        let query = view.query.trimmingCharacters(in: .whitespacesAndNewlines)
        return query.isEmpty ? accounts : "\(accounts) ・ \(query)"
    }

    /// What an empty strip says, by what it was fetched for.
    var emptyState: (title: String, lead: String) {
        if shownFilter?.custom ?? true {
            return ("このビューに当てはまるメールはありません", "条件に合うメールが届いたら、再読み込みで表示されます。")
        }
        if shownFilter?.mailbox == .setAside {
            return ("あとで読むメールはありません", "受信トレイで「あとで読む」を押したメールが、ここに並びます。")
        }
        return ("受信トレイは空です", "新しいメールが届いたら、再読み込みで表示されます。")
    }

    /// What the end of the strip says まとめて完了 does.
    var endSummary: String {
        mailbox == .setAside
            ? "ここまでの \(cards.count) 件を既読にして「あとで読む」から外し、続きを読み込みます。"
            : "ここまでの \(cards.count) 件を既読にしてアーカイブし、次の受信トレイを読み込みます。"
    }

    /// まとめて完了: archives every card on the strip, one request per
    /// account, then fetches the next page. When some accounts' requests
    /// fail, the next page shows which threads are still there.
    func archiveAll() async {
        guard !archivingAll, !cards.isEmpty else { return }
        var byAccount: [(account: String, messageIds: [String])] = []
        for card in cards {
            if let index = byAccount.firstIndex(where: { $0.account == card.account }) {
                byAccount[index].messageIds += card.messageIds
            } else {
                byAccount.append((card.account, card.messageIds))
            }
        }
        archivingAll = true
        defer { archivingAll = false }
        let core = core
        let mailbox = mailbox
        let failures = await withTaskGroup(of: Error?.self) { group in
            for (account, messageIds) in byAccount {
                group.addTask {
                    do {
                        try await core.archive(account: account, messageIds: messageIds, mailbox: mailbox)
                        return nil
                    } catch {
                        return error
                    }
                }
            }
            var failures: [Error] = []
            for await failure in group {
                if let failure { failures.append(failure) }
            }
            return failures
        }
        if failures.count == byAccount.count, let failure = failures.first {
            await fail(failure)
            return
        }
        await load()
        if let failure = failures.first { await fail(failure) }
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
            labels = labels.filter { accounts.contains($0.key) }
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
            checkBrand(card, authentication: content.authentication)
        } catch {
            await fail(error)
        }
    }

    /// Keeps the bodies, and the images let in, of the messages still on the strip.
    private func forgetBodiesOffTheStrip() {
        let keys = Set(cards.map(\.bodyKey))
        bodies = bodies.filter { keys.contains($0.key) }
        brands = brands.filter { keys.contains($0.key) }
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

    // MARK: Sender checks

    /// Starts the BIMI checks of a message whose body just came in. They are
    /// looked up only when Gmail's DMARC result is `pass`; otherwise they
    /// say so without asking anything. While the user has BIMI off, Gmail's
    /// checks show alone.
    private func checkBrand(_ card: Card, authentication: Authentication) {
        guard looksUpBrands else { return }
        let key = card.bodyKey
        guard authentication.dmarc?.result == "pass" else {
            brands[key] = .checked(Brand(bimi: .skipped, domain: "", mark: .none, logo: nil))
            return
        }
        brands[key] = .pending
        brandLookups[key] = Task { await lookUpBrand(card) }
    }

    /// The BIMI lookup of a card's message, while it is on its way.
    func brandLookup(for card: Card) -> Task<Void, Never>? {
        brandLookups[card.bodyKey]
    }

    private func lookUpBrand(_ card: Card) async {
        let key = card.bodyKey
        defer { brandLookups[key] = nil }
        let result: BrandCheck?
        do {
            // Nothing, when the user turned BIMI off since the settings were read.
            result = try await core.getBrand(account: card.account, messageId: card.messageId).map { .checked($0) }
        } catch {
            // Nothing to tell beyond what the checks show.
            result = .failed
        }
        // Turning BIMI off cancels the lookups on their way.
        guard !Task.isCancelled, bodies[key] != nil else { return }
        brands[key] = result
        if result == nil { looksUpBrands = false }
    }

    /// The checks under a card's subject; none until its body is here.
    func trustItems(of card: Card) -> [TrustItem] {
        guard let content = bodies[card.bodyKey] else { return [] }
        return Self.trustItems(authentication: content.authentication, brand: brands[card.bodyKey])
    }

    /// What the row under a card's subject shows: Gmail's SPF, DKIM and
    /// DMARC, then the BIMI and mark certificate checks when there are any.
    /// The words and the tooltips are the Tauri app's (`src/trust.ts`).
    nonisolated static func trustItems(authentication: Authentication, brand: BrandCheck?) -> [TrustItem] {
        [
            gmailItem("spf", "SPF", authentication.spf),
            gmailItem("dkim", "DKIM", authentication.dkim),
            gmailItem("dmarc", "DMARC", authentication.dmarc),
        ] + (brand.map(brandItems) ?? [])
    }

    private nonisolated static let resultText = [
        "pass": "合格",
        "fail": "不合格",
        "softfail": "不合格（ソフトフェイル）",
        "neutral": "判定なし",
        "none": "設定なし",
        "temperror": "一時的なエラー",
        "permerror": "設定の誤り",
        "policy": "ポリシーにより不合格",
    ]

    private nonisolated static func gmailItem(_ key: String, _ label: String, _ check: Check?) -> TrustItem {
        guard let check else {
            return TrustItem(key: key, label: label, tone: .neutral, detail: "\(label): Gmail の検証結果がありません")
        }
        let tone: TrustItem.Tone
        switch check.result {
        case "pass": tone = .pass
        case "fail", "softfail", "permerror": tone = .fail
        default: tone = .neutral
        }
        let text = resultText[check.result] ?? check.result
        let domain = check.domain.isEmpty ? "" : "（\(check.domain)）"
        return TrustItem(key: key, label: label, tone: tone, detail: "\(label): \(text)\(domain)")
    }

    private nonisolated static func brandItems(_ brand: BrandCheck) -> [TrustItem] {
        switch brand {
        case .pending:
            return [
                TrustItem(key: "bimi", label: "BIMI", tone: .pending, detail: "BIMI: 確認しています"),
                TrustItem(key: "mark", label: "VMC", tone: .pending, detail: "VMC: 確認しています"),
            ]
        case .failed:
            return [
                TrustItem(key: "bimi", label: "BIMI", tone: .neutral, detail: "BIMI: 確認できませんでした"),
                TrustItem(key: "mark", label: "VMC", tone: .neutral, detail: "VMC: 確認できませんでした"),
            ]
        case .checked(let brand):
            let (bimiTone, bimiText): (TrustItem.Tone, String) = switch brand.bimi {
            case .pass: (.pass, "登録あり")
            case .none: (.neutral, "登録なし")
            case .declined: (.neutral, "送信元が使わないと宣言しています")
            case .fail: (.fail, "レコードに誤りがあります")
            case .skipped: (.neutral, "DMARC の条件を満たさないため確認していません")
            case .temperror: (.neutral, "DNS に問い合わせられませんでした")
            }
            let (markTone, markText): (TrustItem.Tone, String) = switch brand.mark {
            case .vmc: (.pass, "認証局が登録商標を確認したロゴです")
            case .cmc: (.pass, "認証局が使用実績を確認したロゴです")
            case .none: (.neutral, "証明書はありません")
            case .fail: (.fail, "証明書を確認できませんでした")
            case .temperror: (.neutral, "証明書を取得できませんでした")
            }
            let domain = brand.domain.isEmpty ? "" : "（\(brand.domain)）"
            let markLabel = markLabel(brand.mark)
            return [
                TrustItem(key: "bimi", label: "BIMI", tone: bimiTone, detail: "BIMI: \(bimiText)\(domain)"),
                TrustItem(key: "mark", label: markLabel, tone: markTone, detail: "\(markLabel): \(markText)"),
            ]
        }
    }

    private nonisolated static func markLabel(_ mark: MarkResult) -> String {
        mark == .cmc ? "CMC" : "VMC"
    }

    /// The sender's logo for a card, once its certificate held up: a
    /// `data:image/svg+xml` URL, and nothing else, so that showing it never
    /// reaches the network.
    func logo(of card: Card) -> BrandLogo? {
        guard case .checked(let brand) = brands[card.bodyKey] else { return nil }
        return Self.logo(of: brand)
    }

    nonisolated static func logo(of brand: Brand) -> BrandLogo? {
        guard let url = brand.logo, url.hasPrefix("data:image/svg+xml") else { return nil }
        return BrandLogo(
            url: url,
            title: "\(brand.domain) のロゴ（\(markLabel(brand.mark)) で確認済み）"
        )
    }

    // MARK: Attachments

    /// The files attached to the message a card shows, once its body is here.
    func attachments(of card: Card) -> [Attachment] {
        bodies[card.bodyKey]?.attachments ?? []
    }

    /// 開く: writes the file where opened attachments go and hands it to the
    /// OS. Only the kinds the Rust side calls openable get here, and the Rust
    /// side checks again. Says whether it was opened.
    func openAttachment(_ attachment: Attachment, of card: Card) async -> Bool {
        guard attachment.openable else { return false }
        do {
            let path = try await core.attachmentToOpen(
                account: card.account, messageId: card.messageId,
                attachmentId: attachment.id, openedDir: files.opened.path
            )
            guard files.open(URL(fileURLWithPath: path)) else {
                notice = "添付ファイルを開けませんでした"
                return false
            }
            return true
        } catch {
            await fail(error)
            return false
        }
    }

    /// 保存: writes the file to the Downloads folder, never over another one,
    /// and says under which name. Says whether it was saved.
    func saveAttachment(_ attachment: Attachment, of card: Card) async -> Bool {
        do {
            let path = try await core.saveAttachment(
                account: card.account, messageId: card.messageId,
                attachmentId: attachment.id, dir: files.downloads.path
            )
            let name = URL(fileURLWithPath: path).lastPathComponent
            notice = "ダウンロード フォルダに「\(name)」を保存しました"
            return true
        } catch {
            await fail(error)
            return false
        }
    }

    /// A size in bytes as people read it, as the Tauri app's `formatSize` writes it.
    nonisolated static func formatSize(_ bytes: UInt64) -> String {
        if bytes < 1024 { return "\(bytes) B" }
        let units = ["KB", "MB", "GB"]
        var value = Double(bytes) / 1024
        var unit = 0
        while value >= 1024 && unit < units.count - 1 {
            value /= 1024
            unit += 1
        }
        let number = value < 10 ? String(format: "%.1f", value) : String(Int(value.rounded()))
        return "\(number) \(units[unit])"
    }

    // MARK: Actions

    /// Whether a card offers あとで読む: not on the pile it would go to.
    var offersSetAside: Bool { mailbox == .inbox }

    /// 完了: read, and out of the pile the strip shows.
    func archive(_ card: Card) async {
        let mailbox = mailbox
        await takeOff(card) {
            try await core.archive(account: card.account, messageIds: card.messageIds, mailbox: mailbox)
        }
    }

    /// あとで読む: tagged SetAside, and out of the inbox.
    func setAside(_ card: Card) async {
        await takeOff(card) {
            try await core.setAside(account: card.account, messageIds: card.messageIds)
        }
    }

    /// タグ: tags the card's messages, creating the tag first when it is new,
    /// and takes the card out of the pile the strip shows.
    func applyTag(_ choice: TagChoice, to card: Card) async {
        let account = card.account
        let mailbox = mailbox
        await takeOff(card) {
            let label: TagLabel
            switch choice {
            case .existing(let existing):
                label = existing
            case .new(let name):
                label = try await core.createLabel(account: account, name: name)
                remember(label, in: account)
            }
            try await core.applyLabel(
                account: account, messageIds: card.messageIds, labelId: label.id, mailbox: mailbox
            )
        }
    }

    /// スター: stars the message the card shows, or unstars the whole thread.
    /// The star changes at once, and goes back if Gmail refuses.
    func toggleStar(_ card: Card) async {
        let key = card.key
        guard let index = cards.firstIndex(where: { $0.key == key }), !starring.contains(key) else { return }
        let starred = !cards[index].starred
        cards[index].starred = starred
        starring.insert(key)
        defer { starring.remove(key) }
        do {
            try await core.setStarred(
                account: card.account, messageId: card.messageId, messageIds: card.messageIds, starred: starred
            )
        } catch {
            if let index = cards.firstIndex(where: { $0.key == key }), cards[index].starred == starred {
                cards[index].starred = !starred
            }
            await fail(error)
        }
    }

    /// Takes a card off the strip at once, and puts it back where it was if
    /// `action` fails. The strip shows a page of the pile at a time; once it
    /// has been worked through, the next page is fetched.
    private func takeOff(_ card: Card, _ action: () async throws -> Void) async {
        let key = card.key
        guard let index = cards.firstIndex(where: { $0.key == key }) else { return }
        let taken = cards.remove(at: index)
        let next = cards.indices.contains(index) ? cards[index].key : nil
        do {
            try await action()
        } catch {
            // The strip may have changed meanwhile, a load among other things.
            if !cards.contains(where: { $0.key == key }) {
                let at = next.flatMap { next in cards.firstIndex(where: { $0.key == next }) } ?? cards.endIndex
                cards.insert(taken, at: at)
            }
            await fail(error)
            return
        }
        forgetBodiesOffTheStrip()
        if cards.isEmpty { await load() }
    }

    // MARK: Tags

    /// The labels of the card's account the picker offers, as last fetched.
    func labels(of card: Card) -> [TagLabel] {
        labels[card.account] ?? []
    }

    /// The picker opened on a card: fetches its account's labels when none are here yet.
    func tagPickerOpened(for card: Card) {
        guard labels(of: card).isEmpty else { return }
        Task { await fetchLabels(card.account) }
    }

    /// Fetches the labels of every account on the strip, for the picker.
    private func refreshLabels() {
        for account in Set(cards.map(\.account)) {
            Task { await fetchLabels(account) }
        }
    }

    /// When they cannot be fetched the picker shows what it has, and asks
    /// again when it next opens.
    func fetchLabels(_ account: String) async {
        guard let fetched = try? await core.listLabels(account: account) else { return }
        guard accounts.contains(account) else { return }
        labels[account] = fetched
    }

    /// Adds a label just made to its account's, in the order the picker shows.
    private func remember(_ label: TagLabel, in account: String) {
        let known = (labels[account] ?? []).filter { $0.id != label.id }
        labels[account] = (known + [label]).sorted { $0.name.localizedCompare($1.name) == .orderedAscending }
    }

    /// The picker's rows for what is typed: the labels whose names hold it,
    /// then one that creates it, when no label is called that already.
    nonisolated static func tagRows(labels: [TagLabel], filter: String) -> [TagRow] {
        let query = filter.trimmingCharacters(in: .whitespacesAndNewlines)
        let lower = query.lowercased()
        var rows = labels
            .filter { lower.isEmpty || $0.name.lowercased().contains(lower) }
            .map { TagRow(text: $0.name, choice: .existing($0)) }
        if !query.isEmpty, !labels.contains(where: { $0.name.lowercased() == lower }) {
            rows.append(TagRow(text: "「\(query)」を作成", choice: .new(query)))
        }
        return rows
    }

    // MARK: Settings

    /// Google でログインして追加: signs another account in. Empty fields use
    /// the client of the first account.
    func addAccount(clientId: String, clientSecret: String) async {
        accountError = nil
        addingAccount = true
        defer { addingAccount = false }
        do {
            let pending = try await core.startSignIn(
                clientId: Self.blankToNil(clientId), clientSecret: Self.blankToNil(clientSecret)
            )
            guard let url = URL(string: pending.url()) else {
                accountError = "ログインのページを開けませんでした"
                await core.cancelSignIn()
                return
            }
            openURL(url)
            _ = try await pending.finish()
        } catch MailCrewError.Cancelled {
            return
        } catch {
            accountError = Self.message(of: error)
            return
        }
        await accountsChanged()
    }

    /// 削除 beside an account: Mail Crew stops reading it; its mail on Gmail
    /// stays as it is. Back to the sign-in once nobody is left.
    func removeAccount(_ account: String) async {
        accountError = nil
        removingAccount = account
        defer { removingAccount = nil }
        do {
            try await core.signOut(account: account)
        } catch {
            accountError = Self.message(of: error)
            return
        }
        await accountsChanged()
    }

    /// Asks who is signed in now, and fetches the strip again for them.
    private func accountsChanged() async {
        do {
            accounts = try await core.accountEmails()
        } catch {
            accountError = Self.message(of: error)
            return
        }
        if accounts.isEmpty {
            showSignIn(message: nil)
            return
        }
        cards.removeAll { !accounts.contains($0.account) }
        labels = labels.filter { accounts.contains($0.key) }
        forgetBodiesOffTheStrip()
        await load()
    }

    /// The view form's 保存 and a view's 削除: replaces the saved views. The
    /// strip follows when the tab it shows changed or went.
    @discardableResult
    func saveViews(_ next: [SavedView]) async -> Bool {
        do {
            try await core.saveViews(views: next)
        } catch {
            viewError = Self.message(of: error)
            return false
        }
        viewError = nil
        let before = Self.filter(for: activeView, views: views, accounts: accounts)
        views = next
        if !Self.tabExists(activeView, views: views) { activeView = Self.inboxView }
        if Self.filter(for: activeView, views: views, accounts: accounts) != before { await load() }
        return true
    }

    /// A view the form makes or edits: the name and the search trimmed, and
    /// only the accounts still signed in. Nil, with why, when it has no name.
    func view(id: String?, name: String, accounts chosen: Set<String>, query: String) -> SavedView? {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty else {
            viewError = "ビューの名前を入力してください"
            return nil
        }
        return SavedView(
            id: id ?? Self.newViewId(), name: name,
            accounts: accounts.filter { chosen.contains($0) },
            query: query.trimmingCharacters(in: .whitespacesAndNewlines)
        )
    }

    /// Adds a view the form made, or puts an edited one in its place.
    func saveView(_ view: SavedView) async -> Bool {
        let next = views.contains { $0.id == view.id }
            ? views.map { $0.id == view.id ? view : $0 }
            : views + [view]
        return await saveViews(next)
    }

    func deleteView(_ id: String) async {
        await saveViews(views.filter { $0.id != id })
    }

    func clearViewError() { viewError = nil }

    nonisolated static func newViewId() -> String {
        let time = String(Int(Date().timeIntervalSince1970 * 1000), radix: 36)
        let random = String(UInt32.random(in: 0...UInt32.max), radix: 36)
        return "v\(time)\(random)"
    }

    /// 削除 beside a sender: their messages ask again before showing images.
    func removeImageSender(_ email: String) async {
        imageSenderError = nil
        do {
            imageSenders = try await core.saveImageSenders(senders: imageSenders.filter { $0 != email })
        } catch {
            imageSenderError = Self.message(of: error)
        }
    }

    /// BIMI のロゴを表示する. Turned off, the checks already made go; turned
    /// on, the messages already here are looked up.
    func setLooksUpBrands(_ on: Bool) async {
        guard on != looksUpBrands, !savingBrandSetting else { return }
        brandSettingError = nil
        savingBrandSetting = true
        defer { savingBrandSetting = false }
        do {
            try await core.savePreferences(preferences: Preferences(brandLogos: on))
        } catch {
            brandSettingError = Self.message(of: error)
            return
        }
        looksUpBrands = on
        if on {
            for card in cards {
                if let content = bodies[card.bodyKey], brands[card.bodyKey] == nil {
                    checkBrand(card, authentication: content.authentication)
                }
            }
        } else {
            brandLookups.values.forEach { $0.cancel() }
            brandLookups = [:]
            brands = [:]
        }
    }

    private nonisolated static func blankToNil(_ text: String) -> String? {
        let text = text.trimmingCharacters(in: .whitespacesAndNewlines)
        return text.isEmpty ? nil : text
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
typealias Mailbox = MailCrewCore.Mailbox
typealias Attachment = MailCrewCore.Attachment
typealias Authentication = MailCrewCore.Authentication
typealias Brand = MailCrewCore.Brand
typealias Check = MailCrewCore.Check
typealias MarkResult = MailCrewCore.MarkResult
/// A custom view. SwiftUI has a `View` of its own.
typealias SavedView = MailCrewCore.View
typealias Preferences = MailCrewCore.Preferences

/// What the strip fetches: a pile, across some accounts (every one when
/// empty), narrowed by a Gmail search.
struct InboxFilter: Equatable {
    let mailbox: Mailbox
    let accounts: [String]
    let query: String
    /// A custom view rather than a whole pile, which changes what "empty" says.
    let custom: Bool

    static let inbox = InboxFilter(mailbox: .inbox, accounts: [], query: "", custom: false)
}

/// A tab in the sidebar.
struct ViewTab: Equatable, Identifiable {
    let id: String
    let name: String
    let systemImage: String
}

/// Where a message's BIMI checks stand.
enum BrandCheck: Equatable {
    case pending
    /// The lookup failed; nothing to tell beyond that.
    case failed
    case checked(Brand)
}

/// One check in the row under a card's subject: a word with a dot whose
/// colour says how it went, and a tooltip that says the rest.
struct TrustItem: Equatable {
    enum Tone: Equatable {
        case pass, fail, neutral, pending
    }

    let key: String
    let label: String
    let tone: Tone
    let detail: String
}

/// A sender's logo, as a `data:` URL, with what its tooltip says.
struct BrandLogo: Equatable {
    let url: String
    let title: String
}
/// A Gmail label, shown as a タグ. SwiftUI has a `Label` of its own.
typealias TagLabel = MailCrewCore.Label

/// What the tag picker resolves with: a label the account has, or a name to create.
enum TagChoice: Equatable {
    case existing(TagLabel)
    case new(String)
}

struct TagRow: Equatable {
    let text: String
    let choice: TagChoice
}

extension Card {
    /// Thread ids are only unique within one account.
    var key: String { "\(account)\n\(id)" }
    /// The message the card shows. A reply that arrives changes it, and with
    /// it the body to fetch.
    var bodyKey: String { "\(account)\n\(messageId)" }
}
