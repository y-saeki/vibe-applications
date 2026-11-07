import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

private let alice = URL(string: "https://b.hatena.ne.jp/alice/20261106#bookmark-1")!
private let bob = URL(string: "https://b.hatena.ne.jp/bob/20261107#bookmark-1")!
private let carol = URL(string: "https://b.hatena.ne.jp/carol/20261107#bookmark-1")!

// The shape of s.hatena.ne.jp/entries.json as served in 2026-11, with
// made-up names. A long list is cut short with a bare number standing for
// the stars left out; a user who gave several stars at once has `count`.
// Comments nobody starred are not in `entries`.
private let entries = #"""
{"can_comment":0,"rks":null,"entries":[
  {"uri":"https://b.hatena.ne.jp/alice/20261106#bookmark-1","can_comment":0,
   "stars":[{"quote":"","name":"dave"},67,{"name":"erin","quote":"引用"}]},
  {"uri":"https://b.hatena.ne.jp/bob/20261107#bookmark-1","can_comment":0,
   "stars":[{"quote":"","name":"dave","count":"3"}],
   "colored_stars":[{"color":"green","stars":[{"name":"erin","quote":""},2]},
                    {"color":"red","stars":[{"name":"frank","quote":""}]},
                    {"color":"blue","stars":[]}]}
]}
"""#

private let loginCookies = [
    HTTPCookie(properties: [.name: "rk", .value: "secret", .domain: ".hatena.ne.jp", .path: "/"])!,
]

final class HatenaStarAPITests: XCTestCase {
    func testPermalinkUsesTheDayInJapan() {
        // 20:36 UTC on the 6th is already the 7th in Japan.
        let date = HatenaDate.parseISO8601("2026-11-06T20:36:06Z")!

        XCTAssertEqual(
            HatenaStarAPI.uri(user: "alice", eid: "4795475434958069730", date: date)?.absoluteString,
            "https://b.hatena.ne.jp/alice/20261107#bookmark-4795475434958069730"
        )
    }

    func testCountsEachColourAndNoneForTheUnstarred() async throws {
        let transport = StubTransport(body: entries)

        let counts = try await HatenaStarAPI(http: .stub(transport)).counts(for: [alice, bob, carol])

        XCTAssertEqual(counts, [
            alice: StarCount(yellow: 69, colored: []),
            bob: StarCount(yellow: 3, colored: [.init(color: "green", count: 3), .init(color: "red", count: 1)]),
            carol: .none,
        ])
        XCTAssertEqual(counts[bob]?.total, 7)
        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(
            request.url?.absoluteString,
            "https://s.hatena.ne.jp/entries.json?uri=https%3A%2F%2Fb.hatena.ne.jp%2Falice%2F20261106%23bookmark-1"
                + "&uri=https%3A%2F%2Fb.hatena.ne.jp%2Fbob%2F20261107%23bookmark-1"
                + "&uri=https%3A%2F%2Fb.hatena.ne.jp%2Fcarol%2F20261107%23bookmark-1&no_comments=1"
        )
        XCTAssertNil(request.value(forHTTPHeaderField: "Cookie"))
        XCTAssertEqual(request.cachePolicy, .reloadIgnoringLocalCacheData)
    }

    func testReadingDoesNotSendTheLogin() async throws {
        let transport = StubTransport(body: entries)

        _ = try await HatenaStarAPI(http: .stub(transport), cookies: loginCookies).counts(for: [alice])

        XCTAssertNil(transport.requests.first?.value(forHTTPHeaderField: "Cookie"))
        XCTAssertEqual(transport.requests.first?.httpShouldHandleCookies, false)
    }

    func testManyCommentsAreAskedAboutInBatches() async throws {
        let uris = (0..<45).map { URL(string: "https://b.hatena.ne.jp/user\($0)/20261107#bookmark-1")! }
        let transport = StubTransport(body: #"{"entries":[]}"#)

        let counts = try await HatenaStarAPI(http: .stub(transport)).counts(for: uris)

        XCTAssertEqual(counts.count, 45)
        XCTAssertEqual(transport.requests.count, 3)
        let asked = transport.requests.map { $0.url!.query!.components(separatedBy: "uri=").count - 1 }
        XCTAssertEqual(asked.sorted(), [5, 20, 20])
    }

    func testNoCommentsAskNothing() async throws {
        let transport = StubTransport(body: entries)

        let counts = try await HatenaStarAPI(http: .stub(transport)).counts(for: [])

        XCTAssertEqual(counts, [:])
        XCTAssertTrue(transport.requests.isEmpty)
    }

    func testChangedShapeIsABreakageOfThisSource() async {
        do {
            _ = try await HatenaStarAPI(http: .stub(StubTransport(body: #"{"entries":[{"uri":"x","stars":["dave"]}]}"#))).counts(for: [alice])
            XCTFail("should throw")
        } catch {
            guard case let .parseFailed(source, _)? = error as? HatenaError else {
                return XCTFail("unexpected \(error)")
            }
            XCTAssertEqual(source, "unofficial.star")
        }
    }

    func testAddingAsksForTheTokenThenSendsItWithTheLogin() async throws {
        let transport = StubTransport { request in
            if request.url!.path == "/entries.json" {
                return (200, Data(#"{"rks":"token123","entries":[]}"#.utf8))
            }
            return (200, Data(#"{"name":"me","quote":"","color":"yellow"}"#.utf8))
        }

        try await HatenaStarAPI(http: .stub(transport), cookies: loginCookies).addStar(to: alice)

        XCTAssertEqual(transport.requests.count, 2)
        XCTAssertEqual(transport.requests[0].value(forHTTPHeaderField: "Cookie"), "rk=secret")
        let add = transport.requests[1]
        XCTAssertEqual(
            add.url?.absoluteString,
            "https://s.hatena.ne.jp/star.add.json?uri=https%3A%2F%2Fb.hatena.ne.jp%2Falice%2F20261106%23bookmark-1&rks=token123"
        )
        XCTAssertEqual(add.value(forHTTPHeaderField: "Cookie"), "rk=secret")
        XCTAssertFalse(add.httpShouldHandleCookies)
    }

    func testAddingWithoutCookiesIsUnauthorizedWithoutAsking() async {
        let transport = StubTransport(body: entries)
        await assertThrows(.unauthorized) {
            try await HatenaStarAPI(http: .stub(transport)).addStar(to: alice)
        }
        XCTAssertTrue(transport.requests.isEmpty)
    }

    func testNoTokenMeansTheLoginHasExpired() async {
        // Signed out, entries.json answers `"rks": null`.
        let transport = StubTransport(body: entries)
        await assertThrows(.unauthorized) {
            try await HatenaStarAPI(http: .stub(transport), cookies: loginCookies).addStar(to: alice)
        }
        XCTAssertEqual(transport.requests.count, 1)
    }

    func testAGuestAnswerIsUnauthorized() async {
        let transport = StubTransport { request in
            request.url!.path == "/entries.json"
                ? (200, Data(#"{"rks":"token123","entries":[]}"#.utf8))
                : (200, Data(#"{"is_guest":1}"#.utf8))
        }
        await assertThrows(.unauthorized) {
            try await HatenaStarAPI(http: .stub(transport), cookies: loginCookies).addStar(to: alice)
        }
    }

    func testARefusalSaysWhy() async {
        let transport = StubTransport { request in
            request.url!.path == "/entries.json"
                ? (200, Data(#"{"rks":"token123","entries":[]}"#.utf8))
                : (200, Data(#"{"errors":["Invalid uri"]}"#.utf8))
        }
        await assertThrows(.rejected("Invalid uri")) {
            try await HatenaStarAPI(http: .stub(transport), cookies: loginCookies).addStar(to: alice)
        }
    }

    private func assertThrows(_ expected: HatenaError, _ body: () async throws -> Void, line: UInt = #line) async {
        do {
            try await body()
            XCTFail("should throw", line: line)
        } catch {
            XCTAssertEqual(error as? HatenaError, expected, line: line)
        }
    }
}
