import EntryMute
import Foundation
import XCTest

final class MuteRuleTests: XCTestCase {
    private func muted(
        _ target: MuteRule.Target,
        _ syntax: MuteRule.Syntax,
        _ pattern: String,
        title: String = "記事のタイトル",
        url: String = "https://example.com/"
    ) throws -> Bool {
        try MuteMatcher(MuteRule(target: target, syntax: syntax, pattern: pattern))
            .matches(title: title, url: URL(string: url)!)
    }

    // MARK: Domain

    func testDomainCoversItsSubdomains() throws {
        XCTAssertTrue(try muted(.url, .wildcard, "example.com", url: "https://example.com/a"))
        XCTAssertTrue(try muted(.url, .wildcard, "example.com", url: "https://www.example.com/a"))
        XCTAssertTrue(try muted(.url, .wildcard, "example.com", url: "http://a.b.example.com"))
    }

    func testDomainDoesNotMatchAnotherThatMerelyEndsTheSame() throws {
        XCTAssertFalse(try muted(.url, .wildcard, "example.com", url: "https://notexample.com/"))
        XCTAssertFalse(try muted(.url, .wildcard, "example.com", url: "https://example.com.evil.jp/"))
    }

    func testDomainWildcards() throws {
        XCTAssertTrue(try muted(.url, .wildcard, "*.hatenablog.com", url: "https://someone.hatenablog.com/entry/1"))
        XCTAssertTrue(try muted(.url, .wildcard, "example.*", url: "https://example.co.jp/"))
        XCTAssertTrue(try muted(.url, .wildcard, "blog?.example.com", url: "https://blog2.example.com/"))
        XCTAssertFalse(try muted(.url, .wildcard, "blog?.example.com", url: "https://blog.example.com/"))
    }

    func testDomainIgnoresCaseAndAPastedScheme() throws {
        XCTAssertTrue(try muted(.url, .wildcard, "Example.COM", url: "https://www.example.com/"))
        XCTAssertTrue(try muted(.url, .wildcard, "https://example.com", url: "https://example.com/a"))
    }

    // MARK: URL

    func testURLWithAPathMatchesFromTheStart() throws {
        XCTAssertTrue(try muted(.url, .wildcard, "example.com/blog/", url: "https://example.com/blog/2026/1"))
        XCTAssertFalse(try muted(.url, .wildcard, "example.com/blog/", url: "https://example.com/news/blog/"))
        XCTAssertFalse(try muted(.url, .wildcard, "example.com/blog/", url: "https://www.example.com/blog/"))
    }

    func testURLWildcardInThePath() throws {
        XCTAssertTrue(try muted(.url, .wildcard, "*/ad/*", url: "https://news.example.jp/ad/123"))
        XCTAssertTrue(try muted(.url, .wildcard, "https://example.com/*?ref=", url: "https://example.com/a/b?ref=x"))
        XCTAssertFalse(try muted(.url, .wildcard, "*/ad/*", url: "https://example.com/adult/1"))
    }

    func testURLRegexSeesTheWholeURL() throws {
        XCTAssertTrue(try muted(.url, .regex, #"^https://[^/]+\.example\.com/"#, url: "https://a.example.com/x"))
        XCTAssertFalse(try muted(.url, .regex, #"^https://[^/]+\.example\.com/"#, url: "https://example.com/x"))
        XCTAssertTrue(try muted(.url, .regex, "EXAMPLE", url: "https://example.com/"))
    }

    // MARK: Title

    func testTitleKeywordAnywhere() throws {
        XCTAssertTrue(try muted(.title, .wildcard, "炎上", title: "あの件が炎上した理由"))
        XCTAssertFalse(try muted(.title, .wildcard, "炎上", title: "静かな話題"))
    }

    func testTitleWildcards() throws {
        XCTAssertTrue(try muted(.title, .wildcard, "【*】", title: "【速報】何かが起きた"))
        XCTAssertTrue(try muted(.title, .wildcard, "Swift*リリース", title: "Swift 7 がリリース"))
        XCTAssertFalse(try muted(.title, .wildcard, "Swift*リリース", title: "リリースされた Swift"))
    }

    func testTitleIgnoresCaseAndWidth() throws {
        XCTAssertTrue(try muted(.title, .wildcard, "iphone", title: "ＩＰｈｏｎｅの新機能"))
        XCTAssertTrue(try muted(.title, .wildcard, "ＡＩ", title: "生成AIの話"))
    }

    func testTitleRegex() throws {
        XCTAssertTrue(try muted(.title, .regex, #"^\d+選"#, title: "10選 おすすめの本"))
        XCTAssertFalse(try muted(.title, .regex, #"^\d+選"#, title: "おすすめの本 10選"))
        XCTAssertTrue(try muted(.title, .regex, "chatgpt|claude", title: "Claude の使い方"))
    }

    // MARK: Problems

    func testReportsEmptyPatternsAndBrokenRegexes() {
        XCTAssertEqual(MuteRule(target: .title, syntax: .wildcard, pattern: "  ").problem, .emptyPattern)
        guard case .invalidRegex? = MuteRule(target: .title, syntax: .regex, pattern: "(unclosed").problem else {
            return XCTFail("a regular expression that does not compile is reported")
        }
        XCTAssertNil(MuteRule(target: .title, syntax: .regex, pattern: "(closed)").problem)
        XCTAssertNil(MuteRule(target: .title, syntax: .wildcard, pattern: "(unclosed").problem)
    }
}

final class MuteListTests: XCTestCase {
    private let rule = MuteRule(target: .url, syntax: .wildcard, pattern: "example.com")

    func testSaveAddsThenReplaces() {
        var list = MuteList()
        list.save(rule)
        var edited = rule
        edited.pattern = "example.org"
        list.save(edited)
        XCTAssertEqual(list.rules, [edited])
    }

    func testRemove() {
        var list = MuteList(rules: [rule, MuteRule(target: .title, syntax: .wildcard, pattern: "x")])
        list.remove(id: rule.id)
        XCTAssertEqual(list.rules.map(\.pattern), ["x"])
        list.removeAll()
        XCTAssertTrue(list.rules.isEmpty)
    }

    func testFilterMutesWhenAnyRuleMatchesAndSkipsUnusableRules() {
        let list = MuteList(rules: [
            MuteRule(target: .title, syntax: .regex, pattern: "(unclosed"),
            MuteRule(target: .title, syntax: .wildcard, pattern: ""),
            rule,
            MuteRule(target: .title, syntax: .wildcard, pattern: "炎上"),
        ])
        let filter = MuteFilter(list)
        XCTAssertTrue(filter.isMuted(title: "普通の記事", url: URL(string: "https://www.example.com/")!))
        XCTAssertTrue(filter.isMuted(title: "炎上の記事", url: URL(string: "https://other.jp/")!))
        XCTAssertFalse(filter.isMuted(title: "普通の記事", url: URL(string: "https://other.jp/")!))
    }

    func testNoRulesMuteNothing() {
        XCTAssertFalse(MuteFilter(MuteList()).isMuted(title: "x", url: URL(string: "https://example.com/")!))
    }

    func testManyRulesStillMatch() {
        var list = MuteList()
        for index in 0..<1000 {
            list.save(MuteRule(target: .title, syntax: .wildcard, pattern: "keyword\(index)"))
        }
        let filter = MuteFilter(list)
        XCTAssertTrue(filter.isMuted(title: "about keyword999", url: URL(string: "https://example.com/")!))
        XCTAssertFalse(filter.isMuted(title: "about nothing", url: URL(string: "https://example.com/")!))
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
            MuteRule(target: .title, syntax: .regex, pattern: #"^\d+選"#),
        ])
        try file.save(list)
        XCTAssertEqual(try file.load(), list)
    }

    func testSetsAsideAFileItCannotRead() throws {
        let url = directory.appendingPathComponent("MuteRules.json")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data("not json".utf8).write(to: url)
        let file = MuteListFile(url: url)
        XCTAssertThrowsError(try file.load())

        let aside = try file.setAside()
        XCTAssertEqual(aside.lastPathComponent, "MuteRules.unreadable.json")
        XCTAssertEqual(try Data(contentsOf: aside), Data("not json".utf8))
        XCTAssertEqual(try file.load(), MuteList())
    }
}
