import Foundation

/// One condition under which an entry is hidden from the lists: a pattern
/// matched against either the entry's URL or its title.
///
/// Wildcard patterns know only `*`, which stands for any run of characters;
/// `?` is left literal because it is part of so many URLs. Regular expressions
/// use ICU's syntax, as `NSRegularExpression` reads it. Letter case is ignored
/// either way, and wildcard titles also ignore full and half width.
///
/// - A URL wildcard without `/` names a domain and covers its subdomains:
///   `example.com` hides `example.com` and `www.example.com`. With `/`, it is
///   matched against the start of the URL past the scheme: `example.com/blog/`
///   hides everything under that path. A leading `http://` or `https://` in the
///   pattern is ignored.
/// - A title wildcard is a keyword found anywhere in the title.
/// - A regular expression is searched for anywhere in the full URL or the title.
public struct MuteRule: Codable, Hashable, Identifiable, Sendable {
    public enum Target: String, Codable, CaseIterable, Sendable {
        case url
        case title
    }

    public enum Syntax: String, Codable, CaseIterable, Sendable {
        case wildcard
        case regex
    }

    public var id: UUID
    public var target: Target
    public var syntax: Syntax
    public var pattern: String

    public init(id: UUID = UUID(), target: Target, syntax: Syntax, pattern: String) {
        self.id = id
        self.target = target
        self.syntax = syntax
        self.pattern = pattern
    }

    /// Throws `MuteRuleError` when the rule could never match anything.
    public func validate() throws {
        _ = try MuteMatcher(rule: self)
    }
}

public enum MuteRuleError: Error, Equatable, Sendable {
    case emptyPattern
    case invalidRegex
}

/// The rules compiled once, to be asked of every entry in a list. A rule that
/// does not compile is left out rather than stopping the others.
public struct MuteFilter {
    private let matchers: [MuteMatcher]

    public init(rules: [MuteRule]) {
        matchers = rules.compactMap { try? MuteMatcher(rule: $0) }
    }

    /// Whether any rule matches the entry.
    public func isMuted(title: String, url: URL) -> Bool {
        matchers.contains { $0.matches(title: title, url: url) }
    }
}

struct MuteMatcher {
    /// What the compiled expression is run against.
    private enum Subject {
        case foldedTitle
        case title
        case host
        case urlPastScheme
        case url
    }

    private let subject: Subject
    private let expression: NSRegularExpression

    init(rule: MuteRule) throws {
        let pattern = rule.pattern.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !pattern.isEmpty else { throw MuteRuleError.emptyPattern }

        let source: String
        switch (rule.target, rule.syntax) {
        case (.title, .wildcard):
            subject = .foldedTitle
            source = Self.wildcardBody(Self.fold(pattern))
        case (.url, .wildcard):
            let rest = Self.droppingScheme(pattern)
            guard !rest.isEmpty else { throw MuteRuleError.emptyPattern }
            if rest.contains("/") {
                subject = .urlPastScheme
                source = "^" + Self.wildcardBody(rest)
            } else {
                subject = .host
                source = "^(?:.*\\.)?" + Self.wildcardBody(rest) + "$"
            }
        case (.title, .regex):
            subject = .title
            source = pattern
        case (.url, .regex):
            subject = .url
            source = pattern
        }

        do {
            expression = try NSRegularExpression(pattern: source, options: [.caseInsensitive])
        } catch {
            throw MuteRuleError.invalidRegex
        }
    }

    func matches(title: String, url: URL) -> Bool {
        let text: String
        switch subject {
        case .foldedTitle: text = Self.fold(title)
        case .title: text = title
        case .host:
            guard let host = url.host else { return false }
            text = host
        case .urlPastScheme: text = Self.droppingScheme(url.absoluteString)
        case .url: text = url.absoluteString
        }
        let range = NSRange(text.startIndex..., in: text)
        return expression.firstMatch(in: text, options: [], range: range) != nil
    }

    /// `*` becomes any run of characters; everything else stands for itself.
    private static func wildcardBody(_ pattern: String) -> String {
        pattern
            .split(separator: "*", omittingEmptySubsequences: false)
            .map { NSRegularExpression.escapedPattern(for: String($0)) }
            .joined(separator: ".*")
    }

    private static func droppingScheme(_ text: String) -> String {
        for scheme in ["https://", "http://"] where text.lowercased().hasPrefix(scheme) {
            return String(text.dropFirst(scheme.count))
        }
        return text
    }

    private static func fold(_ text: String) -> String {
        text.folding(options: [.caseInsensitive, .widthInsensitive], locale: nil)
    }
}
