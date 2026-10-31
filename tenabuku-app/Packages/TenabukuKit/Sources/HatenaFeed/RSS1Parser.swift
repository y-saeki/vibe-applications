import Foundation
#if canImport(FoundationXML)
import FoundationXML
#endif
import HatenaShared

/// Reads the RSS 1.0 (RDF) feeds Hatena Bookmark serves.
///
/// Namespace processing is left off, so elements are matched by the prefixes
/// Hatena writes (`dc:date`, `hatena:bookmarkcount`).
final class RSS1Parser: NSObject, XMLParserDelegate {
    private var items: [FeedItem] = []
    private var fields: [String: [String]]?
    private var about: String?
    private var text = ""
    private var sawRoot = false
    private var brokenItem: String?

    static func parse(_ data: Data, source: String) throws -> [FeedItem] {
        let delegate = RSS1Parser()
        let parser = XMLParser(data: data)
        parser.delegate = delegate
        guard parser.parse() else {
            let detail = parser.parserError.map { String(describing: $0) } ?? "XML parse error"
            throw HatenaError.parseFailed(source: source, detail: detail)
        }
        guard delegate.sawRoot else {
            throw HatenaError.parseFailed(source: source, detail: "no rdf:RDF root element")
        }
        if let brokenItem = delegate.brokenItem {
            throw HatenaError.parseFailed(source: source, detail: "item without title or link: \(brokenItem)")
        }
        return delegate.items
    }

    func parser(
        _ parser: XMLParser,
        didStartElement elementName: String,
        namespaceURI: String?,
        qualifiedName qName: String?,
        attributes attributeDict: [String: String] = [:]
    ) {
        if elementName == "rdf:RDF" {
            sawRoot = true
        } else if elementName == "item" {
            fields = [:]
            about = attributeDict["rdf:about"]
        }
        text = ""
    }

    func parser(_ parser: XMLParser, foundCharacters string: String) {
        text += string
    }

    func parser(_ parser: XMLParser, foundCDATA CDATABlock: Data) {
        text += String(decoding: CDATABlock, as: UTF8.self)
    }

    func parser(
        _ parser: XMLParser,
        didEndElement elementName: String,
        namespaceURI: String?,
        qualifiedName qName: String?
    ) {
        guard fields != nil else { return }
        if elementName == "item" {
            finishItem()
        } else {
            fields?[elementName, default: []].append(text.trimmingCharacters(in: .whitespacesAndNewlines))
        }
        text = ""
    }

    private func finishItem() {
        let current = fields ?? [:]
        fields = nil

        func first(_ name: String) -> String? {
            guard let value = current[name]?.first, !value.isEmpty else { return nil }
            return value
        }

        guard let title = first("title"),
              let link = (first("link") ?? about).flatMap(URL.init(string:))
        else {
            if brokenItem == nil {
                brokenItem = about ?? "(no rdf:about)"
            }
            return
        }
        items.append(FeedItem(
            title: title,
            link: link,
            summary: first("description") ?? "",
            date: first("dc:date").flatMap(HatenaDate.parseISO8601),
            subjects: current["dc:subject"] ?? [],
            bookmarkCount: first("hatena:bookmarkcount").flatMap { Int($0) } ?? 0,
            imageURL: first("hatena:imageurl").flatMap(URL.init(string:)),
            creator: first("dc:creator")
        ))
    }
}
