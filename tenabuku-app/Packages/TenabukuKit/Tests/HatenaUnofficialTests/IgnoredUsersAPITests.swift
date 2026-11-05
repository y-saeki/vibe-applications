import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaUnofficial
import HatenaShared
import HatenaTestSupport
import XCTest

// The shape of b.hatena.ne.jp/api/my/ignore_users as served to a signed-in
// browser in 2026-11, with made-up names. Asked for the whole list, it has
// no cursor; asked for part of it, the cursor leads to the rest.
private let wholeList = #"{"users":["alice","bob","carol"]}"#
private let firstPart = #"{"users":["alice","bob"],"cursor":"MTU0ODQ3MTA4MywyNzY2ODE4Mzg5NzgxOTI2NQ=="}"#
private let lastPart = #"{"cursor":null,"users":["carol"]}"#

private let loginCookies = [
    HTTPCookie(properties: [.name: "rk", .value: "secret", .domain: ".hatena.ne.jp", .path: "/"])!,
]

final class IgnoredUsersAPITests: XCTestCase {
    func testReadsTheWholeListWithTheWebsitesCookies() async throws {
        let transport = StubTransport(body: wholeList)

        let users = try await IgnoredUsersAPI(http: .stub(transport), cookies: loginCookies).users()

        XCTAssertEqual(users, ["alice", "bob", "carol"])
        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(transport.requests.count, 1)
        XCTAssertEqual(request.url?.absoluteString, "https://b.hatena.ne.jp/api/my/ignore_users?limit=10000")
        XCTAssertEqual(request.value(forHTTPHeaderField: "Cookie"), "rk=secret")
        XCTAssertFalse(request.httpShouldHandleCookies)
    }

    func testFollowsTheCursor() async throws {
        let transport = StubTransport { request in
            let body = request.url!.query!.contains("cursor=") ? lastPart : firstPart
            return (200, Data(body.utf8))
        }

        let users = try await IgnoredUsersAPI(http: .stub(transport), cookies: loginCookies).users()

        XCTAssertEqual(users, ["alice", "bob", "carol"])
        XCTAssertEqual(transport.requests.count, 2)
        XCTAssertEqual(
            transport.requests[1].url?.absoluteString,
            "https://b.hatena.ne.jp/api/my/ignore_users?limit=10000&cursor=MTU0ODQ3MTA4MywyNzY2ODE4Mzg5NzgxOTI2NQ%3D%3D"
        )
    }

    func testStopsWhenTheCursorRepeats() async throws {
        let transport = StubTransport(body: firstPart)

        let users = try await IgnoredUsersAPI(http: .stub(transport), cookies: loginCookies).users()

        XCTAssertEqual(users, ["alice", "bob"])
        XCTAssertEqual(transport.requests.count, 2)
    }

    func testHidingNobodyIsAnEmptyList() async throws {
        let users = try await IgnoredUsersAPI(http: .stub(StubTransport(body: #"{"users":[]}"#)), cookies: loginCookies).users()

        XCTAssertEqual(users, [])
    }

    func testLoginRequiredIsUnauthorized() async {
        let transport = StubTransport(status: 403, body: "403 Forbidden login required")
        do {
            _ = try await IgnoredUsersAPI(http: .stub(transport), cookies: loginCookies).users()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual(error as? HatenaError, .unauthorized)
        }
    }

    func testNoCookiesIsUnauthorizedWithoutAsking() async {
        let transport = StubTransport(body: wholeList)
        do {
            _ = try await IgnoredUsersAPI(http: .stub(transport), cookies: []).users()
            XCTFail("should throw")
        } catch {
            XCTAssertEqual(error as? HatenaError, .unauthorized)
        }
        XCTAssertTrue(transport.requests.isEmpty)
    }

    func testChangedShapeIsABreakageOfThisSource() async {
        do {
            _ = try await IgnoredUsersAPI(http: .stub(StubTransport(body: #"{"users":[{"name":"alice"}]}"#)), cookies: loginCookies).users()
            XCTFail("should throw")
        } catch {
            guard case let .parseFailed(source, _)? = error as? HatenaError else {
                return XCTFail("unexpected \(error)")
            }
            XCTAssertEqual(source, "unofficial.ignore-users")
        }
    }
}
