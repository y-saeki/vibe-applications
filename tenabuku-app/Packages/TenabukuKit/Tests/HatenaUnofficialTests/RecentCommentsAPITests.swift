import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

// The shape of b.hatena.ne.jp/api/entry/<url>/bookmarks?commented_only=1 as
// served in 2026-11, with made-up content.
private let firstPage = #"""
{"bookmarks":[
  {"created":"2026-11-05T19:04:25Z","location_id":"4795164888309493922",
   "user":{"country":"jp","tz":"Asia/Tokyo","name":"alice","lang":"ja","status":1,
           "image":{"image_url":"https://cdn.profile-image.st-hatena.com/users/alice/profile.png",
                    "small_thumbnail_url":"https://cdn.profile-image.st-hatena.com/users/alice/profile_s.gif"}},
   "entry":null,"comment_expanded":"なるほど","comment":"なるほど","tags":["あとで"],
   "status":"public","url":"https://example.com/","should_nofollow":"1"},
  {"created":"2026-11-05T18:52:46Z","user":{"name":"bob"},"comment":"同意","tags":[]}
 ],
 "cursor":"MTc5MzQyMTUxMiwyNzY2ODg2MzA2MzU4MDE5Mw=="}
"""#

// The last page answers `"cursor": null`.
private let lastPage = #"""
{"bookmarks":[{"created":"2026-11-05T17:44:32Z","user":{"name":"carol"},"comment":"最後","tags":[]}],"cursor":null}
"""#

final class RecentCommentsAPITests: XCTestCase {
    private let entry = URL(string: "https://example.com/")!

    func testDecodesCommentsAndCursorAndAsksForCommentsOnly() async throws {
        let transport = StubTransport(body: firstPage)

        let page = try await RecentCommentsAPI(http: .stub(transport)).comments(for: entry)

        XCTAssertEqual(page.comments.map(\.user), ["alice", "bob"])
        XCTAssertEqual(page.comments[0].comment, "なるほど")
        XCTAssertEqual(page.comments[0].tags, ["あとで"])
        XCTAssertEqual(page.comments[0].iconURL, URL(string: "https://cdn.profile-image.st-hatena.com/users/alice/profile.png"))
        XCTAssertEqual(page.comments[0].date, HatenaDate.parseISO8601("2026-11-05T19:04:25Z"))
        XCTAssertNil(page.comments[1].iconURL)
        // 19:04 UTC on the 5th is the 6th in Japan, the day the permalink names.
        XCTAssertEqual(page.comments[0].starURI, URL(string: "https://b.hatena.ne.jp/alice/20261106#bookmark-4795164888309493922"))
        XCTAssertNil(page.comments[1].starURI)
        XCTAssertEqual(page.nextCursor, "MTc5MzQyMTUxMiwyNzY2ODg2MzA2MzU4MDE5Mw==")
        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(
            request.url?.absoluteString,
            "https://b.hatena.ne.jp/api/entry/https%3A%2F%2Fexample.com%2F/bookmarks?commented_only=1&limit=100"
        )
        // A pull to refresh must not be answered from the device's cache.
        XCTAssertEqual(request.cachePolicy, .reloadIgnoringLocalCacheData)
    }

    func testCursorIsPassedOn() {
        XCTAssertEqual(
            RecentCommentsAPI.url(for: entry, cursor: "ab==").absoluteString,
            "https://b.hatena.ne.jp/api/entry/https%3A%2F%2Fexample.com%2F/bookmarks?commented_only=1&limit=100&cursor=ab%3D%3D"
        )
    }

    func testTheLastPageHasNoCursor() async throws {
        let page = try await RecentCommentsAPI(http: .stub(StubTransport(body: lastPage))).comments(for: entry, cursor: "ab==")

        XCTAssertEqual(page.comments.map(\.user), ["carol"])
        XCTAssertNil(page.nextCursor)
    }

    func testAnUnescapedControlCharacterInACommentIsRead() async throws {
        let body = #"{"bookmarks":[{"user":{"name":"alice"},"comment":"一行目"#
            + "\u{0B}" + #"二行目"}],"cursor":null}"#

        let page = try await RecentCommentsAPI(http: .stub(StubTransport(body: body))).comments(for: entry)

        XCTAssertEqual(page.comments.first?.comment, "一行目 二行目")
    }

    func testChangedShapeIsABreakageOfThisSource() async {
        do {
            _ = try await RecentCommentsAPI(http: .stub(StubTransport(body: #"{"items":[]}"#))).comments(for: entry)
            XCTFail("should throw")
        } catch {
            guard case let .parseFailed(source, _)? = error as? HatenaError else {
                return XCTFail("unexpected \(error)")
            }
            XCTAssertEqual(source, "unofficial.entry-bookmarks")
        }
    }
}
