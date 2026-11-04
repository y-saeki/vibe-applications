import Foundation

/// How a card writes its date: the time for today, the day for this year,
/// the full date for anything older. The same as the Tauri app's
/// `formatDate`.
enum CardDate {
    static func format(epochMs: Int64, now: Date = Date(), calendar: Calendar = .current) -> String {
        let date = Date(timeIntervalSince1970: TimeInterval(epochMs) / 1000)
        let parts = calendar.dateComponents([.year, .month, .day, .hour, .minute], from: date)
        let year = parts.year ?? 0
        let month = parts.month ?? 0
        let day = parts.day ?? 0
        if calendar.isDate(date, inSameDayAs: now) {
            return "\(parts.hour ?? 0):" + String(format: "%02d", parts.minute ?? 0)
        }
        if year == calendar.component(.year, from: now) {
            return "\(month)月\(day)日"
        }
        return "\(year)/\(month)/\(day)"
    }
}
