import Foundation

/// RFC 3986 percent-encoding, the variant OAuth 1.0a signs with. Query strings
/// and form bodies use it too, so that what is sent is exactly what was signed.
public enum PercentEncoding {
    private static let unreserved = CharacterSet(
        charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~"
    )

    public static func encode(_ string: String) -> String {
        string.addingPercentEncoding(withAllowedCharacters: unreserved) ?? string
    }

    public static func encodeForm(_ pairs: [(String, String)]) -> String {
        pairs.map { "\(encode($0.0))=\(encode($0.1))" }.joined(separator: "&")
    }

    /// Decodes `application/x-www-form-urlencoded`, where `+` is a space.
    /// A name without `=` gets an empty value.
    public static func decodeForm(_ string: String) -> [(String, String)] {
        func decode(_ part: Substring) -> String {
            let spaced = part.replacingOccurrences(of: "+", with: " ")
            return spaced.removingPercentEncoding ?? spaced
        }
        return string.split(separator: "&").map { pair in
            let parts = pair.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
            return (decode(parts[0]), parts.count > 1 ? decode(parts[1]) : "")
        }
    }
}
