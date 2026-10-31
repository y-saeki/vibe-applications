import Foundation

// Hatena's JSON is not strict about types: the REST API documents that a
// boolean may be null, 0 or "0", and numbers may arrive as strings. These
// wrappers accept every documented form so that a harmless variation is not
// reported as a breakage.

/// False for null, 0 and "0"; true for anything else.
public struct LooseBool: Decodable, Sendable {
    public let value: Bool

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            value = false
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int != 0
        } else {
            value = try container.decode(String.self) != "0"
        }
    }
}

/// A number written either as a number or as a string.
public struct LooseInt: Decodable, Sendable {
    public let value: Int

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let int = try? container.decode(Int.self) {
            value = int
            return
        }
        let string = try container.decode(String.self)
        guard let int = Int(string) else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "not a number: \(string)")
        }
        value = int
    }
}

/// An identifier written either as a number or as a string.
public struct LooseString: Decodable, Sendable {
    public let value: String

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let int = try? container.decode(Int.self) {
            value = String(int)
        } else {
            value = try container.decode(String.self)
        }
    }
}

public extension JSONDecoder {
    /// Decodes, reporting a mismatch as a breakage of `source`.
    static func hatena<T: Decodable>(_ type: T.Type, from data: Data, source: String) throws -> T {
        do {
            return try JSONDecoder().decode(type, from: data)
        } catch {
            throw HatenaError.parseFailed(source: source, detail: String(describing: error))
        }
    }
}
