import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared

/// Answers every request with a canned response and remembers what was sent.
public final class StubTransport: HTTPTransport, @unchecked Sendable {
    private let lock = NSLock()
    private var sent: [URLRequest] = []
    private let respond: @Sendable (URLRequest) -> (status: Int, body: Data)

    public init(respond: @escaping @Sendable (URLRequest) -> (status: Int, body: Data)) {
        self.respond = respond
    }

    public convenience init(status: Int = 200, body: String) {
        let data = Data(body.utf8)
        self.init { _ in (status, data) }
    }

    public var requests: [URLRequest] {
        lock.lock()
        defer { lock.unlock() }
        return sent
    }

    public func send(_ request: URLRequest) async throws -> (Data, HTTPURLResponse) {
        record(request)
        let (status, body) = respond(request)
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: "HTTP/1.1", headerFields: nil)!
        return (body, response)
    }

    // Synchronous, since NSLock may not be taken in an async context.
    private func record(_ request: URLRequest) {
        lock.lock()
        defer { lock.unlock() }
        sent.append(request)
    }
}

public extension HatenaHTTPClient {
    static func stub(_ transport: StubTransport) -> HatenaHTTPClient {
        HatenaHTTPClient(userAgent: "TenabukuTests", transport: transport)
    }
}

public extension URLRequest {
    /// The form body as name/value pairs, in the order sent.
    var formBody: [(String, String)] {
        PercentEncoding.decodeForm(String(decoding: httpBody ?? Data(), as: UTF8.self))
    }
}
