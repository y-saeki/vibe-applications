import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

private let page = URL(string: "https://Example.com/a/b?c=1")!
private let icon = Data([0x89, 0x50, 0x4E, 0x47, 0x01])
private let duckIcon = Data([0x00, 0x00, 0x01, 0x00, 0x02])

final class FaviconTests: XCTestCase {
    func testAsksHatenaForTheEntryURLLikeTheWebsite() {
        let page = URL(string: "https://note.com/alice/n/n0123456789ab")!

        XCTAssertEqual(
            Favicon.url(for: page)?.absoluteString,
            "https://cdn-ak2.favicon.st-hatena.com/64?url=https%3A%2F%2Fnote.com%2Falice%2Fn%2Fn0123456789ab"
        )
    }

    func testEncodesTheQueryOfTheEntryURL() {
        let page = URL(string: "http://example.com/a?b=1&c=%E3%81%82#top")!

        XCTAssertEqual(
            Favicon.url(for: page)?.absoluteString,
            "https://cdn-ak2.favicon.st-hatena.com/64?url=http%3A%2F%2Fexample.com%2Fa%3Fb%3D1%26c%3D%25E3%2581%2582%23top"
        )
    }

    func testAsksDuckDuckGoForTheHostOnly() {
        XCTAssertEqual(
            Favicon.fallbackURL(for: page)?.absoluteString,
            "https://icons.duckduckgo.com/ip3/example.com.ico"
        )
    }

    func testHasNoIconForURLsOffTheWeb() {
        for url in ["mailto:alice@example.com", "file:///tmp/a.html"] {
            XCTAssertNil(Favicon.url(for: URL(string: url)!))
            XCTAssertNil(Favicon.fallbackURL(for: URL(string: url)!))
        }
    }

    func testUsesHatenasIconWithoutAskingDuckDuckGo() async {
        let transport = StubTransport { _ in (200, icon) }

        let data = await FaviconClient(http: .stub(transport)).icon(for: page)

        XCTAssertEqual(data, icon)
        XCTAssertEqual(transport.requests.map(\.url?.host), ["cdn-ak2.favicon.st-hatena.com"])
    }

    func testFallsBackWhenHatenaFindsNoIcon() async {
        let transport = StubTransport { request in
            request.url?.host == "icons.duckduckgo.com" ? (200, duckIcon) : (404, Favicon.hatenaPlaceholder)
        }

        let data = await FaviconClient(http: .stub(transport)).icon(for: page)

        XCTAssertEqual(data, duckIcon)
        XCTAssertEqual(transport.requests.map(\.url?.absoluteString).last, "https://icons.duckduckgo.com/ip3/example.com.ico")
    }

    func testFallsBackWhenHatenaSendsItsPlaceholderWith200() async {
        // What Hatena does for a site whose favicon is an SVG.
        let transport = StubTransport { request in
            request.url?.host == "icons.duckduckgo.com" ? (200, duckIcon) : (200, Favicon.hatenaPlaceholder)
        }

        let data = await FaviconClient(http: .stub(transport)).icon(for: page)

        XCTAssertEqual(data, duckIcon)
    }

    func testHasNoIconWhenNeitherServiceHasOne() async {
        let transport = StubTransport { request in
            request.url?.host == "icons.duckduckgo.com" ? (404, duckIcon) : (404, Favicon.hatenaPlaceholder)
        }

        let data = await FaviconClient(http: .stub(transport)).icon(for: page)

        XCTAssertNil(data)
        XCTAssertEqual(transport.requests.count, 2)
    }

    func testPlaceholderIsTheImageHatenaSends() {
        // 16 x 16 PNG, as served in 2026-11.
        XCTAssertEqual(Favicon.hatenaPlaceholder.count, 374)
        XCTAssertEqual(Favicon.hatenaPlaceholder.prefix(4), Data([0x89, 0x50, 0x4E, 0x47]))
    }
}
