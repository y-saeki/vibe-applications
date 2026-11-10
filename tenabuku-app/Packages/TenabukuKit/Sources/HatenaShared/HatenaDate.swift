import Foundation

/// The date formats Hatena's responses use.
public enum HatenaDate {
    public static let tokyo = TimeZone(identifier: "Asia/Tokyo")!

    /// `2026-10-30T22:18:39Z` (feeds, the REST API, the website's JSON).
    public static func parseISO8601(_ string: String) -> Date? {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.date(from: string)
    }

    /// `2026/10/31 14:36` in Japan time (the entry info JSON).
    public static func parseSlashed(_ string: String) -> Date? {
        formatter("yyyy/MM/dd HH:mm").date(from: string)
    }

    /// `2026-10-31` in Japan time, as the search page takes it.
    public static func day(_ date: Date) -> String {
        formatter("yyyy-MM-dd").string(from: date)
    }

    /// `20261031` in Japan time, as a bookmark's permalink names its day.
    public static func compactDay(_ date: Date) -> String {
        formatter("yyyyMMdd").string(from: date)
    }

    /// `2026-01-02 01:02` on a 24-hour clock, as the app shows every
    /// timestamp: the same width whatever the date, the locale or the
    /// system's 12-hour setting, so that rows line up.
    public static func timestamp(_ date: Date, in timeZone: TimeZone = .current) -> String {
        formatter("yyyy-MM-dd HH:mm", in: timeZone).string(from: date)
    }

    private static func formatter(_ format: String, in timeZone: TimeZone = tokyo) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.calendar = Calendar(identifier: .gregorian)
        formatter.timeZone = timeZone
        formatter.dateFormat = format
        return formatter
    }
}
