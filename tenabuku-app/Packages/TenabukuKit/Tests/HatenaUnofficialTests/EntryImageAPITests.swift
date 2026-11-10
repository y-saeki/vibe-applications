import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

// The shape of b.hatena.ne.jp/api/entry/info?url=<url> as served in 2026-11,
// cut down to a few of its fields, with made-up content.
private let info = #"""
{"title":"例","canonical_url":"https://example.com/","total_bookmarks":117,"total_comments":31,
 "image":"https://cdn-ak-scissors.b.st-hatena.com/image/square/abc/backend=imagemagick;height=288;version=1;width=512/https%3A%2F%2Fexample.com%2Fog.jpg",
 "category":{"title":"テクノロジー","path":"it"},"created_at":"2026-11-09T22:53:05Z"}
"""#

final class EntryImageAPITests: XCTestCase {
    private let page = URL(string: "https://example.com/a?b=c")!

    func testReadsTheImageOfTheEntry() async throws {
        let transport = StubTransport(body: info)

        let image = try await EntryImageAPI(http: .stub(transport)).imageURL(for: page)

        XCTAssertEqual(image?.host(), "cdn-ak-scissors.b.st-hatena.com")
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/api/entry/info?url=https%3A%2F%2Fexample.com%2Fa%3Fb%3Dc"
        )
    }

    func testAnEntryWithoutAnImageHasNone() async throws {
        let api = EntryImageAPI(http: .stub(StubTransport(body: #"{"title":"例","image":""}"#)))

        let image = try await api.imageURL(for: page)

        XCTAssertNil(image)
    }

    func testAnUnbookmarkedPageHasNoImage() async throws {
        let api = EntryImageAPI(http: .stub(StubTransport(status: 404, body: "404 Not Found")))

        let image = try await api.imageURL(for: page)

        XCTAssertNil(image)
    }

    func testAChangedShapeIsReportedAsABreakage() async throws {
        let api = EntryImageAPI(http: .stub(StubTransport(body: "[]")))

        do {
            _ = try await api.imageURL(for: page)
            XCTFail("should throw")
        } catch HatenaError.parseFailed(let source, _) {
            XCTAssertEqual(source, EntryImageAPI.source)
        }
    }
}
