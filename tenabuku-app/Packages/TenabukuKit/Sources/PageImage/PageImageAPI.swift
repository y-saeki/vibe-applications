import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// The image a page names for itself in its `og:image` (or `twitter:image`),
/// read from the page's own HTML.
///
/// For entries Hatena has no image for. Hatena leaves out pages whose
/// `og:image` is a WebP or AVIF file, or whose URL holds characters outside
/// ASCII, though the app can show all of them (seen in 2026-11).
public struct PageImageAPI: Sendable {
    private let http: HatenaHTTPClient

    public init(http: HatenaHTTPClient) {
        self.http = http
    }

    /// The page's image, or nil when the page names none or is not HTML.
    /// Throws when the page cannot be reached or answers with an error.
    public func imageURL(for page: URL) async throws -> URL? {
        guard Self.isWeb(page) else { return nil }
        var request = URLRequest(url: page)
        request.setValue("text/html,application/xhtml+xml", forHTTPHeaderField: "Accept")
        let (data, response) = try await http.send(request)
        try HatenaHTTPClient.validate(response)
        if let type = response.value(forHTTPHeaderField: "Content-Type")?.lowercased(),
           !type.contains("html") {
            return nil
        }
        // The meta tags are ASCII even on a page in another encoding, so
        // decoding as UTF-8 keeps them readable whatever the page uses.
        let html = String(decoding: data, as: UTF8.self)
        // A page that moved names its image relative to where it ended up.
        return OpenGraphImage.find(in: html, page: response.url ?? page)
    }

    static func isWeb(_ url: URL) -> Bool {
        ["http", "https"].contains(url.scheme?.lowercased() ?? "")
    }
}

/// Picks the page's image out of its HTML.
public enum OpenGraphImage {
    /// The names that carry the image, most preferred first.
    static let names = ["og:image", "og:image:url", "og:image:secure_url", "twitter:image", "twitter:image:src"]

    public static func find(in html: String, page: URL) -> URL? {
        // Only the head: a page may quote another page's tags in its body.
        let head = html.range(of: "</head", options: .caseInsensitive).map { String(html[..<$0.lowerBound]) } ?? html
        var found: [String: String] = [:]
        for tag in matches(of: metaTag, in: head) {
            let attributes = self.attributes(of: tag)
            guard let name = (attributes["property"] ?? attributes["name"])?.lowercased(),
                  names.contains(name), found[name] == nil,
                  let content = attributes["content"]
            else { continue }
            found[name] = content
        }
        for name in names {
            if let content = found[name], let url = resolve(content, page: page) {
                return url
            }
        }
        return nil
    }

    private static let metaTag = try! NSRegularExpression(pattern: #"<meta\b[^>]*>"#, options: [.caseInsensitive])
    private static let attribute = try! NSRegularExpression(
        pattern: #"([A-Za-z_:][-A-Za-z0-9_:.]*)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+))"#
    )

    private static func matches(of regex: NSRegularExpression, in text: String) -> [String] {
        let range = NSRange(text.startIndex..., in: text)
        return regex.matches(in: text, range: range).compactMap { Range($0.range, in: text).map { String(text[$0]) } }
    }

    private static func attributes(of tag: String) -> [String: String] {
        var result: [String: String] = [:]
        let range = NSRange(tag.startIndex..., in: tag)
        for match in attribute.matches(in: tag, range: range) {
            guard let nameRange = Range(match.range(at: 1), in: tag) else { continue }
            let value = (2...4).lazy
                .compactMap { Range(match.range(at: $0), in: tag) }
                .first.map { String(tag[$0]) } ?? ""
            let name = tag[nameRange].lowercased()
            if result[name] == nil {
                result[name] = decodeEntities(value)
            }
        }
        return result
    }

    /// Makes the URL absolute and percent-encodes what a URL cannot hold as
    /// it is, such as a Japanese file name. Nil for anything but the web.
    static func resolve(_ content: String, page: URL) -> URL? {
        let trimmed = content.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              let url = URL(string: encodeUnsafe(trimmed), relativeTo: page)?.absoluteURL,
              PageImageAPI.isWeb(url), url.host != nil
        else { return nil }
        return url
    }

    private static func encodeUnsafe(_ text: String) -> String {
        var result = ""
        for scalar in text.unicodeScalars {
            if scalar.value > 0x20, scalar.value < 0x7F, !#""<>\^`{|}"#.unicodeScalars.contains(scalar) {
                result.unicodeScalars.append(scalar)
            } else {
                for byte in String(scalar).utf8 {
                    result += String(format: "%%%02X", byte)
                }
            }
        }
        return result
    }

    private static let namedEntities = ["amp": "&", "lt": "<", "gt": ">", "quot": "\"", "apos": "'", "#39": "'"]

    /// Decodes the character references an attribute value may hold:
    /// `&amp;`, `&#38;` and `&#x26;` all stand for `&`.
    static func decodeEntities(_ text: String) -> String {
        guard text.contains("&") else { return text }
        var result = ""
        var rest = Substring(text)
        while let amp = rest.firstIndex(of: "&") {
            result += rest[..<amp]
            let after = rest[rest.index(after: amp)...]
            if let semicolon = after.prefix(10).firstIndex(of: ";"),
               let decoded = decodeEntity(String(after[..<semicolon])) {
                result += decoded
                rest = after[after.index(after: semicolon)...]
            } else {
                result += "&"
                rest = after
            }
        }
        return result + rest
    }

    private static func decodeEntity(_ name: String) -> String? {
        if let named = namedEntities[name.lowercased()] {
            return named
        }
        guard name.hasPrefix("#") else { return nil }
        let digits = name.dropFirst()
        let value = digits.first.map { $0 == "x" || $0 == "X" } == true
            ? UInt32(digits.dropFirst(), radix: 16)
            : UInt32(digits)
        return value.flatMap(Unicode.Scalar.init).map { String(Character($0)) }
    }
}
