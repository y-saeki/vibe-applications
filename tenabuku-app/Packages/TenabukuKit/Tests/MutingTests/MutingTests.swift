import Foundation
import Muting
import XCTest

final class MuteFilterTests: XCTestCase {
    private func muted(_ rule: MuteRule, title: String = "記事", url: String) -> Bool {
        MuteFilter(rules: [rule]).isMuted(title: title, url: URL(string: url)!)
    }

    private func url(_ pattern: String, _ syntax: MuteRule.Syntax = .wildcard) -> MuteRule {
        MuteRule(target: .url, syntax: syntax, pattern: pattern)
    }

    private func title(_ pattern: String, _ syntax: MuteRule.Syntax = .wildcard) -> MuteRule {
        MuteRule(target: .title, syntax: syntax, pattern: pattern)
    }

    func testDomainCoversItsSubdomains() {
        let rule = url("example.com")
        XCTAssertTrue(muted(rule, url: "https://example.com/a"))
        XCTAssertTrue(muted(rule, url: "https://www.example.com/a"))
        XCTAssertTrue(muted(rule, url: "http://blog.example.com/"))
        XCTAssertFalse(muted(rule, url: "https://notexample.com/a"))
        XCTAssertFalse(muted(rule, url: "https://example.com.example.net/a"))
        XCTAssertFalse(muted(rule, url: "https://other.net/example.com"))
    }

    func testDomainWildcard() {
        let rule = url("*.example.com")
        XCTAssertTrue(muted(rule, url: "https://www.example.com/"))
        XCTAssertFalse(muted(rule, url: "https://example.com/"))

        XCTAssertTrue(muted(url("*.hatenablog.*"), url: "https://someone.hatenablog.jp/entry/1"))
    }

    func testPathIsMatchedFromTheStartPastTheScheme() {
        let rule = url("example.com/blog/")
        XCTAssertTrue(muted(rule, url: "https://example.com/blog/2026/11/03"))
        XCTAssertFalse(muted(rule, url: "https://example.com/about"))
        XCTAssertFalse(muted(rule, url: "https://www.example.com/blog/1"))

        XCTAssertTrue(muted(url("https://example.com/blog/"), url: "http://example.com/blog/1"))
        XCTAssertTrue(muted(url("*/amp/"), url: "https://news.example.com/amp/1"))
    }

    func testQuestionMarkIsLiteralInWildcards() {
        let rule = url("example.com/view?id=1")
        XCTAssertTrue(muted(rule, url: "https://example.com/view?id=1"))
        XCTAssertFalse(muted(rule, url: "https://example.com/viewXid=1"))
    }

    func testURLWildcardIgnoresCase() {
        XCTAssertTrue(muted(url("Example.COM"), url: "https://www.example.com/"))
    }

    func testURLRegexSearchesTheWholeURL() {
        let rule = url(#"^https://[^/]+\.hatenablog\.com/"#, .regex)
        XCTAssertTrue(muted(rule, url: "https://someone.hatenablog.com/entry/1"))
        XCTAssertFalse(muted(rule, url: "http://someone.hatenablog.com/entry/1"))
        XCTAssertTrue(muted(url("/amp/", .regex), url: "https://example.com/amp/1"))
    }

    func testTitleWildcardIsAKeywordAnywhere() {
        let rule = title("PR")
        XCTAssertTrue(muted(rule, title: "【PR】新しい製品", url: "https://example.com/"))
        XCTAssertTrue(muted(rule, title: "【ｐｒ】新しい製品", url: "https://example.com/"))
        XCTAssertFalse(muted(rule, title: "新しい製品", url: "https://example.com/pr"))

        let starred = title("Apple*発表")
        XCTAssertTrue(muted(starred, title: "Appleが新製品を発表", url: "https://example.com/"))
        XCTAssertFalse(muted(starred, title: "発表: Apple", url: "https://example.com/"))
    }

    func testTitleRegex() {
        let rule = title(#"^\[速報\]"#, .regex)
        XCTAssertTrue(muted(rule, title: "[速報] 何かが起きた", url: "https://example.com/"))
        XCTAssertFalse(muted(rule, title: "まとめ [速報]", url: "https://example.com/"))
    }

    func testAnyRuleMutes() {
        let filter = MuteFilter(rules: [url("example.com"), title("広告")])
        XCTAssertTrue(filter.isMuted(title: "記事", url: URL(string: "https://example.com/")!))
        XCTAssertTrue(filter.isMuted(title: "広告です", url: URL(string: "https://other.net/")!))
        XCTAssertFalse(filter.isMuted(title: "記事", url: URL(string: "https://other.net/")!))
        XCTAssertFalse(MuteFilter(rules: []).isMuted(title: "記事", url: URL(string: "https://other.net/")!))
    }

    func testABrokenRuleDoesNotStopTheOthers() {
        let filter = MuteFilter(rules: [title("(", .regex), title("広告")])
        XCTAssertTrue(filter.isMuted(title: "広告です", url: URL(string: "https://example.com/")!))
        XCTAssertFalse(filter.isMuted(title: "(", url: URL(string: "https://example.com/")!))
    }

    func testValidation() {
        XCTAssertThrowsError(try title("  ").validate()) { XCTAssertEqual($0 as? MuteRuleError, .emptyPattern) }
        XCTAssertThrowsError(try url("https://").validate()) { XCTAssertEqual($0 as? MuteRuleError, .emptyPattern) }
        XCTAssertThrowsError(try title("(", .regex).validate()) { XCTAssertEqual($0 as? MuteRuleError, .invalidRegex) }
        XCTAssertNoThrow(try title("(").validate())
        XCTAssertNoThrow(try url("example.com").validate())
    }
}

final class MuteListTests: XCTestCase {
    func testUpsertReplacesByIdOrAppends() {
        var list = MuteList()
        var rule = MuteRule(target: .title, syntax: .wildcard, pattern: "広告")
        list.upsert(rule)
        list.upsert(MuteRule(target: .url, syntax: .wildcard, pattern: "example.com"))
        rule.pattern = "PR"
        list.upsert(rule)
        XCTAssertEqual(list.rules.map(\.pattern), ["PR", "example.com"])

        list.remove(id: rule.id)
        XCTAssertEqual(list.rules.map(\.pattern), ["example.com"])
    }
}

final class MuteListFileTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("MuteListFileTests-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
    }

    func testLoadsEmptyWhenNothingWasSaved() throws {
        let file = MuteListFile(url: directory.appendingPathComponent("MuteRules.json"))
        XCTAssertEqual(try file.load(), MuteList())
    }

    func testRoundTripsThroughAFileInANewDirectory() throws {
        let file = MuteListFile(url: directory.appendingPathComponent("nested/MuteRules.json"))
        let list = MuteList(rules: [
            MuteRule(target: .url, syntax: .wildcard, pattern: "example.com"),
            MuteRule(target: .title, syntax: .regex, pattern: #"^\[PR\]"#),
        ])
        try file.save(list)
        XCTAssertEqual(try file.load(), list)
    }

    func testReportsAFileItCannotRead() throws {
        let url = directory.appendingPathComponent("MuteRules.json")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data("not json".utf8).write(to: url)
        XCTAssertThrowsError(try MuteListFile(url: url).load())
    }
}
