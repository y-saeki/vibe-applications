import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

private let page = URL(string: "https://Example.com/a/b?c=1")!
private let hatenaIcon = Data([0x89, 0x50, 0x4E, 0x47, 0x01])
private let duckIcon = Data([0x00, 0x00, 0x01, 0x00, 0x02])
private let googleIcon = Data([0x89, 0x50, 0x4E, 0x47, 0x03])

private let hatenaHost = "cdn-ak2.favicon.st-hatena.com"
private let duckHost = "icons.duckduckgo.com"
private let googleHost = "www.google.com"

final class FaviconSourceTests: XCTestCase {
    func testAsksHatenaForTheEntryURLLikeTheWebsite() {
        let page = URL(string: "https://note.com/alice/n/n0123456789ab")!

        XCTAssertEqual(
            FaviconSource.hatena.url(for: page)?.absoluteString,
            "https://cdn-ak2.favicon.st-hatena.com/64?url=https%3A%2F%2Fnote.com%2Falice%2Fn%2Fn0123456789ab"
        )
    }

    func testEncodesTheQueryOfTheEntryURL() {
        let page = URL(string: "http://example.com/a?b=1&c=%E3%81%82#top")!

        XCTAssertEqual(
            FaviconSource.hatena.url(for: page)?.absoluteString,
            "https://cdn-ak2.favicon.st-hatena.com/64?url=http%3A%2F%2Fexample.com%2Fa%3Fb%3D1%26c%3D%25E3%2581%2582%23top"
        )
    }

    func testAsksDuckDuckGoForTheHostOnly() {
        XCTAssertEqual(
            FaviconSource.duckDuckGo.url(for: page)?.absoluteString,
            "https://icons.duckduckgo.com/ip3/example.com.ico"
        )
    }

    func testAsksGoogleForTheEntryURL() {
        XCTAssertEqual(
            FaviconSource.google.url(for: page)?.absoluteString,
            "https://www.google.com/s2/favicons?domain_url=https%3A%2F%2FExample.com%2Fa%2Fb%3Fc%3D1&sz=64"
        )
    }

    func testHasNoIconForURLsOffTheWeb() {
        for url in ["mailto:alice@example.com", "file:///tmp/a.html"] {
            for source in FaviconSource.allCases {
                XCTAssertNil(source.url(for: URL(string: url)!), "\(source) \(url)")
            }
        }
    }

    func testPlaceholderIsTheImageHatenaSends() {
        // 16 x 16 PNG, as served in 2026-11.
        XCTAssertEqual(FaviconSource.hatenaPlaceholder.count, 374)
        XCTAssertEqual(FaviconSource.hatenaPlaceholder.prefix(4), Data([0x89, 0x50, 0x4E, 0x47]))
    }
}

final class FaviconClientTests: XCTestCase {
    private func transport(
        hatena: (Int, Data) = (404, FaviconSource.hatenaPlaceholder),
        duck: (Int, Data) = (404, duckIcon),
        google: (Int, Data) = (404, googleIcon)
    ) -> StubTransport {
        StubTransport { request in
            switch request.url?.host {
            case hatenaHost: hatena
            case duckHost: duck
            default: google
            }
        }
    }

    func testStopsAtTheFirstSourceWithAnIcon() async {
        let stub = transport(hatena: (200, hatenaIcon), google: (200, googleIcon))

        let data = await FaviconClient(http: .stub(stub)).icon(for: page, from: [.hatena, .google])

        XCTAssertEqual(data, hatenaIcon)
        XCTAssertEqual(stub.requests.map(\.url?.host), [hatenaHost])
    }

    func testMovesOnWhenHatenaFindsNoIcon() async {
        let stub = transport(google: (200, googleIcon))

        let data = await FaviconClient(http: .stub(stub)).icon(for: page, from: [.hatena, .google])

        XCTAssertEqual(data, googleIcon)
        XCTAssertEqual(stub.requests.map(\.url?.host), [hatenaHost, googleHost])
    }

    func testMovesOnWhenHatenaSendsItsPlaceholderWith200() async {
        // What Hatena does for a page whose icon is an SVG.
        let stub = transport(hatena: (200, FaviconSource.hatenaPlaceholder), google: (200, googleIcon))

        let data = await FaviconClient(http: .stub(stub)).icon(for: page, from: [.hatena, .google])

        XCTAssertEqual(data, googleIcon)
    }

    func testAsksOnlyTheSourcesGivenInTheirOrder() async {
        let stub = transport(duck: (200, duckIcon), google: (200, googleIcon))

        let data = await FaviconClient(http: .stub(stub)).icon(for: page, from: [.google, .duckDuckGo])

        XCTAssertEqual(data, googleIcon)
        XCTAssertEqual(stub.requests.map(\.url?.host), [googleHost])
    }

    func testHasNoIconWhenNoSourceHasOne() async {
        let stub = transport()

        let data = await FaviconClient(http: .stub(stub)).icon(for: page, from: [.hatena, .duckDuckGo, .google])

        XCTAssertNil(data)
        XCTAssertEqual(stub.requests.count, 3)
    }

    func testAsksNothingWithNoSources() async {
        let stub = transport(hatena: (200, hatenaIcon))

        let data = await FaviconClient(http: .stub(stub)).icon(for: page, from: [])

        XCTAssertNil(data)
        XCTAssertTrue(stub.requests.isEmpty)
    }
}

final class FaviconPreferencesTests: XCTestCase {
    func testDefaultAsksHatenaThenGoogleWithDuckDuckGoOff() {
        let preferences = FaviconPreferences.default

        XCTAssertEqual(preferences.entries.map(\.source), [.hatena, .duckDuckGo, .google])
        XCTAssertEqual(preferences.enabledSources, [.hatena, .google])
    }

    func testTurnsSourcesOnAndOff() {
        var preferences = FaviconPreferences.default
        preferences.setEnabled(true, for: .duckDuckGo)
        preferences.setEnabled(false, for: .hatena)

        XCTAssertEqual(preferences.enabledSources, [.duckDuckGo, .google])
    }

    func testMovesLikeSwiftUIsOnMove() {
        var preferences = FaviconPreferences.default
        // Dragging Google to the top.
        preferences.move(fromOffsets: [2], toOffset: 0)
        XCTAssertEqual(preferences.entries.map(\.source), [.google, .hatena, .duckDuckGo])

        // Dragging the top one to the bottom.
        preferences.move(fromOffsets: [0], toOffset: 3)
        XCTAssertEqual(preferences.entries.map(\.source), [.hatena, .duckDuckGo, .google])

        // Two rows at once, dropped between the others: they keep their order.
        preferences.move(fromOffsets: [0, 2], toOffset: 2)
        XCTAssertEqual(preferences.entries.map(\.source), [.duckDuckGo, .hatena, .google])
    }

    func testKeepsEverySourceOnce() {
        let preferences = FaviconPreferences(entries: [
            .init(source: .google, isEnabled: true),
            .init(source: .google, isEnabled: false),
        ])

        XCTAssertEqual(preferences.entries.map(\.source), [.google, .hatena, .duckDuckGo])
        XCTAssertEqual(preferences.enabledSources, [.google])
    }

    func testReadsBackWhatItSaved() throws {
        var preferences = FaviconPreferences.default
        preferences.move(fromOffsets: [2], toOffset: 0)
        preferences.setEnabled(true, for: .duckDuckGo)

        let data = try JSONEncoder().encode(preferences)

        XCTAssertEqual(try JSONDecoder().decode(FaviconPreferences.self, from: data), preferences)
    }

    func testSkipsSourcesItDoesNotKnow() throws {
        let json = #"{"entries":[{"source":"bing","isEnabled":true},{"source":"google","isEnabled":true}]}"#

        let preferences = try JSONDecoder().decode(FaviconPreferences.self, from: Data(json.utf8))

        XCTAssertEqual(preferences.entries.map(\.source), [.google, .hatena, .duckDuckGo])
        XCTAssertEqual(preferences.enabledSources, [.google])
    }
}
