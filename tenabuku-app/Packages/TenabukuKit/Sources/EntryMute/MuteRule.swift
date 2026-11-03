import Foundation

/// One condition that hides an entry from the lists. The pattern is kept as
/// typed; how it is read depends on `target` and `syntax`, as described on
/// each case.
public struct MuteRule: Codable, Hashable, Identifiable, Sendable {
    public enum Target: String, Codable, CaseIterable, Sendable {
        /// The entry's URL. A wildcard pattern without a `/` names a domain and
        /// covers its subdomains: `example.com` mutes `www.example.com` too.
        /// One with a `/` is matched from the start of the URL with its scheme
        /// left off, and anything may follow it: `example.com/blog/` mutes
        /// everything under that path. A regular expression is searched for
        /// in the whole URL, scheme included.
        case url
        /// The entry's title. A wildcard pattern is searched for anywhere in it.
        case title
    }

    public enum Syntax: String, Codable, CaseIterable, Sendable {
        /// `*` stands for any run of characters, `?` for any one character.
        /// Neither case nor full and half width count.
        case wildcard
        /// A regular expression, searched for anywhere and ignoring case.
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

    /// Why the rule cannot be used, or nil when it can: an empty pattern, or a
    /// regular expression that does not compile.
    public var problem: MuteRuleProblem? {
        do {
            _ = try MuteMatcher(self)
            return nil
        } catch let problem as MuteRuleProblem {
            return problem
        } catch {
            return .invalidRegex(String(describing: error))
        }
    }
}

public enum MuteRuleProblem: Error, Equatable, Sendable {
    case emptyPattern
    case invalidRegex(String)
}

/// A rule made ready to test entries with: the regular expression compiled,
/// the wildcard pattern normalised. Built once per change of the rules rather
/// than once per entry.
public struct MuteMatcher {
    private enum Kind {
        case host(Glob)
        case urlPrefix(Glob)
        case titleGlob(Glob)
        case regex(Regex<AnyRegexOutput>)
    }

    private let target: MuteRule.Target
    private let kind: Kind

    public init(_ rule: MuteRule) throws {
        let pattern = rule.pattern.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !pattern.isEmpty else { throw MuteRuleProblem.emptyPattern }
        target = rule.target
        switch (rule.syntax, rule.target) {
        case (.regex, _):
            do {
                kind = .regex(try Regex(pattern).ignoresCase())
            } catch {
                throw MuteRuleProblem.invalidRegex(String(describing: error))
            }
        case (.wildcard, .title):
            kind = .titleGlob(Glob(Self.fold(pattern)))
        case (.wildcard, .url):
            let address = Self.fold(Self.droppingScheme(pattern))
            kind = address.contains("/") ? .urlPrefix(Glob(address)) : .host(Glob(address))
        }
    }

    public func matches(title: String, url: URL) -> Bool {
        switch kind {
        case let .regex(regex):
            let text = target == .title ? title : url.absoluteString
            return (try? regex.firstMatch(in: text)) != nil
        case let .titleGlob(glob):
            return glob.occurs(in: Self.fold(title))
        case let .host(glob):
            guard let host = url.host() else { return false }
            return Self.domainSuffixes(of: Self.fold(host)).contains { glob.matches($0) }
        case let .urlPrefix(glob):
            return glob.isPrefix(of: Self.fold(Self.droppingScheme(url.absoluteString)))
        }
    }

    private static func fold(_ text: String) -> String {
        text.folding(options: [.caseInsensitive, .widthInsensitive], locale: nil)
    }

    /// `https://example.com/a` becomes `example.com/a`, so that a pasted URL
    /// and one typed without its scheme read the same.
    private static func droppingScheme(_ text: String) -> String {
        guard let range = text.range(of: "://") else { return text }
        return String(text[range.upperBound...])
    }

    /// `a.b.example.com`, `b.example.com`, `example.com` and `com`: a domain
    /// pattern matches the host or any domain above it.
    private static func domainSuffixes(of host: String) -> [String] {
        let labels = host.split(separator: ".", omittingEmptySubsequences: false)
        return labels.indices.map { labels[$0...].joined(separator: ".") }
    }
}

/// A pattern where `*` matches any run of characters and `?` any single one.
struct Glob {
    private let pattern: [Character]

    init(_ pattern: String) {
        self.pattern = Array(pattern)
    }

    /// The whole text matches.
    func matches(_ text: String) -> Bool {
        Self.match(pattern, Array(text))
    }

    /// The text starts with something the pattern matches.
    func isPrefix(of text: String) -> Bool {
        Self.match(pattern + ["*"], Array(text))
    }

    /// Something the pattern matches occurs anywhere in the text.
    func occurs(in text: String) -> Bool {
        Self.match(["*"] + pattern + ["*"], Array(text))
    }

    /// Greedy matching that returns to the last `*` on a mismatch, which keeps
    /// it linear in practice and never recurses.
    private static func match(_ pattern: [Character], _ text: [Character]) -> Bool {
        var p = 0, t = 0
        var star: Int?
        var resume = 0
        while t < text.count {
            if p < pattern.count, pattern[p] == "?" || pattern[p] == text[t] {
                p += 1
                t += 1
            } else if p < pattern.count, pattern[p] == "*" {
                star = p
                resume = t
                p += 1
            } else if let star {
                p = star + 1
                resume += 1
                t = resume
            } else {
                return false
            }
        }
        while p < pattern.count, pattern[p] == "*" {
            p += 1
        }
        return p == pattern.count
    }
}
