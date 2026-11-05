import MailCrewCore
import XCTest

@testable import MailCrewMac

/// The buttons at the foot of a card — 完了, タグ, あとで読む, スター — and
/// the tag picker's rows. What Gmail then does with the labels is out of
/// reach (DEVELOPMENT.md): these assert what was asked of the Rust side.
@MainActor
final class CardActionsTests: XCTestCase {
    private var core: FakeCore!

    override func setUp() {
        core = FakeCore()
        core.accounts = ["a@example.com", "b@example.com"]
    }

    private func model(cards: [Card], next: [Card] = []) async -> AppModel {
        core.inboxAnswers = [
            .success(InboxPage(cards: cards, failures: [])),
            .success(InboxPage(cards: next, failures: [])),
        ]
        let model = AppModel(core: core)
        await model.start()
        return model
    }

    private func label(_ name: String) -> MailCrewCore.Label {
        MailCrewCore.Label(id: "Label_\(name)", name: name)
    }

    // MARK: 完了 and あとで読む

    func testDoneArchivesEveryMessageOfTheCardInItsAccount() async {
        let theirs = card("t2", account: "b@example.com", messageIds: ["m1", "m2"])
        let model = await model(cards: [card("t1"), theirs])
        await model.archive(theirs)
        XCTAssertEqual(core.actions, [.archive(account: "b@example.com", messageIds: ["m1", "m2"], mailbox: .inbox)])
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
    }

    func testLaterSetsTheCardAside() async {
        let model = await model(cards: [card("t1"), card("t2")])
        XCTAssertTrue(model.offersSetAside)
        await model.setAside(model.cards[0])
        XCTAssertEqual(core.actions, [.setAside(account: "a@example.com", messageIds: ["m-t1"])])
        XCTAssertEqual(model.cards.map(\.id), ["t2"])
    }

    func testACardThatCouldNotBeTakenOffGoesBackWhereItWas() async {
        let model = await model(cards: [card("t1"), card("t2"), card("t3")])
        core.actionError = .Network(message: "つながりません")
        await model.archive(model.cards[1])
        XCTAssertEqual(model.cards.map(\.id), ["t1", "t2", "t3"])
        XCTAssertEqual(model.notice, "つながりません")
    }

    func testTheBodyOfACardTakenOffIsForgotten() async {
        core.messages = ["m-t1": .success(html())]
        let model = await model(cards: [card("t1"), card("t2")])
        let taken = model.cards[0]
        await model.fetchBody(taken)
        await model.archive(taken)
        XCTAssertNil(model.bodies[taken.bodyKey])
    }

    func testTheNextPageIsFetchedOnceTheStripIsWorkedThrough() async {
        let model = await model(cards: [card("t1")], next: [card("t2")])
        await model.archive(model.cards[0])
        XCTAssertEqual(core.inboxCalls.count, 2)
        XCTAssertEqual(model.cards.map(\.id), ["t2"])
    }

    // MARK: スター

    func testStarStarsTheMessageShownAndUnstarsTheThread() async {
        let model = await model(cards: [card("t1", messageId: "m2", messageIds: ["m1", "m2"])])
        await model.toggleStar(model.cards[0])
        XCTAssertTrue(model.cards[0].starred)
        await model.toggleStar(model.cards[0])
        XCTAssertFalse(model.cards[0].starred)
        XCTAssertEqual(core.actions, [
            .setStarred(account: "a@example.com", messageId: "m2", messageIds: ["m1", "m2"], starred: true),
            .setStarred(account: "a@example.com", messageId: "m2", messageIds: ["m1", "m2"], starred: false),
        ])
        XCTAssertTrue(model.starring.isEmpty)
    }

    func testAStarGmailRefusesGoesBack() async {
        let model = await model(cards: [card("t1", starred: true)])
        core.actionError = .Google(message: "Gmail が受け付けませんでした")
        await model.toggleStar(model.cards[0])
        XCTAssertTrue(model.cards[0].starred)
        XCTAssertEqual(model.notice, "Gmail が受け付けませんでした")
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
    }

    // MARK: タグ

    func testTheLabelsOfEveryAccountOnTheStripAreFetched() async {
        core.labels = ["a@example.com": [label("仕事")], "b@example.com": [label("家")]]
        let model = await model(cards: [card("t1"), card("t2", account: "b@example.com")])
        // Fetched alongside the load; asked for again here to wait for them.
        await model.fetchLabels("a@example.com")
        await model.fetchLabels("b@example.com")
        XCTAssertEqual(Set(core.labelCalls), ["a@example.com", "b@example.com"])
        XCTAssertEqual(model.labels(of: model.cards[0]).map(\.name), ["仕事"])
        XCTAssertEqual(model.labels(of: model.cards[1]).map(\.name), ["家"])
    }

    func testAnExistingTagIsAppliedInTheCardsAccount() async {
        let theirs = card("t2", account: "b@example.com")
        let model = await model(cards: [card("t1"), theirs])
        await model.applyTag(.existing(label("家")), to: theirs)
        XCTAssertEqual(core.actions, [
            .applyLabel(account: "b@example.com", messageIds: ["m-t2"], labelId: "Label_家", mailbox: .inbox),
        ])
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
    }

    func testANewTagIsCreatedThenAppliedAndOfferedNextTime() async {
        core.labels = ["a@example.com": [label("仕事"), label("旅行")]]
        let model = await model(cards: [card("t1"), card("t2")])
        await model.fetchLabels("a@example.com")
        await model.applyTag(.new("家計"), to: model.cards[0])
        XCTAssertEqual(core.actions, [
            .createLabel(account: "a@example.com", name: "家計"),
            .applyLabel(account: "a@example.com", messageIds: ["m-t1"], labelId: "Label_家計", mailbox: .inbox),
        ])
        XCTAssertEqual(model.labels(of: model.cards[0]).map(\.name).sorted(), ["仕事", "家計", "旅行"].sorted())
    }

    func testATagThatCouldNotBeCreatedPutsTheCardBack() async {
        let model = await model(cards: [card("t1")])
        core.actionError = .Google(message: "作れませんでした")
        await model.applyTag(.new("家計"), to: model.cards[0])
        XCTAssertEqual(core.actions, [.createLabel(account: "a@example.com", name: "家計")])
        XCTAssertEqual(model.cards.map(\.id), ["t1"])
        XCTAssertEqual(model.notice, "作れませんでした")
    }

    func testTheRowsMatchWhatIsTypedAndOfferToCreateANewName() {
        let labels = [label("Work"), label("仕事"), label("Workshop")]
        XCTAssertEqual(AppModel.tagRows(labels: labels, filter: "").map(\.text), ["Work", "仕事", "Workshop"])
        XCTAssertEqual(AppModel.tagRows(labels: labels, filter: " work ").map(\.text), ["Work", "Workshop"])
        XCTAssertEqual(AppModel.tagRows(labels: labels, filter: "shop").map(\.text), ["Workshop", "「shop」を作成"])
        XCTAssertEqual(AppModel.tagRows(labels: labels, filter: "shop").last?.choice, .new("shop"))
        XCTAssertEqual(AppModel.tagRows(labels: [], filter: "").count, 0)
    }
}
