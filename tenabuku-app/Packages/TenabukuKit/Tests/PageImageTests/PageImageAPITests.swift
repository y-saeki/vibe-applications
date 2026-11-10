import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared
import HatenaTestSupport
@testable import PageImage
import XCTest

// The heads of pages seen in 2026-11 whose image Hatena left out, with
// made-up content.
private let webpPage = #"""
<!DOCTYPE html><html><head><meta charset="utf-8">
<title>例</title>
<meta property="og:title" content="例"/>
<meta property="og:image" content="https://cdn.example.com/visual/1200x630.webp"/>
<meta property="og:image:width" content="1200"/>
</head><body></body></html>
"""#

final class OpenGraphImageTests: XCTestCase {
    private let page = URL(string: "https://example.com/entry/1")!

    func testReadsTheOpenGraphImage() {
        XCTAssertEqual(
            OpenGraphImage.find(in: webpPage, page: page)?.absoluteString,
            "https://cdn.example.com/visual/1200x630.webp"
        )
    }

    func testPrefersTheOpenGraphImageOverTheTwitterOne() {
        let html = #"""
        <head><meta name="twitter:image" content="https://example.com/t.png">
        <meta property="og:image" content="https://example.com/og.avif"></head>
        """#

        XCTAssertEqual(OpenGraphImage.find(in: html, page: page)?.lastPathComponent, "og.avif")
    }

    func testFallsBackToTheTwitterImage() {
        let html = #"<head><meta name="twitter:image" content="https://example.com/t.png"></head>"#

        XCTAssertEqual(OpenGraphImage.find(in: html, page: page)?.lastPathComponent, "t.png")
    }

    func testReadsAttributesInAnyOrderCaseAndQuoting() {
        let html = #"<HEAD><META content='https://example.com/a.jpg' PROPERTY=og:image></HEAD>"#

        XCTAssertEqual(OpenGraphImage.find(in: html, page: page)?.absoluteString, "https://example.com/a.jpg")
    }

    func testResolvesARelativeImageAgainstThePage() {
        let html = #"<head><meta property="og:image" content="/img/a.png"></head>"#
        let protocolRelative = #"<head><meta property="og:image" content="//cdn.example.net/b.png"></head>"#

        XCTAssertEqual(OpenGraphImage.find(in: html, page: page)?.absoluteString, "https://example.com/img/a.png")
        XCTAssertEqual(
            OpenGraphImage.find(in: protocolRelative, page: page)?.absoluteString,
            "https://cdn.example.net/b.png"
        )
    }

    func testEncodesAJapaneseFileName() {
        let html = #"<head><meta property="og:image" content="https://example.com/uploads/エントリー１-1.jpg" /></head>"#

        XCTAssertEqual(
            OpenGraphImage.find(in: html, page: page)?.absoluteString,
            "https://example.com/uploads/%E3%82%A8%E3%83%B3%E3%83%88%E3%83%AA%E3%83%BC%EF%BC%91-1.jpg"
        )
    }

    func testKeepsWhatIsAlreadyEncoded() {
        let html = #"<head><meta property="og:image" content="https://example.com/scale/https%3A%2F%2Fa.example%2Fb.avif"></head>"#

        XCTAssertEqual(
            OpenGraphImage.find(in: html, page: page)?.absoluteString,
            "https://example.com/scale/https%3A%2F%2Fa.example%2Fb.avif"
        )
    }

    func testDecodesCharacterReferences() {
        let html = #"<head><meta property="og:image" content="https://example.com/i?a=1&amp;b=2&#x26;c=3&#38;d=4"></head>"#

        XCTAssertEqual(
            OpenGraphImage.find(in: html, page: page)?.absoluteString,
            "https://example.com/i?a=1&b=2&c=3&d=4"
        )
    }

    func testIgnoresTagsInTheBody() {
        let html = #"<head></head><body><meta property="og:image" content="https://other.example/x.png"></body>"#

        XCTAssertNil(OpenGraphImage.find(in: html, page: page))
    }

    func testAPageWithoutAnImageHasNone() {
        let empty = #"<head><meta property="og:image" content=""></head>"#
        let notWeb = #"<head><meta property="og:image" content="data:image/png;base64,AAAA"></head>"#

        XCTAssertNil(OpenGraphImage.find(in: "<head><title>例</title></head>", page: page))
        XCTAssertNil(OpenGraphImage.find(in: empty, page: page))
        XCTAssertNil(OpenGraphImage.find(in: notWeb, page: page))
    }
}

final class PageImageAPITests: XCTestCase {
    private let page = URL(string: "https://example.com/entry/1")!

    func testReadsTheImageFromThePage() async throws {
        let transport = StubTransport(body: webpPage)

        let image = try await PageImageAPI(http: .stub(transport)).imageURL(for: page)

        XCTAssertEqual(image?.pathExtension, "webp")
        XCTAssertEqual(transport.requests.first?.url, page)
    }

    func testAnErrorStatusIsAnError() async {
        let api = PageImageAPI(http: .stub(StubTransport(status: 503, body: "")))

        do {
            _ = try await api.imageURL(for: page)
            XCTFail("should throw")
        } catch HatenaError.httpStatus(let status) {
            XCTAssertEqual(status, 503)
        } catch {
            XCTFail("unexpected \(error)")
        }
    }

    func testDoesNotAskForAnythingButTheWeb() async throws {
        let transport = StubTransport(body: webpPage)

        let image = try await PageImageAPI(http: .stub(transport)).imageURL(for: URL(string: "ftp://example.com/a")!)

        XCTAssertNil(image)
        XCTAssertTrue(transport.requests.isEmpty)
    }
}
