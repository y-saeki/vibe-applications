import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Sends one request. Abstracted so that tests can answer without a network.
public protocol HTTPTransport: Sendable {
    func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse)
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
