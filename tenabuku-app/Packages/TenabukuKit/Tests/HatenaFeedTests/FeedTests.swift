import Foundation
@testable import HatenaFeed
import HatenaShared
import HatenaTestSupport
import XCTest

// Trimmed to the structure of b.hatena.ne.jp/hotentry/it.rss as served in
// 2026-10, with made-up content. Japanese text is written as character
// references, as Hatena does.
private let hotEntryFeed = #"""
<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF
 xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
 xmlns="http://purl.org/rss/1.0/"
 xmlns:content="http://purl.org/rss/1.0/modules/content/"
 xmlns:dc="http://purl.org/dc/elements/1.1/"
 xmlns:hatena="http://www.hatena.ne.jp/info/xmlns#"
 xmlns:taxo="http://purl.org/rss/1.0/modules/taxonomy/">
<channel rdf:about="https://b.hatena.ne.jp/hotentry/it">
<title>&#x306F;&#x3066;&#x306A;</title>
<link>https://b.hatena.ne.jp/hotentry/it</link>
<items><rdf:Seq><rdf:li rdf:resource="https://example.com/one" /></rdf:Seq></items>
</channel>
<item rdf:about="https://example.com/one">
<title>&#x30C6;&#x30B9;&#x30C8; &amp; more</title>
<link>https://example.com/one</link>
<description>summary</description>
<content:encoded>&#x3C;blockquote&#x3E;quoted&#x3C;/blockquote&#x3E;</content:encoded>
<dc:date>2026-10-30T22:18:39Z</dc:date>
<dc:subject>&#x30C6;&#x30AF;&#x30CE;&#x30ED;&#x30B8;&#x30FC;</dc:subject>
<dc:subject>AI</dc:subject>
<taxo:topics><rdf:Bag><rdf:li resource="https://b.hatena.ne.jp/q/AI" /></rdf:Bag></taxo:topics>
<hatena:bookmarkcount>123</hatena:bookmarkcount>
<hatena:bookmarkCommentListPageUrl>https://b.hatena.ne.jp/entry/s/example.com/one</hatena:bookmarkCommentListPageUrl>
<hatena:imageurl>https://example.com/one.png</hatena:imageurl>
</item>
<item rdf:about="https://example.com/two">
<title>two</title>
<link>https://example.com/two</link>
<dc:date>2026-10-30T21:00:00Z</dc:date>
<hatena:bookmarkcount>5</hatena:bookmarkcount>
</item>
</rdf:RDF>
"""#

private let emptyFeed = #"""
<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns="http://purl.org/rss/1.0/">
<channel rdf:about="https://b.hatena.ne.jp/hotentry/it"><title>t</title></channel>
</rdf:RDF>
"""#

final class RSS1ParserTests: XCTestCase {
    func testReadsHotEntryItems() throws {
        let items = try RSS1Parser.parse(Data(hotEntryFeed.utf8), source: "test")

        XCTAssertEqual(items.count, 2)
        let first = items[0]
        XCTAssertEqual(first.title, "テスト & more")
        XCTAssertEqual(first.link, URL(string: "https://example.com/one"))
        XCTAssertEqual(first.summary, "summary")
        XCTAssertEqual(first.subjects, ["テクノロジー", "AI"])
        XCTAssertEqual(first.bookmarkCount, 123)
        XCTAssertEqual(first.imageURL, URL(string: "https://example.com/one.png"))
        XCTAssertEqual(first.date, HatenaDate.parseISO8601("2026-10-30T22:18:39Z"))
        XCTAssertNil(items[1].imageURL)
    }

    func testHTMLIsABreakage() {
        XCTAssertThrowsError(try RSS1Parser.parse(Data("<html><body>maintenance</body></html>".utf8), source: "test")) {
            XCTAssertEqual(($0 as? HatenaError)?.isBreakage, true)
        }
    }

    func testItemWithoutTitleIsABreakage() {
        let feed = hotEntryFeed.replacingOccurrences(of: "<title>two</title>", with: "")
        XCTAssertThrowsError(try RSS1Parser.parse(Data(feed.utf8), source: "test")) {
            XCTAssertEqual($0 as? HatenaError, .parseFailed(source: "test", detail: "item without title or link: https://example.com/two"))
        }
    }
}

final class FeedClientTests: XCTestCase {
    func testHotEntriesComeFromTheCategoryFeed() async throws {
        let transport = StubTransport(body: hotEntryFeed)
        let items = try await FeedClient(http: .stub(transport)).hotEntries(.it)
        XCTAssertEqual(items.count, 2)
        XCTAssertEqual(transport.requests.first?.url?.absoluteString, "https://b.hatena.ne.jp/hotentry/it.rss")
    }

    func testEmptyHotEntriesAreABreakage() async {
        do {
            _ = try await FeedClient(http: .stub(StubTransport(body: emptyFeed))).hotEntries(.all)
            XCTFail("should throw")
        } catch {
            XCTAssertEqual((error as? HatenaError)?.isBreakage, true)
        }
    }

    func testEmptySearchIsFine() async throws {
        let items = try await FeedClient(http: .stub(StubTransport(body: emptyFeed))).search(SearchQuery(keyword: "x"))
        XCTAssertEqual(items, [])
    }

    func testSearchURLCarriesPeriodAndThreshold() {
        let query = SearchQuery(
            keyword: "Swift UI/設計",
            sort: .popular,
            since: HatenaDate.parseISO8601("2024-01-01T00:00:00+09:00"),
            until: HatenaDate.parseISO8601("2024-12-31T00:00:00+09:00"),
            minimumBookmarks: 50
        )
        XCTAssertEqual(
            query.url.absoluteString,
            "https://b.hatena.ne.jp/q/Swift%20UI%2F%E8%A8%AD%E8%A8%88?mode=rss&sort=popular&date_begin=2024-01-01&date_end=2024-12-31&users=50"
        )
    }

    func testUserFeedURL() {
        XCTAssertEqual(FeedClient.userFeedURL("sample", page: 1, tag: nil).absoluteString, "https://b.hatena.ne.jp/sample/bookmark.rss")
        XCTAssertEqual(
            FeedClient.userFeedURL("sample", page: 2, tag: "あとで読む").absoluteString,
            "https://b.hatena.ne.jp/sample/bookmark.rss?page=2&tag=%E3%81%82%E3%81%A8%E3%81%A7%E8%AA%AD%E3%82%80"
        )
    }

    func testEveryCategoryHasAName() {
        XCTAssertEqual(HotEntryCategory.allCases.count, 10)
        XCTAssertEqual(Set(HotEntryCategory.allCases.map(\.displayName)).count, 10)
    }
}
