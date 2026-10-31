import Foundation
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

// The shape of b.hatena.ne.jp/api/entry/<url>/bookmarks as served in
// 2026-10, with made-up content.
private let popularPage = #"""
{"bookmarks":[
  {"created":"2026-10-31T05:36:46Z","location_id":"4795164888309493922",
   "user":{"country":"jp","tz":"Asia/Tokyo","name":"alice","lang":"ja","status":1,
           "image":{"image_url":"https://cdn.profile-image.st-hatena.com/users/alice/profile.png",
                    "small_thumbnail_url":"https://cdn.profile-image.st-hatena.com/users/alice/profile_s.gif"}},
   "entry":null,"comment_expanded":"なるほど","comment":"なるほど","tags":["あとで"],
   "status":"public","url":"https://example.com/","should_nofollow":"1"},
  {"created":"2026-10-31T05:21:00Z","user":{"name":"bob"},"comment":"","tags":[]}
 ],
 "cursor":"MTc5MzQyMTUxMiwyNzY2ODg2MzA2MzU4MDE5Mw=="}
"""#

final class EntryCommentsAPITests: XCTestCase {
    func testDecodesCommentsAndCursor() async throws {
        let transport = StubTransport(body: popularPage)

        let page = try await EntryCommentsAPI(http: .stub(transport)).comments(for: URL(string: "https://example.com/")!, sort: .popular)

        XCTAssertEqual(page.comments.map(\.user), ["alice", "bob"])
        XCTAssertEqual(page.comments[0].comment, "なるほど")
        XCTAssertEqual(page.comments[0].tags, ["あとで"])
        XCTAssertEqual(page.comments[0].iconURL, URL(string: "https://cdn.profile-image.st-hatena.com/users/alice/profile.png"))
        XCTAssertNil(page.comments[1].iconURL)
        XCTAssertEqual(page.nextCursor, "MTc5MzQyMTUxMiwyNzY2ODg2MzA2MzU4MDE5Mw==")
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/api/entry/https%3A%2F%2Fexample.com%2F/bookmarks?sort=popular"
        )
    }

    func testCursorIsPassedOn() {
        XCTAssertEqual(
            EntryCommentsAPI.url(for: URL(string: "https://example.com/")!, sort: .recent, cursor: "ab==").absoluteString,
            "https://b.hatena.ne.jp/api/entry/https%3A%2F%2Fexample.com%2F/bookmarks?sort=recent&cursor=ab%3D%3D"
        )
    }

    func testChangedShapeIsABreakageOfThisSource() async {
        do {
            _ = try await EntryCommentsAPI(http: .stub(StubTransport(body: #"{"items":[]}"#)))
                .comments(for: URL(string: "https://example.com/")!, sort: .popular)
            XCTFail("should throw")
        } catch {
            guard case let .parseFailed(source, _)? = error as? HatenaError else {
                return XCTFail("unexpected \(error)")
            }
            XCTAssertEqual(source, "unofficial.entry-bookmarks")
        }
    }
}
