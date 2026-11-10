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
        XCTAssertEqual(result.hits.first?.eid, "9")
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/my/search/json?q=q&of=0&limit=100"
        )
    }

    func testSearchStopsShortOfTheDepthItCanRead() async throws {
        let transport = StubTransport(body: #"{"bookmarks":[],"meta":{"total":0}}"#)

        _ = try await api(transport).search("https", offset: 9_980, limit: 50)

        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/my/search/json?q=https&of=9980&limit=20"
        )
    }

    func testTagsAcceptsABareArray() async throws {
        let transport = StubTransport(body: #"[{"tag":"swift","count":12},{"tag":"ios","count":"3"}]"#)

        let tags = try await api(transport).tags()

        XCTAssertEqual(tags, [UserTag(name: "swift", count: 12), UserTag(name: "ios", count: 3)])
        XCTAssertEqual(transport.requests.first?.url?.absoluteString, "https://bookmark.hatenaapis.com/rest/1/my/tags")
        XCTAssertEqual(transport.requests.first?.httpMethod, "GET")
    }

    func testTagsAcceptsAWrappedArray() async throws {
        let transport = StubTransport(body: #"{"tags":[{"tag":"swift","count":12}]}"#)
        let tags = try await api(transport).tags()
        XCTAssertEqual(tags, [UserTag(name: "swift", count: 12)])
    }

    func testTagsInAnUnknownShapeAreABreakage() async {
        do {
            _ = try await api(StubTransport(body: #"{"items":[]}"#)).tags()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual((error as? HatenaError)?.isBreakage, true)
        }
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

final class BookmarkDraftTests: XCTestCase {
    private let page = URL(string: "https://example.com/")!

    func testStartsFromTheSavedBookmark() {
        let saved = MyBookmark(user: "sample", comment: "c", tags: ["a"], isPrivate: true, createdAt: nil, permalink: nil)
        XCTAssertEqual(
            BookmarkDraft(url: page, editing: saved),
            BookmarkDraft(url: page, comment: "c", tags: ["a"], isPrivate: true)
        )
    }

    func testAddTagTrimsAndSkipsEmptyAndDuplicates() {
        var draft = BookmarkDraft(url: page)
        XCTAssertTrue(draft.addTag("  swift "))
        XCTAssertFalse(draft.addTag("swift"))
        XCTAssertFalse(draft.addTag("   "))
        XCTAssertEqual(draft.tags, ["swift"])
    }

    func testAddTagStopsAtTen() {
        var draft = BookmarkDraft(url: page, tags: (1...9).map { "t\($0)" })
        XCTAssertTrue(draft.addTag("t10"))
        XCTAssertFalse(draft.canAddTag)
        XCTAssertFalse(draft.addTag("t11"))
        XCTAssertEqual(draft.tags.count, BookmarkDraft.maxTags)

        draft.removeTag("t1")
        XCTAssertTrue(draft.canAddTag)
        XCTAssertFalse(draft.tags.contains("t1"))
    }
}

final class TagSuggestionTests: XCTestCase {
    private let tags = [
        UserTag(name: "iOS", count: 5),
        UserTag(name: "Swift", count: 30),
        UserTag(name: "SwiftUI", count: 8),
        UserTag(name: "あとで読む", count: 50),
        UserTag(name: "TaylorSwift", count: 40),
    ]

    private func names(_ input: String, excluding: [String] = []) -> [String] {
        UserTag.suggestions(from: tags, matching: input, excluding: excluding).map(\.name)
    }

    func testEmptyInputOffersEveryTagMostUsedFirst() {
        XCTAssertEqual(names(""), ["あとで読む", "TaylorSwift", "Swift", "SwiftUI", "iOS"])
    }

    func testPrefixMatchesComeBeforeOtherMatches() {
        XCTAssertEqual(names("swift"), ["Swift", "SwiftUI", "TaylorSwift"])
    }

    func testIgnoresCaseAndWidth() {
        XCTAssertEqual(names("ＩＯＳ"), ["iOS"])
    }

    func testLeavesOutTagsAlreadyChosen() {
        XCTAssertEqual(names("swift", excluding: ["Swift"]), ["SwiftUI", "TaylorSwift"])
    }

    func testNoMatchIsEmpty() {
        XCTAssertEqual(names("rust"), [])
    }
}

final class MyBookmarkPagesTests: XCTestCase {
    private func hit(_ path: String, comment: String = "") -> MyBookmarkSearchHit {
        MyBookmarkSearchHit(
            title: path,
            url: URL(string: "https://example.com/\(path)")!,
            bookmarkCount: 1,
            snippet: "",
            comment: comment,
            isPrivate: false,
            bookmarkedAt: nil
        )
    }

    private func page(total: Int, _ paths: String...) -> MyBookmarkSearchPage {
        MyBookmarkSearchPage(total: total, hits: paths.map { hit($0) })
    }

    func testReadsOnUntilTheTotal() {
        var pages = MyBookmarkPages(query: "")
        XCTAssertEqual(pages.nextRequests, [MyBookmarkSearchRequest(query: "", offset: 0)])
        XCTAssertNil(pages.total)

        pages.append(page(total: 3, "a", "b"))
        XCTAssertEqual(pages.nextOffset, 2)
        XCTAssertTrue(pages.hasMore)

        pages.append(page(total: 3, "c"))
        XCTAssertEqual(pages.hits.map(\.title), ["a", "b", "c"])
        XCTAssertFalse(pages.hasMore)
    }

    func testAnEmptyPageEndsTheList() {
        var pages = MyBookmarkPages(query: "")
        pages.append(page(total: 10, "a"))
        pages.append(page(total: 10))
        XCTAssertFalse(pages.hasMore)
    }

    func testRepeatedHitsAreShownOnceButStillAdvanceTheOffset() {
        var pages = MyBookmarkPages(query: "")
        pages.append(page(total: 4, "a", "b"))
        pages.append(page(total: 4, "b", "c"))
        XCTAssertEqual(pages.hits.map(\.title), ["a", "b", "c"])
        XCTAssertEqual(pages.nextOffset, 4)
    }

    func testRemovingAHitPullsTheOffsetBack() {
        var pages = MyBookmarkPages(query: "")
        pages.append(page(total: 3, "a", "b"))
        pages.remove(URL(string: "https://example.com/a")!)
        XCTAssertEqual(pages.hits.map(\.title), ["b"])
        XCTAssertEqual(pages.nextOffset, 1)
        XCTAssertEqual(pages.total, 2)
        XCTAssertTrue(pages.hasMore)
    }

    func testAPageAnsweringAnEarlierRequestIsDropped() {
        var pages = MyBookmarkPages(query: "swift")
        pages.receive(page(total: 3, "a"), for: MyBookmarkSearchRequest(query: "swift", offset: 0))
        pages.receive(page(total: 3, "a"), for: MyBookmarkSearchRequest(query: "swift", offset: 0))
        XCTAssertEqual(pages.hits.map(\.title), ["a"])
        XCTAssertEqual(pages.nextOffset, 1)
    }

    func testNothingPastTheSearchDepthIsAskedFor() {
        var pages = MyBookmarkPages(query: "swift")
        pages.append(MyBookmarkSearchPage(total: 20_000, hits: (0..<BookmarkAPI.searchDepth).map { hit("\($0)") }))
        XCTAssertEqual(pages.nextRequests, [])
    }

    func testUpdatingAHitTakesTheSavedCommentTagsAndPrivacy() {
        var pages = MyBookmarkPages(query: "")
        pages.append(page(total: 1, "a"))
        let saved = MyBookmark(user: "sample", comment: "よい", tags: ["swift", "ios"], isPrivate: true, createdAt: nil, permalink: nil)

        pages.update(URL(string: "https://example.com/a")!, with: saved)

        // The search writes tags into the comment, which the REST API keeps apart.
        XCTAssertEqual(pages.hits.first?.comment, "[swift][ios]よい")
        XCTAssertEqual(pages.hits.first?.isPrivate, true)
        XCTAssertEqual(pages.hits.first?.title, "a")
    }
}

final class MyBookmarkSearchHitCommentTests: XCTestCase {
    private func hit(_ comment: String) -> MyBookmarkSearchHit {
        MyBookmarkSearchHit(
            title: "", url: URL(string: "https://example.com/")!, bookmarkCount: 1,
            snippet: "", comment: comment, isPrivate: false, bookmarkedAt: nil
        )
    }

    func testSplitsTheLeadingTagsFromTheComment() {
        let hit = hit("[!!][あとで読む]なるほど[笑]")

        XCTAssertEqual(hit.tags, ["!!", "あとで読む"])
        XCTAssertEqual(hit.commentBody, "なるほど[笑]")
    }

    func testACommentWithoutTagsIsLeftAsItIs() {
        XCTAssertEqual(hit("なるほど").tags, [])
        XCTAssertEqual(hit("なるほど").commentBody, "なるほど")
    }

    func testAnUnclosedOrEmptyBracketIsPartOfTheComment() {
        XCTAssertEqual(hit("[a][b").tags, ["a"])
        XCTAssertEqual(hit("[a][b").commentBody, "[b")
        XCTAssertEqual(hit("[]です").tags, [])
        XCTAssertEqual(hit("[]です").commentBody, "[]です")
    }

    func testTagsAloneLeaveAnEmptyComment() {
        XCTAssertEqual(hit("[a]").tags, ["a"])
        XCTAssertEqual(hit("[a]").commentBody, "")
    }
}

final class MyBookmarkTimelineTests: XCTestCase {
    /// A hit bookmarked `minutesAgo` before a fixed moment.
    private func hit(_ path: String, _ minutesAgo: Int, scheme: String = "https") -> MyBookmarkSearchHit {
        MyBookmarkSearchHit(
            title: path,
            url: URL(string: "\(scheme)://example.com/\(path)")!,
            bookmarkCount: 1,
            snippet: "",
            comment: "",
            isPrivate: false,
            bookmarkedAt: Date(timeIntervalSince1970: 1_800_000_000 - TimeInterval(minutesAgo * 60))
        )
    }

    /// Answers the timeline's request for `query` with `hits`.
    private func answer(
        _ timeline: inout MyBookmarkTimeline, _ query: String, total: Int, _ hits: MyBookmarkSearchHit...
    ) {
        guard let request = timeline.nextRequests.first(where: { $0.query == query }) else {
            return XCTFail("\(query) was not asked for")
        }
        timeline.receive(MyBookmarkSearchPage(total: total, hits: hits), for: request)
    }

    func testAsksForBothSchemesTogetherBeforeShowingAnything() {
        var timeline = MyBookmarkTimeline()
        XCTAssertEqual(timeline.nextRequests, [
            MyBookmarkSearchRequest(query: "https", offset: 0),
            MyBookmarkSearchRequest(query: "http", offset: 0),
        ])

        answer(&timeline, "https", total: 2, hit("a", 1), hit("b", 3))
        XCTAssertEqual(timeline.hits, [])
        XCTAssertEqual(timeline.nextRequests, [MyBookmarkSearchRequest(query: "http", offset: 0)])
    }

    func testInterleavesBothSearchesNewestFirst() {
        var timeline = MyBookmarkTimeline()
        answer(&timeline, "https", total: 2, hit("a", 1), hit("c", 3))
        answer(&timeline, "http", total: 2, hit("b", 2, scheme: "http"), hit("d", 4, scheme: "http"))

        XCTAssertEqual(timeline.hits.map(\.title), ["a", "b", "c", "d"])
        XCTAssertFalse(timeline.hasMore)
        XCTAssertFalse(timeline.isCutShort)
    }

    func testWaitsForTheNextPageOfASearchThatRanDry() {
        var timeline = MyBookmarkTimeline()
        answer(&timeline, "https", total: 3, hit("a", 1), hit("c", 3))
        answer(&timeline, "http", total: 1, hit("b", 2, scheme: "http"))

        // "https" has a third hit that may be newer than anything waiting.
        XCTAssertEqual(timeline.hits.map(\.title), ["a", "b", "c"])
        XCTAssertEqual(timeline.nextRequests, [MyBookmarkSearchRequest(query: "https", offset: 2)])

        answer(&timeline, "https", total: 3, hit("e", 5))
        XCTAssertEqual(timeline.hits.map(\.title), ["a", "b", "c", "e"])
        XCTAssertFalse(timeline.hasMore)
    }

    func testTheSecondSearchMayAnswerFirst() {
        var timeline = MyBookmarkTimeline()
        answer(&timeline, "http", total: 1, hit("b", 2, scheme: "http"))
        answer(&timeline, "https", total: 1, hit("a", 1))

        XCTAssertEqual(timeline.hits.map(\.title), ["a", "b"])
    }

    func testABookmarkBothSearchesHoldIsShownOnce() {
        var timeline = MyBookmarkTimeline()
        answer(&timeline, "https", total: 2, hit("a", 1), hit("b", 2))
        answer(&timeline, "http", total: 1, hit("a", 1))

        XCTAssertEqual(timeline.hits.map(\.title), ["a", "b"])
    }

    func testEndsWhereASearchCanReadNoDeeper() {
        var timeline = MyBookmarkTimeline()
        let deep = (0..<BookmarkAPI.searchDepth).map { hit("s\($0)", $0 * 2) }
        timeline.receive(
            MyBookmarkSearchPage(total: 20_000, hits: deep),
            for: MyBookmarkSearchRequest(query: "https", offset: 0)
        )
        answer(&timeline, "http", total: 20_000, hit("old", BookmarkAPI.searchDepth * 3, scheme: "http"))

        XCTAssertEqual(timeline.hits.count, BookmarkAPI.searchDepth)
        XCTAssertTrue(timeline.isCutShort)
        XCTAssertEqual(timeline.nextRequests, [])
    }

    func testRemovingABookmarkPullsBackTheSearchesThatHeldIt() {
        var timeline = MyBookmarkTimeline()
        answer(&timeline, "https", total: 3, hit("a", 1), hit("c", 3))
        answer(&timeline, "http", total: 1, hit("b", 2, scheme: "http"))

        timeline.remove(URL(string: "https://example.com/a")!)

        XCTAssertEqual(timeline.hits.map(\.title), ["b", "c"])
        XCTAssertEqual(timeline.nextRequests, [MyBookmarkSearchRequest(query: "https", offset: 1)])
    }

    func testUpdatingABookmarkTakesTheSavedComment() {
        var timeline = MyBookmarkTimeline()
        answer(&timeline, "https", total: 1, hit("a", 1))
        answer(&timeline, "http", total: 0)
        let saved = MyBookmark(user: "sample", comment: "よい", tags: [], isPrivate: true, createdAt: nil, permalink: nil)

        timeline.update(URL(string: "https://example.com/a")!, with: saved)

        XCTAssertEqual(timeline.hits.first?.comment, "よい")
        XCTAssertEqual(timeline.hits.first?.isPrivate, true)
    }
}

final class MyBookmarkTimelineStoreTests: XCTestCase {
    private var url: URL!

    override func setUp() {
        url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
            .appendingPathComponent("MyBookmarks.json")
    }

    override func tearDown() {
        try? FileManager.default.removeItem(at: url.deletingLastPathComponent())
    }

    /// A timeline that has shown one bookmark and waits for more of "https".
    private func timeline() -> MyBookmarkTimeline {
        let hit = { (path: String, minutesAgo: Int) in
            MyBookmarkSearchHit(
                title: path,
                url: URL(string: "https://example.com/\(path)")!,
                bookmarkCount: 3,
                snippet: "本文",
                comment: "コメント",
                isPrivate: true,
                bookmarkedAt: Date(timeIntervalSince1970: 1_800_000_000 - TimeInterval(minutesAgo * 60))
            )
        }
        var timeline = MyBookmarkTimeline()
        timeline.receive(
            MyBookmarkSearchPage(total: 5, hits: [hit("a", 1)]),
            for: MyBookmarkSearchRequest(query: "https", offset: 0)
        )
        timeline.receive(
            MyBookmarkSearchPage(total: 0, hits: []),
            for: MyBookmarkSearchRequest(query: "http", offset: 0)
        )
        return timeline
    }

    func testASavedTimelineReadsOnFromWhereItWas() async throws {
        let store = MyBookmarkTimelineStore(url: url)
        let saved = MyBookmarkTimelineStore.Saved(user: "sample", savedAt: Date(timeIntervalSince1970: 1_800_000_000), timeline: timeline())

        try await store.save(saved)
        let loaded = await store.load(user: "sample")

        XCTAssertEqual(loaded, saved)
        XCTAssertEqual(loaded?.timeline.hits.map(\.title), ["a"])
        XCTAssertEqual(loaded?.timeline.nextRequests, [MyBookmarkSearchRequest(query: "https", offset: 1)])
    }

    func testAnotherUsersTimelineIsNotShown() async throws {
        let store = MyBookmarkTimelineStore(url: url)
        try await store.save(.init(user: "sample", savedAt: .now, timeline: timeline()))

        let loaded = await store.load(user: "someone")

        XCTAssertNil(loaded)
    }

    func testClearingForgetsTheTimeline() async throws {
        let store = MyBookmarkTimelineStore(url: url)
        try await store.save(.init(user: "sample", savedAt: .now, timeline: timeline()))

        try await store.clear()
        let loaded = await store.load(user: "sample")

        XCTAssertNil(loaded)
        try await store.clear()
    }

    func testATimelineSavedInAnEarlierFormatIsReadAgain() async throws {
        let store = MyBookmarkTimelineStore(url: url)
        try await store.save(.init(user: "sample", savedAt: .now, timeline: timeline()))
        // As written before the format was kept: no `format` at all.
        var object = try XCTUnwrap(JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
        object["format"] = nil
        try JSONSerialization.data(withJSONObject: object).write(to: url)

        let loaded = await store.load(user: "sample")

        XCTAssertNil(loaded)
    }

    func testAnUnreadableFileIsNoTimeline() async throws {
        try FileManager.default.createDirectory(at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
        try Data("{".utf8).write(to: url)

        let loaded = await MyBookmarkTimelineStore(url: url).load(user: "sample")

        XCTAssertNil(loaded)
    }
}
