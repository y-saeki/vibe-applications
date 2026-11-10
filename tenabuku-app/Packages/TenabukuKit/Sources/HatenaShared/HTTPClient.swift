import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Sends one request. Abstracted so that tests can answer without a network.
public protocol HTTPTransport: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)

    /// Receives only the start of the body: up to and including `marker`
    /// (matched without regard to ASCII case), or `limit` bytes when the
    /// marker does not come first. The rest is never downloaded where the
    /// transport can stop early.
    func sendPrefix(_ request: URLRequest, endingAt marker: String, limit: Int) async throws -> (Data, HTTPURLResponse)
}

public extension HTTPTransport {
    /// Downloads the whole body and keeps its start, for transports that
    /// cannot stop early.
    func sendPrefix(_ request: URLRequest, endingAt marker: String, limit: Int) async throws -> (Data, HTTPURLResponse) {
        let (data, response) = try await send(request)
        var prefix = BodyPrefix(marker: marker, limit: limit)
        for byte in data where !prefix.append(byte) {}
        return (prefix.data, response)
    }
}

/// Collects the start of a body a byte at a time and says when to stop.
public struct BodyPrefix: Sendable {
    public private(set) var data = Data()
    private let marker: [UInt8]
    private let limit: Int
    private var done = false

    public init(marker: String, limit: Int) {
        self.marker = Array(marker.lowercased().utf8)
        self.limit = limit
    }

    /// Adds the byte unless the prefix is complete; false once it is.
    public mutating func append(_ byte: UInt8) -> Bool {
        guard !done else { return false }
        data.append(byte)
        done = data.count >= limit || endsWithMarker
        return !done
    }

    private var endsWithMarker: Bool {
        guard !marker.isEmpty, data.count >= marker.count else { return false }
        return zip(data.suffix(marker.count), marker).allSatisfy { byte, expected in
            Self.lowercased(byte) == expected
        }
    }

    private static func lowercased(_ byte: UInt8) -> UInt8 {
        (0x41...0x5A).contains(byte) ? byte + 0x20 : byte
    }
}

public struct URLSessionTransport: HTTPTransport {
    private let session: URLSession

    public init(session: URLSession = .shared) {
        self.session = session
    }

    // The completion-handler API rather than `data(for:)`, which older
    // swift-corelibs-foundation releases on Linux do not have.
    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        try await withCheckedThrowingContinuation { continuation in
            let task = session.dataTask(with: request) { data, response, error in
                if let error {
                    continuation.resume(throwing: error)
                } else if let response = response as? HTTPURLResponse {
                    continuation.resume(returning: (data ?? Data(), response))
                } else {
                    continuation.resume(throwing: HatenaError.transport("no HTTP response"))
                }
            }
            task.resume()
        }
    }

    public func sendPrefix(_ request: URLRequest, endingAt marker: String, limit: Int) async throws -> (Data, HTTPURLResponse) {
        #if canImport(Darwin)
        let (bytes, response) = try await session.bytes(for: request)
        // Whatever happens below, the rest of the body is not wanted.
        defer { bytes.task.cancel() }
        guard let response = response as? HTTPURLResponse else {
            throw HatenaError.transport("no HTTP response")
        }
        var prefix = BodyPrefix(marker: marker, limit: limit)
        for try await byte in bytes where !prefix.append(byte) {
            break
        }
        return (prefix.data, response)
        #else
        let (data, response) = try await send(request)
        var prefix = BodyPrefix(marker: marker, limit: limit)
        for byte in data where !prefix.append(byte) {}
        return (prefix.data, response)
        #endif
    }
}

/// The one way every module talks to Hatena: it adds the User-Agent and turns
/// transport errors and status codes into `HatenaError`.
public struct HatenaHTTPClient: Sendable {
    public let userAgent: String
    private let transport: any HTTPTransport

    public init(userAgent: String, transport: any HTTPTransport = URLSessionTransport()) {
        self.userAgent = userAgent
        self.transport = transport
    }

    /// Sends the request and returns whatever status came back, for callers
    /// that give some non-2xx status a meaning of their own (404 as "none").
    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        var request = request
        request.setValue(userAgent, forHTTPHeaderField: "User-Agent")
        do {
            return try await transport.send(request)
        } catch let error as HatenaError {
            throw error
        } catch {
            throw HatenaError.transport(error.localizedDescription)
        }
    }

    /// Like `send`, but receives only the start of the body (see
    /// `HTTPTransport.sendPrefix`).
    public func sendPrefix(_ request: URLRequest, endingAt marker: String, limit: Int) async throws -> (Data, HTTPURLResponse) {
        var request = request
        request.setValue(userAgent, forHTTPHeaderField: "User-Agent")
        do {
            return try await transport.sendPrefix(request, endingAt: marker, limit: limit)
        } catch let error as HatenaError {
            throw error
        } catch {
            throw HatenaError.transport(error.localizedDescription)
        }
    }

    /// Like `send`, but any status outside 2xx is an error.
    public func data(for request: URLRequest) async throws -> Data {
        let (data, response) = try await send(request)
        try Self.validate(response)
        return data
    }

    public func get(_ url: URL) async throws -> Data {
        try await data(for: URLRequest(url: url))
    }

    public static func validate(_ response: HTTPURLResponse) throws {
        switch response.statusCode {
        case 200..<300:
            return
        case 401:
            throw HatenaError.unauthorized
        default:
            throw HatenaError.httpStatus(response.statusCode)
        }
    }
}

public enum UserAgent {
    /// Hatena restricts clients that do not identify themselves, so every
    /// request names the app and where it comes from.
    public static func tenabuku(version: String) -> String {
        "Tenabuku/\(version) (+https://github.com/y-saeki/vibe-applications)"
    }
}
