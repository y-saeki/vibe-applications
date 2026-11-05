import XCTest

@testable import MailCrewMac

/// The rules a message's HTML is shown under (`MessageFrame`). What WebKit
/// does with them is checked by hand (DEVELOPMENT.md).
final class MessageFrameTests: XCTestCase {
    private let openable: (String) -> Bool = { AppModel.isOpenableLink($0) }

    func testTheDocumentRefusesEveryLoadButInlineStylesAndDataImages() {
        let document = MessageFrame.document(html: "<p>本文</p>", allowRemoteImages: false)
        XCTAssertTrue(document.contains(
            "content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:\""
        ), document)
        XCTAssertTrue(document.contains("<meta name=\"referrer\" content=\"no-referrer\">"))
        XCTAssertTrue(document.contains("<body><p>本文</p><mailcrew-end"))
    }

    func testAllowedImagesComeFromTheWebOnly() {
        XCTAssertEqual(
            MessageFrame.policy(allowRemoteImages: true),
            "default-src 'none'; style-src 'unsafe-inline'; img-src data: https: http:"
        )
    }

    func testTheRulesBlockEverythingButTheDocumentAndData() throws {
        let rules = try decode(MessageFrame.ruleList(allowRemoteImages: false))
        XCTAssertEqual(rules.count, 3)
        XCTAssertEqual(rules[0]["action"]?["type"] as? String, "block")
        XCTAssertEqual(rules[0]["trigger"]?["url-filter"] as? String, ".*")
        XCTAssertEqual(rules.dropFirst().map { $0["action"]?["type"] as? String }, ["ignore-previous-rules", "ignore-previous-rules"])
        XCTAssertEqual(rules.dropFirst().map { $0["trigger"]?["url-filter"] as? String }, ["^about:", "^data:"])
    }

    func testAllowedRulesLetImagesThroughAndNothingElse() throws {
        let rules = try decode(MessageFrame.ruleList(allowRemoteImages: true))
        XCTAssertEqual(rules.count, 4)
        XCTAssertEqual(rules[3]["action"]?["type"] as? String, "ignore-previous-rules")
        XCTAssertEqual(rules[3]["trigger"]?["url-filter"] as? String, "^https?://")
        XCTAssertEqual(rules[3]["trigger"]?["resource-type"] as? [String], ["image"])
    }

    func testOnlyTheAppsOwnDocumentLoads() {
        XCTAssertEqual(decide("about:blank", linkActivated: false), .allow)
        XCTAssertEqual(decide("about:blank", linkActivated: true), .refuse)
        XCTAssertEqual(decide("about:blank#top", linkActivated: true), .refuse)
        // A navigation the user did not click, such as a file dropped on the view.
        XCTAssertEqual(decide("file:///Users/me/a.html", linkActivated: false), .refuse)
        XCTAssertEqual(decide("https://example.com", linkActivated: false), .refuse)
        XCTAssertEqual(MessageFrame.decide(url: nil, linkActivated: true, isOpenable: openable), .refuse)
    }

    func testClickedWebAndMailLinksOpenOutside() {
        for link in ["https://example.com/a?b=1", "http://example.com", "mailto:a@example.com"] {
            XCTAssertEqual(decide(link, linkActivated: true), .openOutside(URL(string: link)!), link)
        }
        for link in ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,x", "smb://host/share"] {
            XCTAssertEqual(decide(link, linkActivated: true), .refuse, link)
        }
    }

    func testTextLinksTheWebAddressesInIt() {
        let text = MessageText.attributed(
            "詳しくは https://example.com/a. か（https://example.com/b）へ。file:///etc/passwd",
            isOpenable: openable
        )
        let links = text.runs.compactMap { run in run.link.map { ($0.absoluteString, String(text[run.range].characters)) } }
        XCTAssertEqual(links.map(\.0), ["https://example.com/a", "https://example.com/b"])
        XCTAssertEqual(links.map(\.1), ["https://example.com/a", "https://example.com/b"])
        XCTAssertEqual(String(text.characters), "詳しくは https://example.com/a. か（https://example.com/b）へ。file:///etc/passwd")
    }

    private func decide(_ url: String, linkActivated: Bool) -> MessageFrame.Navigation {
        MessageFrame.decide(url: URL(string: url), linkActivated: linkActivated, isOpenable: openable)
    }

    private func decode(_ json: String) throws -> [[String: [String: Any]]] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: Data(json.utf8)) as? [[String: [String: Any]]])
    }
}
