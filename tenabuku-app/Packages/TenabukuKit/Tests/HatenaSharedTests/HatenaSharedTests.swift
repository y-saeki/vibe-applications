import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
import HatenaShared
import HatenaTestSupport
import XCTest

final class PercentEncodingTests: XCTestCase {
    func testEncodesEverythingButUnreserved() {
        XCTAssertEqual(PercentEncoding.encode("日本 語+/~"), "%E6%97%A5%E6%9C%AC%20%E8%AA%9E%2B%2F~")
        XCTAssertEqual(PercentEncoding.encode("Az09-._~"), "Az09-._~")
    }

    func testDecodesFormWithPlusAsSpaceAndBareNames() {
        let pairs = PercentEncoding.decodeForm("a3=2+q&c2&b5=%3D%253D")
        XCTAssertEqual(pairs.map { $0.0 }, ["a3", "c2", "b5"])
        XCTAssertEqual(pairs.map { $0.1 }, ["2 q", "", "=%3D"])
    }
}

final class HatenaDateTests: XCTestCase {
    func testParsesISO8601() {
        XCTAssertEqual(HatenaDate.parseISO8601("2026-10-30T22:18:39Z")?.timeIntervalSince1970, 1_793_398_719)
        XCTAssertEqual(HatenaDate.parseISO8601("2026-10-31T07:18:39+09:00")?.timeIntervalSince1970, 1_793_398_719)
    }

    func testSlashedDatesAreJapanTime() {
        // 14:36 in Tokyo is 05:36 UTC.
        XCTAssertEqual(HatenaDate.parseSlashed("2026/10/31 14:36"), HatenaDate.parseISO8601("2026-10-31T05:36:00Z"))
    }

    func testDayIsJapanTime() {
        // 23:00 UTC on the 30th is already the 31st in Tokyo.
        XCTAssertEqual(HatenaDate.day(HatenaDate.parseISO8601("2026-10-30T23:00:00Z")!), "2026-10-31")
    }
}

final class LooseJSONTests: XCTestCase {
    private struct Flags: Decodable {
        let values: [LooseBool]
    }

    func testLooseBoolFollowsTheDocumentedRule() throws {
        let json = #"{"values": [null, 0, "0", 1, "1", true, false]}"#
        let flags = try JSONDecoder().decode(Flags.self, from: Data(json.utf8))
        XCTAssertEqual(flags.values.map(\.value), [false, false, false, true, true, true, false])
    }

    func testLooseIntAcceptsStrings() throws {
        let values = try JSONDecoder().decode([LooseInt].self, from: Data(#"[12, "34"]"#.utf8))
        XCTAssertEqual(values.map(\.value), [12, 34])
    }

    func testMismatchIsReportedAsBreakage() {
        XCTAssertThrowsError(try JSONDecoder.hatena([LooseInt].self, from: Data("{}".utf8), source: "test")) { error in
            XCTAssertTrue((error as? HatenaError)?.isBreakage == true)
        }
    }
}

final class HatenaHTTPClientTests: XCTestCase {
    func testSendsUserAgent() async throws {
        let transport = StubTransport(body: "ok")
        _ = try await HatenaHTTPClient.stub(transport).get(URL(string: "https://b.hatena.ne.jp/")!)
        XCTAssertEqual(transport.requests.first?.value(forHTTPHeaderField: "User-Agent"), "TenabukuTests")
    }

    func testMapsStatusCodes() async {
        for (status, expected) in [(401, HatenaError.unauthorized), (503, HatenaError.httpStatus(503))] {
            let client = HatenaHTTPClient.stub(StubTransport(status: status, body: ""))
            do {
                _ = try await client.get(URL(string: "https://b.hatena.ne.jp/")!)
                XCTFail("status \(status) should throw")
            } catch {
                XCTAssertEqual(error as? HatenaError, expected)
            }
        }
    }
}
