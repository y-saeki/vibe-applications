import Foundation
@testable import HatenaUnofficial
import XCTest

final class FaviconTests: XCTestCase {
    func testAsksForTheEntryURLLikeTheWebsite() {
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

    func testHasNoIconForURLsOffTheWeb() {
        XCTAssertNil(Favicon.url(for: URL(string: "mailto:alice@example.com")!))
        XCTAssertNil(Favicon.url(for: URL(string: "file:///tmp/a.html")!))
    }
}
