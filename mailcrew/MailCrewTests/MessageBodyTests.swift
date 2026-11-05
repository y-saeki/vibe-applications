import MailCrewCore
import XCTest

@testable import MailCrewMac

/// When a card's body is fetched, and what the model decides about its images
/// and links. How WebKit then shows the body is in `MessageFrameTests` and the
/// manual pass (DEVELOPMENT.md).
@MainActor
final class MessageBodyTests: XCTestCase {
    private var core: FakeCore!
    private var opened: [URL] = []

    override func setUp() {
        core = FakeCore()
        core.accounts = ["a@example.com"]
        opened = []
    }

    private func model(cards: [Card]) async -> AppModel {
        core.inboxAnswers = [.success(InboxPage(cards: cards, failures: []))]
        let model = AppModel(core: core, openURL: { [unowned self] in opened.append($0) })
        await model.start()
        return model
    }

    func testNoBodyIsFetchedUntilACardComesIntoView() async {
        let model = await model(cards: [card("t1")])
        XCTAssertTrue(core.messageCalls.isEmpty)
        XCTAssertNil(model.bodies[model.cards[0].bodyKey])
    }

    func testACardInViewFetchesItselfThenItsNeighboursOutwards() async {
        let cards = (1...7).map { card("t\($0)") }
        let model = await model(cards: cards)
        await model.fetchBodies(near: cards[3])
        XCTAssertEqual(core.messageCalls.map(\.messageId), ["m-t4", "m-t5", "m-t3", "m-t6", "m-t2"])
        XCTAssertTrue(core.messageCalls.allSatisfy { $0.account == "a@example.com" })
    }

    func testTheBodyIsAskedOfTheCardsAccount() async {
        core.accounts = ["a@example.com", "b@example.com"]
        let theirs = card("t1", account: "b@example.com")
        core.messages = ["m-t1": .success(html())]
        let model = await model(cards: [card("t1"), theirs])
        await model.fetchBody(theirs)
        XCTAssertEqual(core.messageCalls.map(\.account), ["b@example.com"])
        XCTAssertNotNil(model.bodies[theirs.bodyKey])
        XCTAssertNil(model.bodies[model.cards[0].bodyKey])
    }

    func testABodyIsFetchedOnce() async {
        core.messages = ["m-t1": .success(html())]
        let model = await model(cards: [card("t1")])
        await model.fetchBody(model.cards[0])
        await model.fetchBody(model.cards[0])
        XCTAssertEqual(core.messageCalls.count, 1)
    }

    func testAFailedBodyKeepsTheSnippetAndIsTriedAgain() async {
        let model = await model(cards: [card("t1")])
        await model.fetchBody(model.cards[0])
        XCTAssertNil(model.bodies[model.cards[0].bodyKey])
        XCTAssertEqual(model.notice, "not scripted")
        core.messages = ["m-t1": .success(html())]
        await model.fetchBody(model.cards[0])
        XCTAssertEqual(core.messageCalls.count, 2)
        XCTAssertNotNil(model.bodies[model.cards[0].bodyKey])
    }

    func testAReloadKeepsTheBodiesOfMessagesStillOnTheStrip() async {
        core.messages = ["m-t1": .success(html()), "m-t2": .success(html())]
        let model = await model(cards: [card("t1"), card("t2")])
        await model.fetchBody(model.cards[0])
        await model.fetchBody(model.cards[1])
        model.showRemoteImages(model.cards[1])
        // t1 is gone, and a reply came into t2.
        core.inboxAnswers = [.success(InboxPage(cards: [card("t2", messageId: "m-t2-reply")], failures: []))]
        await model.load()
        XCTAssertTrue(model.bodies.isEmpty)
        XCTAssertTrue(model.remoteImagesShown.isEmpty)
    }

    func testImagesAreHeldBackUntilShown() async {
        core.messages = ["m-t1": .success(html(remoteImages: true)), "m-t2": .success(html(remoteImages: true))]
        let model = await model(cards: [card("t1"), card("t2")])
        await model.fetchBody(model.cards[0])
        await model.fetchBody(model.cards[1])
        XCTAssertTrue(model.asksForRemoteImages(model.cards[0]))
        XCTAssertFalse(model.showsRemoteImages(model.cards[0]))
        model.showRemoteImages(model.cards[0])
        XCTAssertTrue(model.showsRemoteImages(model.cards[0]))
        XCTAssertFalse(model.asksForRemoteImages(model.cards[0]))
        // Only the one message.
        XCTAssertTrue(model.asksForRemoteImages(model.cards[1]))
        XCTAssertTrue(core.savedImageSenders.isEmpty)
    }

    func testAMessageWithoutImagesFromTheNetworkDoesNotAsk() async {
        core.messages = ["m-t1": .success(html()), "m-t2": .success(content(.text(text: "本文")))]
        let model = await model(cards: [card("t1"), card("t2")])
        await model.fetchBody(model.cards[0])
        await model.fetchBody(model.cards[1])
        XCTAssertFalse(model.asksForRemoteImages(model.cards[0]))
        XCTAssertFalse(model.asksForRemoteImages(model.cards[1]))
    }

    func testASenderShownAlwaysHasImagesFromTheStart() async {
        core.imageSenders = ["news@example.com"]
        core.messages = ["m-t1": .success(html(remoteImages: true))]
        let model = await model(cards: [card("t1", from: "News@Example.com")])
        await model.fetchBody(model.cards[0])
        XCTAssertTrue(model.showsRemoteImages(model.cards[0]))
        XCTAssertFalse(model.asksForRemoteImages(model.cards[0]))
    }

    func testShowingASenderAlwaysIsKeptPerAddressAndShowsEveryCardFromThem() async {
        core.imageSenders = ["old@example.com"]
        core.messages = [
            "m-t1": .success(html(remoteImages: true)),
            "m-t2": .success(html(remoteImages: true)),
            "m-t3": .success(html(remoteImages: true)),
        ]
        let model = await model(cards: [
            card("t1", from: "news@example.com"),
            card("t2", from: "news@example.com"),
            card("t3", from: "other@example.com"),
        ])
        for card in model.cards { await model.fetchBody(card) }
        await model.alwaysShowImages(from: model.cards[0])
        XCTAssertEqual(core.savedImageSenders, [["old@example.com", "news@example.com"]])
        XCTAssertEqual(model.imageSenders, ["old@example.com", "news@example.com"])
        XCTAssertTrue(model.showsRemoteImages(model.cards[1]))
        // Same domain, another address: still asks.
        XCTAssertTrue(model.asksForRemoteImages(model.cards[2]))
    }

    func testImagesAskWhenTheSendersCannotBeRead() async {
        core.imageSendersError = .Internal(message: "読めませんでした")
        core.messages = ["m-t1": .success(html(remoteImages: true))]
        let model = await model(cards: [card("t1")])
        XCTAssertEqual(model.notice, "読めませんでした")
        XCTAssertEqual(model.screen, .inbox)
        await model.fetchBody(model.cards[0])
        XCTAssertTrue(model.asksForRemoteImages(model.cards[0]))
    }

    func testOnlyWebAndMailLinksAreOpened() async {
        let model = await model(cards: [])
        for link in ["https://example.com/a", "http://example.com", "mailto:a@example.com",
                     "javascript:alert(1)", "file:///etc/passwd", "data:text/html,x", "smb://host/share"] {
            model.openLink(URL(string: link)!)
        }
        XCTAssertEqual(opened.map(\.absoluteString), ["https://example.com/a", "http://example.com", "mailto:a@example.com"])
    }
}
