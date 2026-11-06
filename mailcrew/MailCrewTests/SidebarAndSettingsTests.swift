import MailCrewCore
import XCTest

@testable import MailCrewMac

/// The sidebar's tabs and what each fetches, まとめて完了 and the end of the
/// strip, and what the settings ask of the Rust side. The browser a sign-in
/// opens and Google are out of reach (DEVELOPMENT.md).
@MainActor
final class SidebarAndSettingsTests: XCTestCase {
    private var core: FakeCore!
    private var opened: [URL] = []

    override func setUp() {
        core = FakeCore()
        opened = []
    }

    private func model() -> AppModel {
        AppModel(core: core, openURL: { [unowned self] in opened.append($0) })
    }

    private func view(
        _ id: String, name: String = "ビュー", accounts: [String] = [], query: String = ""
    ) -> MailCrewCore.View {
        MailCrewCore.View(id: id, name: name, accounts: accounts, query: query)
    }

    private func page(_ cards: [Card]) -> Result<InboxPage, MailCrewError> {
        .success(InboxPage(cards: cards, failures: []))
    }

    private func started(_ accounts: [String] = ["a@example.com"], cards: [Card] = []) async -> AppModel {
        core.accounts = accounts
        core.inboxAnswers = [page(cards)]
        let model = model()
        await model.start()
        return model
    }

    // MARK: Tabs

    func testTheSidebarListsTheInboxSetAsideThenTheViews() async {
        core.views = [view("v1", name: "仕事"), view("v2", name: "お知らせ")]
        let model = await started()
        XCTAssertEqual(model.tabs.map(\.id), [AppModel.inboxView, AppModel.setAsideView, "v1", "v2"])
        XCTAssertEqual(model.tabs.map(\.name), ["受信トレイ", "あとで読む", "仕事", "お知らせ"])
        XCTAssertEqual(model.activeView, AppModel.inboxView)
        XCTAssertEqual(model.title, "受信トレイ")
    }

    func testViewsThatCannotBeReadAreANotice() async {
        core.viewsError = .Internal(message: "ビューを読めませんでした")
        let model = await started()
        XCTAssertEqual(model.tabs.count, 2)
        XCTAssertEqual(model.notice, "ビューを読めませんでした")
        XCTAssertEqual(model.screen, .inbox)
    }

    func testSetAsideFetchesThatPileAndItsActionsTakeFromIt() async {
        let model = await started()
        core.inboxAnswers = [page([card("t1")])]
        await model.select(view: AppModel.setAsideView)
        XCTAssertEqual(model.title, "あとで読む")
        XCTAssertEqual(core.inboxCalls.last?.mailbox, .setAside)
        XCTAssertFalse(model.offersSetAside)
        await model.archive(model.cards[0])
        XCTAssertEqual(core.actions.first, .archive(account: "a@example.com", messageIds: ["m-t1"], mailbox: .setAside))
    }

    func testACustomViewFetchesItsAccountsStillSignedInAndItsSearch() async {
        core.views = [view("v1", name: "仕事", accounts: ["b@example.com", "gone@example.com"], query: "  is:unread ")]
        let model = await started(["a@example.com", "b@example.com"])
        await model.select(view: "v1")
        XCTAssertEqual(model.title, "仕事")
        XCTAssertEqual(core.inboxCalls.last?.accounts, ["b@example.com"])
        XCTAssertEqual(core.inboxCalls.last?.mailbox, .inbox)
        XCTAssertEqual(core.inboxCalls.last?.query, "is:unread")
        XCTAssertEqual(model.emptyState.title, "このビューに当てはまるメールはありません")
    }

    func testAViewWhoseAccountsHaveAllGoneFetchesNothing() async {
        core.views = [view("v1", accounts: ["gone@example.com"])]
        let model = await started(cards: [card("t1")])
        await model.select(view: "v1")
        XCTAssertEqual(core.inboxCalls.count, 1)
        XCTAssertTrue(model.cards.isEmpty)
        XCTAssertEqual(model.emptyState.title, "このビューに当てはまるメールはありません")
    }

    func testPickingTheTabShownFetchesNothing() async {
        let model = await started()
        await model.select(view: AppModel.inboxView)
        await model.select(view: "unknown")
        XCTAssertEqual(core.inboxCalls.count, 1)
    }

    func testTheEmptyStripSaysWhatItWasFetchedFor() async {
        let model = await started()
        XCTAssertEqual(model.emptyState.title, "受信トレイは空です")
        await model.select(view: AppModel.setAsideView)
        XCTAssertEqual(model.emptyState.title, "あとで読むメールはありません")
    }

    func testDescribeSaysTheAccountsAndTheSearch() {
        XCTAssertEqual(AppModel.describe(view("v")), "すべてのアカウント")
        XCTAssertEqual(
            AppModel.describe(view("v", accounts: ["a@example.com", "b@example.com"], query: " is:unread ")),
            "a@example.com、b@example.com ・ is:unread"
        )
    }

    // MARK: まとめて完了

    func testTheEndSaysHowManyAndWhere() async {
        let model = await started(cards: [card("t1"), card("t2")])
        XCTAssertEqual(model.endSummary, "ここまでの 2 件を既読にしてアーカイブし、次の受信トレイを読み込みます。")
        core.inboxAnswers = [page([card("t3")])]
        await model.select(view: AppModel.setAsideView)
        XCTAssertEqual(model.endSummary, "ここまでの 1 件を既読にして「あとで読む」から外し、続きを読み込みます。")
    }

    func testArchiveAllSendsOneRequestPerAccountThenFetchesTheNextPage() async {
        let model = await started(["a@example.com", "b@example.com"], cards: [
            card("t1", messageIds: ["m1", "m2"]),
            card("t2", account: "b@example.com"),
            card("t3"),
        ])
        core.inboxAnswers = [page([card("t4")])]
        await model.archiveAll()
        XCTAssertEqual(Set(core.actions.map { "\($0)" }), Set([
            FakeCore.Action.archive(account: "a@example.com", messageIds: ["m1", "m2", "m-t3"], mailbox: .inbox),
            FakeCore.Action.archive(account: "b@example.com", messageIds: ["m-t2"], mailbox: .inbox),
        ].map { "\($0)" }))
        XCTAssertEqual(core.inboxCalls.count, 2)
        XCTAssertEqual(model.cards.map(\.id), ["t4"])
        XCTAssertNil(model.notice)
    }

    func testArchiveAllThatFailsKeepsTheStrip() async {
        let model = await started(cards: [card("t1"), card("t2")])
        core.actionError = .Network(message: "つながりません")
        await model.archiveAll()
        XCTAssertEqual(model.cards.map(\.id), ["t1", "t2"])
        XCTAssertEqual(core.inboxCalls.count, 1)
        XCTAssertEqual(model.notice, "つながりません")
    }

    func testArchiveAllThatFailsForOneAccountFetchesTheNextPageAndSaysSo() async {
        let model = await started(["a@example.com", "b@example.com"], cards: [
            card("t1"), card("t2", account: "b@example.com"),
        ])
        core.actionError = .Network(message: "つながりません")
        core.inboxAnswers = [page([card("t1")])]
        await model.archiveAll()
        XCTAssertEqual(core.actions.count, 2)
        XCTAssertEqual(core.inboxCalls.count, 2)
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
        XCTAssertEqual(model.notice, "つながりません")
    }

    // MARK: Accounts

    func testRemovingAnAccountSignsItOutAndFetchesTheOthers() async {
        let model = await started(["a@example.com", "b@example.com"], cards: [
            card("t1"), card("t2", account: "b@example.com"),
        ])
        core.inboxAnswers = [page([card("t1")])]
        await model.removeAccount("b@example.com")
        XCTAssertEqual(core.signOutCalls, ["b@example.com"])
        XCTAssertEqual(model.accounts, ["a@example.com"])
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
        XCTAssertEqual(core.inboxCalls.count, 2)
        XCTAssertFalse(model.showsAccounts)
    }

    func testRemovingTheLastAccountGoesBackToTheSignIn() async {
        let model = await started(cards: [card("t1")])
        await model.removeAccount("a@example.com")
        XCTAssertEqual(model.screen, .signIn)
        XCTAssertNil(model.signInError)
        XCTAssertTrue(model.cards.isEmpty)
    }

    func testAnAccountThatCannotBeRemovedSaysWhy() async {
        let model = await started(cards: [card("t1")])
        core.signOutError = .Keychain(message: "キーチェーンに書けませんでした")
        await model.removeAccount("a@example.com")
        XCTAssertEqual(model.accountError, "キーチェーンに書けませんでした")
        XCTAssertEqual(model.accounts, ["a@example.com"])
        XCTAssertEqual(model.screen, .inbox)
    }

    func testAddingAnAccountThatCannotStartSaysWhyAndStaysInTheInbox() async {
        let model = await started()
        core.startSignInError = .Network(message: "つながりません")
        await model.addAccount(clientId: " ", clientSecret: "")
        XCTAssertEqual(model.accountError, "つながりません")
        XCTAssertFalse(model.addingAccount)
        XCTAssertEqual(model.screen, .inbox)
        XCTAssertTrue(opened.isEmpty)
    }

    func testACancelledAddIsNotAnError() async {
        let model = await started()
        core.startSignInError = .Cancelled(message: "中止しました")
        await model.addAccount(clientId: "", clientSecret: "")
        XCTAssertNil(model.accountError)
    }

    // MARK: Views

    func testSavingANewViewAddsItToTheSidebar() async {
        let model = await started(["a@example.com", "b@example.com"])
        let made = model.view(id: nil, name: " 仕事 ", accounts: ["b@example.com", "gone@example.com"], query: " is:unread ")
        let saved = await model.saveView(made!)
        XCTAssertTrue(saved)
        XCTAssertEqual(core.savedViews.last?.count, 1)
        let view = core.savedViews.last![0]
        XCTAssertEqual(view.name, "仕事")
        XCTAssertEqual(view.accounts, ["b@example.com"])
        XCTAssertEqual(view.query, "is:unread")
        XCTAssertTrue(view.id.hasPrefix("v"))
        XCTAssertEqual(model.tabs.last?.name, "仕事")
        // A view nobody is looking at changes nothing on the strip.
        XCTAssertEqual(core.inboxCalls.count, 1)
    }

    func testAViewNeedsAName() async {
        let model = await started()
        XCTAssertNil(model.view(id: nil, name: "  ", accounts: [], query: ""))
        XCTAssertEqual(model.viewError, "ビューの名前を入力してください")
        XCTAssertTrue(core.savedViews.isEmpty)
    }

    func testEditingTheViewShownFetchesItAgain() async {
        core.views = [view("v1", name: "仕事", query: "is:unread")]
        let model = await started()
        await model.select(view: "v1")
        let edited = model.view(id: "v1", name: "仕事", accounts: [], query: "is:starred")!
        _ = await model.saveView(edited)
        XCTAssertEqual(core.savedViews.last, [view("v1", name: "仕事", query: "is:starred")])
        XCTAssertEqual(core.inboxCalls.last?.query, "is:starred")
        XCTAssertEqual(core.inboxCalls.count, 3)
    }

    func testDeletingTheViewShownGoesBackToTheInbox() async {
        core.views = [view("v1"), view("v2")]
        let model = await started()
        await model.select(view: "v1")
        await model.deleteView("v1")
        XCTAssertEqual(core.savedViews.last?.map(\.id), ["v2"])
        XCTAssertEqual(model.activeView, AppModel.inboxView)
        XCTAssertEqual(core.inboxCalls.last?.mailbox, .inbox)
        XCTAssertEqual(core.inboxCalls.last?.query, "")
    }

    func testViewsThatCannotBeSavedStayAsTheyWere() async {
        core.views = [view("v1")]
        let model = await started()
        core.saveViewsError = .Internal(message: "保存できませんでした")
        await model.deleteView("v1")
        XCTAssertEqual(model.viewError, "保存できませんでした")
        XCTAssertEqual(model.views.map(\.id), ["v1"])
    }

    // MARK: Images and BIMI

    func testRemovingASenderStoresTheOthers() async {
        core.imageSenders = ["a@example.com", "b@example.com"]
        let model = await started()
        await model.removeImageSender("a@example.com")
        XCTAssertEqual(core.savedImageSenders.last, ["b@example.com"])
        XCTAssertEqual(model.imageSenders, ["b@example.com"])
    }

    func testTurningBimiOffSavesItAndForgetsTheChecks() async {
        let model = await started(cards: [card("t1")])
        core.messages["m-t1"] = .success(content(
            .html(html: "<p>本文</p>", remoteImages: false),
            authentication: Authentication(spf: nil, dkim: nil, dmarc: Check(result: "fail", domain: "example.com"))
        ))
        await model.fetchBody(model.cards[0])
        XCTAssertEqual(model.trustItems(of: model.cards[0]).count, 5)
        await model.setLooksUpBrands(false)
        XCTAssertEqual(core.savedPreferences, [Preferences(brandLogos: false)])
        XCTAssertFalse(model.looksUpBrands)
        XCTAssertEqual(model.trustItems(of: model.cards[0]).map(\.key), ["spf", "dkim", "dmarc"])
    }

    func testTurningBimiOnChecksTheMessagesAlreadyHere() async {
        core.brandLogos = false
        let model = await started(cards: [card("t1")])
        core.messages["m-t1"] = .success(content(
            .html(html: "<p>本文</p>", remoteImages: false),
            authentication: Authentication(spf: nil, dkim: nil, dmarc: Check(result: "fail", domain: "example.com"))
        ))
        await model.fetchBody(model.cards[0])
        XCTAssertEqual(model.trustItems(of: model.cards[0]).count, 3)
        await model.setLooksUpBrands(true)
        XCTAssertTrue(model.looksUpBrands)
        XCTAssertEqual(model.trustItems(of: model.cards[0]).map(\.key), ["spf", "dkim", "dmarc", "bimi", "mark"])
        // DMARC did not pass, so nothing was asked.
        XCTAssertTrue(core.brandCalls.isEmpty)
    }

    func testABimiSettingThatCannotBeSavedStaysAsItWas() async {
        let model = await started()
        core.savePreferencesError = .Internal(message: "保存できませんでした")
        await model.setLooksUpBrands(false)
        XCTAssertTrue(model.looksUpBrands)
        XCTAssertEqual(model.brandSettingError, "保存できませんでした")
    }
}
