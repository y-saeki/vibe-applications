import MailCrewCore
import XCTest

@testable import MailCrewMac

/// Which screen shows, and what the inbox does with what the Rust side
/// answers. Google, the Keychain and the browser are out of reach
/// (DEVELOPMENT.md): the Rust side here is `FakeCore`.
@MainActor
final class AppModelTests: XCTestCase {
    private var core: FakeCore!
    private var opened: [URL] = []

    override func setUp() {
        core = FakeCore()
        opened = []
    }

    private func model() -> AppModel {
        AppModel(core: core, openURL: { [unowned self] in opened.append($0) })
    }

    func testStartsAtTheSignInWhenNobodyIsSignedIn() async {
        let model = model()
        await model.start()
        XCTAssertEqual(model.screen, .signIn)
        XCTAssertNil(model.signInError)
        XCTAssertTrue(core.inboxCalls.isEmpty)
    }

    func testStartsAtTheSignInWithWhyWhenTheAccountsCannotBeRead() async {
        core.accountsError = .Keychain(message: "キーチェーンを読めませんでした")
        let model = model()
        await model.start()
        XCTAssertEqual(model.screen, .signIn)
        XCTAssertEqual(model.signInError, "キーチェーンを読めませんでした")
    }

    func testStartsAtTheInboxOfEveryAccount() async {
        core.accounts = ["a@example.com"]
        core.inboxAnswers = [.success(InboxPage(cards: [card("t1"), card("t2")], failures: []))]
        let model = model()
        await model.start()
        XCTAssertEqual(model.screen, .inbox)
        XCTAssertEqual(model.cards.map(\.id), ["t1", "t2"])
        XCTAssertEqual(core.inboxCalls.count, 1)
        XCTAssertEqual(core.inboxCalls[0].accounts, [])
        XCTAssertEqual(core.inboxCalls[0].mailbox, .inbox)
        XCTAssertEqual(core.inboxCalls[0].query, "")
    }

    func testAnAccountThatDidNotAnswerIsANotice() async {
        core.accounts = ["a@example.com", "b@example.com"]
        core.inboxAnswers = [.success(InboxPage(
            cards: [card("t1")],
            failures: [AccountFailure(account: "b@example.com", kind: .network, message: "つながりません")]
        ))]
        let model = model()
        await model.start()
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
        XCTAssertEqual(model.notice, "b@example.com: つながりません")
        XCTAssertTrue(model.showsAccounts)
    }

    func testAnAccountGoogleNoLongerAcceptsLeavesTheStrip() async {
        core.accounts = ["a@example.com", "b@example.com"]
        let model = model()
        await model.start()
        // The Rust side removed b@ before it answered.
        core.accounts = ["a@example.com"]
        core.inboxAnswers = [.success(InboxPage(
            cards: [card("t1"), card("t2", account: "b@example.com")],
            failures: [AccountFailure(account: "b@example.com", kind: .signedOut, message: "ログインし直してください")]
        ))]
        await model.load()
        XCTAssertEqual(model.screen, .inbox)
        XCTAssertEqual(model.cards.map(\.key), ["a@example.com\nt1"])
        XCTAssertEqual(model.notice, "b@example.com: ログインし直してください")
        XCTAssertFalse(model.showsAccounts)
    }

    func testTheLastAccountSignedOutGoesBackToTheSignIn() async {
        core.accounts = ["a@example.com"]
        let model = model()
        await model.start()
        core.accounts = []
        core.inboxAnswers = [.failure(.SignedOut(message: "ログインし直してください"))]
        await model.load()
        XCTAssertEqual(model.screen, .signIn)
        XCTAssertEqual(model.signInError, "ログインし直してください")
        XCTAssertTrue(model.cards.isEmpty)
    }

    func testAFailedLoadKeepsTheCardsAndSaysWhy() async {
        core.accounts = ["a@example.com"]
        core.inboxAnswers = [.success(InboxPage(cards: [card("t1")], failures: []))]
        let model = model()
        await model.start()
        core.inboxAnswers = [.failure(.Network(message: "つながりません"))]
        await model.load()
        XCTAssertEqual(model.screen, .inbox)
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
        XCTAssertEqual(model.notice, "つながりません")
    }

    func testASignInThatCannotStartSaysWhy() async {
        core.startSignInError = .Internal(message: "クライアント ID とクライアント シークレットを入力してください")
        let model = model()
        await model.start()
        await model.signIn(clientId: "", clientSecret: "")
        XCTAssertEqual(model.screen, .signIn)
        XCTAssertFalse(model.signingIn)
        XCTAssertEqual(model.signInError, "クライアント ID とクライアント シークレットを入力してください")
        XCTAssertTrue(opened.isEmpty)
    }

    func testACancelledSignInSaysNothing() async {
        core.startSignInError = .Cancelled(message: "中止しました")
        let model = model()
        await model.start()
        await model.signIn(clientId: "id", clientSecret: "secret")
        XCTAssertEqual(model.screen, .signIn)
        XCTAssertNil(model.signInError)
    }
}
