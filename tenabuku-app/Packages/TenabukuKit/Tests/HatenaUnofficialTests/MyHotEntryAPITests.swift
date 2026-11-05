import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

// The shape of b.hatena.ne.jp/api/my/feed_digest as served to a signed-in
// browser in 2026-11, with made-up content.
private let digest = #"""
{"pager":{"next":{"label":"2026年11月4日","page_path":"/alice/hotentry?date=20261104",
                  "xhr_path":"/api/my/feed_digest?date=20261104"},
          "pages":[{"label":"2026年11月5日"}]},
 "item":[
  {"entry":{"entry_id":"4795395765970965410",
            "image_url":"https://cdn-ak-scissors.b.st-hatena.com/image/square/abc/https%3A%2F%2Fexample.com%2Fa.jpg",
            "url":"https://example.com/articles/1","created":"2026-11-04T20:41:10+09:00",
            "title":"一つ目の記事","count":2,"content":"本文の抜粋"},
   "entry_id":4795395765970966000,"score":217,"reactions":[],"service":"boston"},
  {"entry":{"entry_id":"4795395765970965411","image_url":null,
            "url":"https://example.org/2","created":"2026-11-04T09:00:00+09:00",
            "title":"二つ目の記事","count":"36","content":""},
   "entry_id":4795395765970966001,"score":106,"reactions":[],"service":"boston"}
 ]}
"""#

private func cookie(_ name: String, _ value: String, domain: String, path: String = "/", expires: Date? = nil) -> HTTPCookie {
    var properties: [HTTPCookiePropertyKey: Any] = [
        .name: name, .value: value, .domain: domain, .path: path,
    ]
    if let expires {
        properties[.expires] = expires
    }
    return HTTPCookie(properties: properties)!
}

private let loginCookies = [cookie("rk", "secret", domain: ".hatena.ne.jp")]

final class MyHotEntryAPITests: XCTestCase {
    func testDecodesEntriesInHatenasOrder() async throws {
        let entries = try await MyHotEntryAPI(http: .stub(StubTransport(body: digest)), cookies: loginCookies).entries()

        XCTAssertEqual(entries.map(\.title), ["一つ目の記事", "二つ目の記事"])
        XCTAssertEqual(entries[0].url, URL(string: "https://example.com/articles/1"))
        XCTAssertEqual(entries[0].summary, "本文の抜粋")
        XCTAssertEqual(entries[0].bookmarkCount, 2)
        XCTAssertEqual(entries[0].imageURL, URL(string: "https://cdn-ak-scissors.b.st-hatena.com/image/square/abc/https%3A%2F%2Fexample.com%2Fa.jpg"))
        XCTAssertEqual(entries[0].date, HatenaDate.parseISO8601("2026-11-04T11:41:10Z"))
        XCTAssertEqual(entries[1].bookmarkCount, 36)
        XCTAssertNil(entries[1].imageURL)
    }

    func testSendsTheWebsitesCookies() async throws {
        let transport = StubTransport(body: digest)
        let cookies = loginCookies + [cookie("b", "1", domain: "b.hatena.ne.jp")]

        _ = try await MyHotEntryAPI(http: .stub(transport), cookies: cookies).entries()

        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(request.url?.absoluteString, "https://b.hatena.ne.jp/api/my/feed_digest")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Cookie"), "rk=secret; b=1")
        XCTAssertFalse(request.httpShouldHandleCookies)
    }

    func testEmptyDigestIsNotABreakage() async throws {
        let entries = try await MyHotEntryAPI(http: .stub(StubTransport(body: #"{"pager":{},"item":[]}"#)), cookies: loginCookies).entries()

        XCTAssertEqual(entries, [])
    }

    func testLoginRequiredIsUnauthorized() async {
        let transport = StubTransport(status: 403, body: #"{"reason":"login required"}"#)
        do {
            _ = try await MyHotEntryAPI(http: .stub(transport), cookies: loginCookies).entries()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual(error as? HatenaError, .unauthorized)
        }
    }

    func testNoCookiesIsUnauthorizedWithoutAsking() async {
        let transport = StubTransport(body: digest)
        let elsewhere = [cookie("sid", "x", domain: "example.com")]
        do {
            _ = try await MyHotEntryAPI(http: .stub(transport), cookies: elsewhere).entries()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual(error as? HatenaError, .unauthorized)
        }
        XCTAssertTrue(transport.requests.isEmpty)
    }

    func testOtherStatusesStayHTTPErrors() async {
        do {
            _ = try await MyHotEntryAPI(http: .stub(StubTransport(status: 500, body: "")), cookies: loginCookies).entries()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual(error as? HatenaError, .httpStatus(500))
        }
    }

    func testChangedShapeIsABreakageOfThisSource() async {
        do {
            _ = try await MyHotEntryAPI(http: .stub(StubTransport(body: #"{"entries":[]}"#)), cookies: loginCookies).entries()
            XCTFail("should throw")
        } catch {
            guard case let .parseFailed(source, _)? = error as? HatenaError else {
                return XCTFail("unexpected \(error)")
            }
            XCTAssertEqual(source, "unofficial.my-hotentry")
        }
    }

    func testCookieHeaderKeepsOnlyCookiesTheEndpointWouldGet() {
        let now = Date(timeIntervalSince1970: 1_800_000_000)
        let cookies = [
            cookie("parent", "1", domain: ".hatena.ne.jp"),
            cookie("bare", "2", domain: "hatena.ne.jp"),
            cookie("host", "3", domain: "b.hatena.ne.jp"),
            cookie("sibling", "4", domain: "www.hatena.ne.jp"),
            cookie("lookalike", "5", domain: "nothatena.ne.jp"),
            cookie("otherPath", "6", domain: ".hatena.ne.jp", path: "/oauth"),
            cookie("expired", "7", domain: ".hatena.ne.jp", expires: now.addingTimeInterval(-1)),
            cookie("live", "8", domain: ".hatena.ne.jp", expires: now.addingTimeInterval(60)),
        ]

        XCTAssertEqual(
            WebCookies.header(for: MyHotEntryAPI.url, from: cookies, now: now),
            "parent=1; bare=2; host=3; live=8"
        )
        XCTAssertNil(WebCookies.header(for: MyHotEntryAPI.url, from: [], now: now))
    }
}
