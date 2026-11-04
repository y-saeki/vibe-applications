import XCTest

@testable import MailCrewMac

/// The same cases as the Tauri app's `formatDate`.
final class CardDateTests: XCTestCase {
    private var calendar: Calendar = {
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = TimeZone(identifier: "Asia/Tokyo")!
        return calendar
    }()

    private func epochMs(_ year: Int, _ month: Int, _ day: Int, _ hour: Int = 0, _ minute: Int = 0) -> Int64 {
        let date = calendar.date(from: DateComponents(year: year, month: month, day: day, hour: hour, minute: minute))!
        return Int64(date.timeIntervalSince1970 * 1000)
    }

    private var now: Date {
        Date(timeIntervalSince1970: TimeInterval(epochMs(2026, 11, 5, 15, 30)) / 1000)
    }

    func testTodayIsTheTime() {
        XCTAssertEqual(CardDate.format(epochMs: epochMs(2026, 11, 5, 9, 5), now: now, calendar: calendar), "9:05")
    }

    func testThisYearIsTheDay() {
        XCTAssertEqual(CardDate.format(epochMs: epochMs(2026, 11, 4, 23, 59), now: now, calendar: calendar), "11月4日")
        XCTAssertEqual(CardDate.format(epochMs: epochMs(2026, 1, 1), now: now, calendar: calendar), "1月1日")
    }

    func testOlderIsTheFullDate() {
        XCTAssertEqual(CardDate.format(epochMs: epochMs(2025, 12, 31, 23), now: now, calendar: calendar), "2025/12/31")
    }
}
