import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

/// One comment as b.hatena.ne.jp/entry pages wrote it in 2026-11, with
/// made-up content and the menus left out.
private func item(user: String, text: String, tags: [String] = [], date: String = "2026-11-04T04:28:19Z") -> String {
    let tagItems = tags.map { #"<li class=""><a href="/\#(user)/\#($0)/" data-gtm-label="entry-popular-tags">\#($0)</a></li>"# }
    return #"""
    <div class="entry-comment-contents  js-ignorable-user-content js-bookmark-item js-user-bookmark-comment" data-user-name="\#(user)" data-url="https://example.com/"  itemprop="comment" itemtype="https://schema.org/Comment" itemscope >
      <div class="entry-comment-contents-main">
        <a href="/\#(user)/" class="entry-user-icon" data-gtm-label="entry-popular-icon"><img src="https://cdn.profile-image.st-hatena.com/users/\#(user)/profile.png"/ alt="\#(user)" title="\#(user)"></a>
        <div class="entry-comments-contents-body">
            <span class="entry-comment-username"  itemprop="author" itemtype="https://schema.org/Person" itemscope ><a href="/\#(user)/" data-gtm-label="entry-popular-username"   itemprop="url" ><span   itemprop="name" >\#(user)</span></a></span>
          <span class="entry-comment-text js-bookmark-comment"   itemprop="text" >\#(text)</span>
          <ul class="entry-comment-tags">
                \#(tagItems.joined(separator: "\n"))
          </ul>
          <div class="entry-comment-contents-foot">
            <p class="entry-comment-meta">
              <span class="entry-comment-timestamp"><a class="js-bookmark-anchor-path" data-gtm-label="entry-popular-timestamp" href="https://b.hatena.ne.jp/\#(user)/20261104#bookmark-1"  itemprop="datePublished" content="\#(date)" >2026/11/04</a></span>
            </p>
          </div>
        </div>
      </div>
    </div>
    """#
}

private func page(popular: [String], recent: [String]) -> String {
    #"""
    <html><body>
    <div class="entry-comment-tabs js-bookmarks-sort-tabs">
      <li class="js-bookmarks-sort-tab is-active" data-sort="popular">注目コメント</li>
      <li class="js-bookmarks-sort-tab" data-sort="recent">新着コメント</li>
    </div>
    <div class="bookmarks-sort-panels js-bookmarks-sort-panels">
      <div class="is-active bookmarks-sort-panel js-bookmarks-sort-panel" data-sort="popular">
        <div class="js-bookmarks js-bookmarks-popular">
    \#(popular.joined(separator: "\n"))
        </div>
      </div>
      <div class="bookmarks-sort-panel js-bookmarks-sort-panel js-bookmark-autoloader-recent" data-sort="recent">
        <div class="js-bookmarks js-bookmarks-recent">
    \#(recent.joined(separator: "\n"))
        </div>
      </div>
    </div>
    </body></html>
    """#
}

final class PopularCommentsAPITests: XCTestCase {
    private let entry = URL(string: "https://example.com/")!

    func testReadsThePopularPanelOnlyInPageOrder() async throws {
        let html = page(
            popular: [
                item(user: "alice", text: "なるほど", tags: ["あとで", "!!"]),
                item(user: "bob", text: "同意"),
            ],
            recent: [item(user: "carol", text: "新しいコメント")]
        )
        let transport = StubTransport(body: html)

        let comments = try await PopularCommentsAPI(http: .stub(transport)).comments(for: entry)

        XCTAssertEqual(comments.map(\.user), ["alice", "bob"])
        XCTAssertEqual(comments[0].comment, "なるほど")
        XCTAssertEqual(comments[0].tags, ["あとで", "!!"])
        XCTAssertEqual(comments[1].tags, [])
        XCTAssertEqual(comments[0].iconURL, URL(string: "https://cdn.profile-image.st-hatena.com/users/alice/profile.png"))
        XCTAssertEqual(comments[0].date, HatenaDate.parseISO8601("2026-11-04T04:28:19Z"))
        XCTAssertEqual(comments[1].starURI, URL(string: "https://b.hatena.ne.jp/bob/20261104#bookmark-1"))
        XCTAssertEqual(
            transport.requests.first?.url?.absoluteString,
            "https://b.hatena.ne.jp/entry?url=https%3A%2F%2Fexample.com%2F"
        )
    }

    func testCommentTextLosesLinksAndEntities() throws {
        let html = page(popular: [
            item(user: "alice", text: #"&quot;引用&quot; &amp; 補足 &#x2192; <a href="https://example.org/" target="_blank" rel="noopener nofollow">https://example.org/</a>"#),
        ], recent: [])

        XCTAssertEqual(try PopularCommentsAPI.parse(html).first?.comment, "\"引用\" & 補足 → https://example.org/")
    }

    func testAnEntryWithoutPopularCommentsIsEmpty() throws {
        XCTAssertEqual(try PopularCommentsAPI.parse(page(popular: [], recent: [item(user: "carol", text: "ひとこと")])), [])
    }

    func testAnUnbookmarkedURLIsEmpty() async throws {
        // Hatena redirects to the site's entry list, which has no comments at all.
        let transport = StubTransport { _ in (200, Data("<html>site</html>".utf8)) }
        let redirected = RedirectingTransport(to: URL(string: "https://b.hatena.ne.jp/site/example.com/")!, inner: transport)

        let comments = try await PopularCommentsAPI(http: HatenaHTTPClient(userAgent: "TenabukuTests", transport: redirected)).comments(for: entry)

        XCTAssertEqual(comments, [])
    }

    func testAnEntryPageWithoutTheSectionIsABreakageOfThisSource() async {
        do {
            _ = try await PopularCommentsAPI(http: .stub(StubTransport(body: "<html><body>renewed</body></html>"))).comments(for: entry)
            XCTFail("should throw")
        } catch {
            guard case let .parseFailed(source, _)? = error as? HatenaError else {
                return XCTFail("unexpected \(error)")
            }
            XCTAssertEqual(source, "unofficial.entry-page-popular")
        }
    }
}

/// Answers as URLSession does after following a redirect: the response names
/// where it ended up.
private struct RedirectingTransport: HTTPTransport {
    let to: URL
    let inner: StubTransport

    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        let (data, _) = try await inner.send(request)
        return (data, HTTPURLResponse(url: to, statusCode: 200, httpVersion: "HTTP/1.1", headerFields: nil)!)
    }
}
