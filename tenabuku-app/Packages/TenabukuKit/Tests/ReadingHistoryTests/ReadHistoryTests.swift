import Foundation
import ReadingHistory
import XCTest

final class ReadHistoryTests: XCTestCase {
    private let article = URL(string: "https://example.com/article")!
    private let other = URL(string: "https://example.com/other")!
    private let now = Date(timeIntervalSince1970: 1_793_398_719)
    private let day: TimeInterval = 24 * 60 * 60

    func testRemembersOpenedEntries() {
        var history = ReadHistory()
        history.markRead(article, at: now)
        XCTAssertTrue(history.contains(article))
        XCTAssertFalse(history.contains(other))
        XCTAssertEqual(history.count, 1)
    }

    func testPruneDropsMarksOlderThanThirtyDays() {
        var history = ReadHistory()
        history.markRead(article, at: now.addingTimeInterval(-30 * day))
        history.markRead(other, at: now.addingTimeInterval(-30 * day - 1))
        history.prune(now: now)
        XCTAssertTrue(history.contains(article))
        XCTAssertFalse(history.contains(other))
    }

    func testOpeningAgainRestartsTheThirtyDays() {
        var history = ReadHistory()
        history.markRead(article, at: now.addingTimeInterval(-40 * day))
        history.markRead(article, at: now.addingTimeInterval(-1 * day))
        history.prune(now: now)
        XCTAssertTrue(history.contains(article))
    }

    func testRemoveAll() {
        var history = ReadHistory()
        history.markRead(article, at: now)
        history.removeAll()
        XCTAssertEqual(history.count, 0)
        XCTAssertFalse(history.contains(article))
    }
}

final class ReadHistoryFileTests: XCTestCase {
    private var directory: URL!

    override func setUpWithError() throws {
        directory = FileManager.default.temporaryDirectory
            .appendingPathComponent("ReadHistoryFileTests-\(UUID().uuidString)")
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: directory)
    }

    func testLoadsEmptyWhenNothingWasSaved() throws {
        let file = ReadHistoryFile(url: directory.appendingPathComponent("ReadHistory.json"))
        XCTAssertEqual(try file.load(), ReadHistory())
    }

    func testRoundTripsThroughAFileInANewDirectory() throws {
        let file = ReadHistoryFile(url: directory.appendingPathComponent("nested/ReadHistory.json"))
        var history = ReadHistory()
        history.markRead(URL(string: "https://example.com/article")!, at: Date(timeIntervalSince1970: 1_793_398_719))
        try file.save(history)
        XCTAssertEqual(try file.load(), history)
    }

    func testReportsAFileItCannotRead() throws {
        let url = directory.appendingPathComponent("ReadHistory.json")
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        try Data("not json".utf8).write(to: url)
        XCTAssertThrowsError(try ReadHistoryFile(url: url).load())
    }
}
