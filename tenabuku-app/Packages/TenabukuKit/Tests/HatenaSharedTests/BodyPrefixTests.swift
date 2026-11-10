import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared
import HatenaTestSupport
import XCTest

final class BodyPrefixTests: XCTestCase {
    private func prefix(of body: String, marker: String = "</head>", limit: Int = 1024) -> String {
        var prefix = BodyPrefix(marker: marker, limit: limit)
        for byte in Data(body.utf8) where !prefix.append(byte) {}
        return String(decoding: prefix.data, as: UTF8.self)
    }

    func testStopsRightAfterTheMarker() {
        XCTAssertEqual(prefix(of: "<head><title>a</title></head><body>b</body>"), "<head><title>a</title></head>")
    }

    func testMatchesTheMarkerInAnyCase() {
        XCTAssertEqual(prefix(of: "<HEAD></HEAD><BODY>"), "<HEAD></HEAD>")
    }

    func testStopsAtTheLimitWithoutTheMarker() {
        XCTAssertEqual(prefix(of: String(repeating: "a", count: 100), limit: 10), String(repeating: "a", count: 10))
    }

    func testKeepsAShortBodyWhole() {
        XCTAssertEqual(prefix(of: "<p>short</p>"), "<p>short</p>")
    }

    func testTheClientSendsTheUserAgentForAPrefix() async throws {
        let transport = StubTransport(body: "<head></head><body></body>")

        let (data, _) = try await HatenaHTTPClient.stub(transport)
            .sendPrefix(URLRequest(url: URL(string: "https://example.com/")!), endingAt: "</head>", limit: 1024)

        XCTAssertEqual(String(decoding: data, as: UTF8.self), "<head></head>")
        XCTAssertEqual(transport.requests.first?.value(forHTTPHeaderField: "User-Agent"), "TenabukuTests")
    }
}
