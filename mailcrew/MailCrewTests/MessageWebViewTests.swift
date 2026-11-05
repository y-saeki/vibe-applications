import AppKit
import Network
import XCTest

@testable import MailCrewMac

/// `MessageFrameView` in a real WKWebView, against a server on this Mac that
/// counts what reaches it. Gmail and the browser a link opens in stay out of
/// reach (DEVELOPMENT.md).
@MainActor
final class MessageWebViewTests: XCTestCase {
    private var server: CountingServer!
    private var window: NSWindow!

    override func setUp() async throws {
        server = try await CountingServer.start()
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 570, height: 600), styleMask: [.titled], backing: .buffered, defer: false)
    }

    override func tearDown() async throws {
        server.stop()
        window.close()
    }

    /// Shows `html` at the card's width and waits for its height.
    private func show(_ html: String, allowRemoteImages: Bool = false, width: CGFloat = 570) async throws -> (MessageFrameView, CGFloat) {
        let measured = expectation(description: "height")
        measured.assertForOverFulfill = false
        var height: CGFloat = 0
        let view = MessageFrameView(openLink: { _ in }) { value in
            height = value
            measured.fulfill()
        }
        view.frame = NSRect(x: 0, y: 0, width: width, height: 600)
        window.contentView = view
        view.layoutSubtreeIfNeeded()
        view.show(html: html, allowRemoteImages: allowRemoteImages)
        await fulfillment(of: [measured], timeout: 10)
        return (view, height)
    }

    /// Gives a blocked load time it would have needed to arrive.
    private func settle() async throws {
        try await Task.sleep(for: .milliseconds(800))
    }

    private var images: String {
        """
        <img src="\(server.url)/img.png" width="10" height="10">
        <div style="background: url('\(server.url)/bg.png'); height: 10px"></div>
        <table background="\(server.url)/table.png"><tr><td>x</td></tr></table>
        <link rel="stylesheet" href="\(server.url)/a.css">
        <iframe src="\(server.url)/frame.html"></iframe>
        <video src="\(server.url)/v.mp4"></video>
        """
    }

    func testNothingLoadsFromTheNetworkUntilAllowed() async throws {
        _ = try await show(images)
        try await settle()
        XCTAssertEqual(server.paths, [])
    }

    func testAllowedImagesLoadAndNothingElse() async throws {
        _ = try await show(images, allowRemoteImages: true)
        try await settle()
        XCTAssertTrue(server.paths.contains("/img.png"), "\(server.paths)")
        XCTAssertFalse(server.paths.contains("/a.css"), "\(server.paths)")
        XCTAssertFalse(server.paths.contains("/frame.html"), "\(server.paths)")
        XCTAssertFalse(server.paths.contains("/v.mp4"), "\(server.paths)")
    }

    func testAllowingImagesLaterLoadsThem() async throws {
        let (view, _) = try await show(images)
        let measured = expectation(description: "height again")
        measured.assertForOverFulfill = false
        view.onHeight = { _ in measured.fulfill() }
        view.show(html: images, allowRemoteImages: true)
        await fulfillment(of: [measured], timeout: 10)
        try await settle()
        XCTAssertTrue(server.paths.contains("/img.png"), "\(server.paths)")
    }

    func testTheMessageDoesNotRunScripts() async throws {
        _ = try await show("""
        <p>x</p><script>new Image().src = "\(server.url)/script.png"</script>
        <img src="x" onerror="new Image().src='\(server.url)/onerror.png'">
        """, allowRemoteImages: true)
        try await settle()
        XCTAssertEqual(server.paths, [])
    }

    func testTheViewIsAsTallAsTheMessage() async throws {
        let (_, short) = try await show("<p>一行</p>")
        let (_, long) = try await show(String(repeating: "<p>段落</p>", count: 40))
        XCTAssertGreaterThan(short, 20)
        XCTAssertLessThan(short, 120)
        XCTAssertGreaterThan(long, short * 10)
    }

    func testAFullHeightMessageIsNotAsTallAsTheView() async throws {
        let (_, height) = try await show("<style>html, body { height: 100% }</style><p>一行</p>")
        XCTAssertGreaterThan(height, 20)
        XCTAssertLessThan(height, 120)
    }

    func testAWideMessageIsScaledToTheWidth() async throws {
        let wide = "<table width=\"1140\" style=\"height: 1000px\"><tr><td>x</td></tr></table>"
        let (_, height) = try await show(wide)
        // Half the width, so about half the height.
        XCTAssertGreaterThan(height, 450)
        XCTAssertLessThan(height, 600)
    }
}

/// An HTTP server on 127.0.0.1 that answers every request with an empty 200
/// and remembers its path.
final class CountingServer: @unchecked Sendable {
    private let listener: NWListener
    private let lock = NSLock()
    private var seen: [String] = []
    let url: String

    var paths: [String] { lock.withLock { seen } }

    private init(listener: NWListener, port: UInt16) {
        self.listener = listener
        url = "http://127.0.0.1:\(port)"
    }

    static func start() async throws -> CountingServer {
        let parameters = NWParameters.tcp
        parameters.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        let listener = try NWListener(using: parameters)
        let ready = AsyncStream<UInt16?> { continuation in
            listener.stateUpdateHandler = { state in
                switch state {
                case .ready: continuation.yield(listener.port?.rawValue); continuation.finish()
                case .failed: continuation.yield(nil); continuation.finish()
                default: break
                }
            }
        }
        var server: CountingServer?
        listener.newConnectionHandler = { connection in server?.serve(connection) }
        listener.start(queue: .global())
        var port: UInt16?
        for await value in ready { port = value }
        guard let port else { throw URLError(.cannotConnectToHost) }
        server = CountingServer(listener: listener, port: port)
        return server!
    }

    private func serve(_ connection: NWConnection) {
        connection.start(queue: .global())
        connection.receive(minimumIncompleteLength: 1, maximumLength: 65536) { [self] data, _, _, _ in
            if let data, let line = String(decoding: data, as: UTF8.self).split(separator: "\r\n").first {
                let parts = line.split(separator: " ")
                if parts.count > 1 { lock.withLock { seen.append(String(parts[1])) } }
            }
            let reply = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            connection.send(content: Data(reply.utf8), completion: .contentProcessed { _ in connection.cancel() })
        }
    }

    func stop() { listener.cancel() }
}
