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

    private static func formatter(_ format: String) -> DateFormatter {
        let formatter = DateFormatter()
        formatter.locale = Locale(identifier: "en_US_POSIX")
        formatter.timeZone = tokyo
        formatter.dateFormat = format
        return formatter
    }
}
