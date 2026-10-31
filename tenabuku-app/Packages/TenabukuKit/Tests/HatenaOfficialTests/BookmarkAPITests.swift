import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaOfficial
import HatenaShared
import HatenaTestSupport
import XCTest

// Response bodies follow the documented shapes; the real ones need sign-in
// and have not been captured yet.
final class BookmarkAPITests: XCTestCase {
    private let page = URL(string: "https://example.com/a?b=c")!

    private func api(_ transport: StubTransport) -> BookmarkAPI {
        BookmarkAPI(http: .stub(transport), signer: fixedSigner(), token: OAuthToken(token: "at", secret: "as"))
    }

    func testSavePostsCommentTagsAndPrivacy() async throws {
        let transport = StubTransport(body: #"""
        {"user":"sample","comment":"よい","tags":["swift","ios"],"private":0,
         "created_epoch":1793398719,"permalink":"https://b.hatena.ne.jp/sample/20261031#bookmark-1"}
        """#)

        let saved = try await api(transport).save(BookmarkDraft(url: page, comment: "よい", tags: ["swift", "ios"]))

        XCTAssertEqual(saved.tags, ["swift", "ios"])
        XCTAssertFalse(saved.isPrivate)
        XCTAssertEqual(saved.createdAt, Date(timeIntervalSince1970: 1_793_398_719))
        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.url?.absoluteString, "https://bookmark.hatenaapis.com/rest/1/my/bookmark")
        XCTAssertEqual(request.formBody.map { $0.0 }, ["url", "comment", "tags", "tags", "private"])
        XCTAssertEqual(request.formBody.map { $0.1 }, [page.absoluteString, "よい", "swift", "ios", "0"])
        XCTAssertNotNil(request.value(forHTTPHeaderField: "Authorization"))
    }

    func testMissingBookmarkIsNil() async throws {
        let transport = StubTransport(status: 404, body: "Not Found")
        let bookmark = try await api(transport).bookmark(for: page)
        XCTAssertNil(bookmark)
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://bookmark.hatenaapis.com/rest/1/my/bookmark?url=https%3A%2F%2Fexample.com%2Fa%3Fb%3Dc"
        )
    }

    func testDeleteToleratesAMissingBookmark() async throws {
        let transport = StubTransport(status: 404, body: "")
        try await api(transport).delete(page)
        XCTAssertEqual(transport.requests.first?.httpMethod, "DELETE")
    }

    func testExpiredTokenIsUnauthorized() async {
        do {
            _ = try await api(StubTransport(status: 401, body: "401 Unauthorized")).me()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual(error as? HatenaError, .unauthorized)
        }
    }

    func testSearchDecodesHits() async throws {
        let transport = StubTransport(body: #"""
        {"bookmarks":[{"entry":{"title":"T","count":"12","url":"https://example.com/","eid":"9","snippet":"s"},
                       "timestamp":1793398719,"comment":"c","is_private":1}],
         "meta":{"total":1,"query":{"original":"q","queries":["q"]},"status":200,"elapsed":0.01}}
        """#)

        let result = try await api(transport).search("q", limit: 500)

        XCTAssertEqual(result.total, 1)
        XCTAssertEqual(result.hits.first?.bookmarkCount, 12)
        XCTAssertEqual(result.hits.first?.isPrivate, true)
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/my/search/json?q=q&of=0&limit=100"
        )
    }
}

final class EntryInfoAPITests: XCTestCase {
    func testDecodesEntryAndJapanTimeStamps() async throws {
        let transport = StubTransport(body: #"""
        {"url":"https://example.com/","title":"例","eid":"4795164888309493922","count":280,
         "entry_url":"https://b.hatena.ne.jp/entry/s/example.com/","screenshot":"https://example.com/s.png",
         "requested_url":"https://example.com/",
         "bookmarks":[{"user":"alice","tags":["a"],"comment":"なるほど","timestamp":"2026/10/31 14:36"},
                      {"user":"bob","tags":[],"comment":"","timestamp":"2026/10/31 14:21"}]}
        """#)

        let entry = try await EntryInfoAPI(http: .stub(transport)).entry(for: URL(string: "https://example.com/")!)

        XCTAssertEqual(entry?.count, 280)
        XCTAssertEqual(entry?.bookmarks.map(\.user), ["alice", "bob"])
        XCTAssertEqual(entry?.bookmarks.first?.date, HatenaDate.parseISO8601("2026-10-31T05:36:00Z"))
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/entry/jsonlite/?url=https%3A%2F%2Fexample.com%2F"
        )
    }

    func testUnbookmarkedURLIsNil() async throws {
        let entry = try await EntryInfoAPI(http: .stub(StubTransport(body: "null"))).entry(for: URL(string: "https://example.com/")!)
        XCTAssertNil(entry)
    }

    func testUnexpectedShapeIsABreakage() async {
        do {
            _ = try await EntryInfoAPI(http: .stub(StubTransport(body: #"{"title":"x"}"#))).entry(for: URL(string: "https://example.com/")!)
            XCTFail("should throw")
        } catch {
            XCTAssertTrue((error as? HatenaError)?.isBreakage == true)
        }
    }
}
