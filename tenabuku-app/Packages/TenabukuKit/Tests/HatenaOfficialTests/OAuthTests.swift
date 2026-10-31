import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import HatenaOfficial
import HatenaShared
import HatenaTestSupport
import XCTest

final class SHA1Tests: XCTestCase {
    private func hex(_ bytes: [UInt8]) -> String {
        bytes.map { String(format: "%02x", $0) }.joined()
    }

    func testKnownDigests() {
        XCTAssertEqual(hex(SHA1.hash([])), "da39a3ee5e6b4b0d3255bfef95601890afd80709")
        XCTAssertEqual(hex(SHA1.hash(Array("abc".utf8))), "a9993e364706816aba3e25717850c26c9cd0d89d")
        // 56 bytes: the padding spills into a second block.
        XCTAssertEqual(
            hex(SHA1.hash(Array("abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".utf8))),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        )
        XCTAssertEqual(hex(SHA1.hash([UInt8](repeating: 0x61, count: 1000))), "291e9a6c66994949b57ba5e650361e98fc36b1ba")
    }

    // RFC 2202 test cases 1, 2 and 6.
    func testHMACVectors() {
        XCTAssertEqual(
            hex(HMACSHA1.authenticate(Array("Hi There".utf8), key: [UInt8](repeating: 0x0B, count: 20))),
            "b617318655057264e28bc0b6fb378c8ef146be00"
        )
        XCTAssertEqual(
            hex(HMACSHA1.authenticate(Array("what do ya want for nothing?".utf8), key: Array("Jefe".utf8))),
            "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
        )
        XCTAssertEqual(
            hex(HMACSHA1.authenticate(
                Array("Test Using Larger Than Block-Size Key - Hash Key First".utf8),
                key: [UInt8](repeating: 0xAA, count: 80)
            )),
            "aa4ae5e15272d00e95705637ce8a3b55ed402112"
        )
    }
}

final class OAuthSignerTests: XCTestCase {
    // The example of RFC 5849 section 3.4.1.1.
    func testBaseStringMatchesRFC5849() {
        let url = URL(string: "http://EXAMPLE.com:80/request?b5=%3D%253D&a3=a&c%40=&a2=r%20b")!
        let parameters = [
            ("oauth_consumer_key", "9djdj82h48djs9d2"),
            ("oauth_token", "kkk9d7dh3k39sjv7"),
            ("oauth_signature_method", "HMAC-SHA1"),
            ("oauth_timestamp", "137131201"),
            ("oauth_nonce", "7d8f3e4a"),
            ("c2", ""),
            ("a3", "2 q"),
        ]
        let base = OAuthSigner.signatureBaseString(method: "post", url: url, parameters: parameters)
        XCTAssertEqual(
            base,
            "POST&http%3A%2F%2Fexample.com%2Frequest&a2%3Dr%2520b%26a3%3D2%2520q%26a3%3Da%26b5%3D%253D%25253D%26c%2540%3D%26c2%3D%26oauth_consumer_key%3D9djdj82h48djs9d2%26oauth_nonce%3D7d8f3e4a%26oauth_signature_method%3DHMAC-SHA1%26oauth_timestamp%3D137131201%26oauth_token%3Dkkk9d7dh3k39sjv7"
        )
        // Computed independently with Python's hmac module.
        XCTAssertEqual(
            OAuthSigner.signature(baseString: base, consumerSecret: "j49sk3j29djd", tokenSecret: "dh893hdasih9"),
            "r6/TJjbCOr97/+UU0NsvSne7s5g="
        )
    }

    func testRequestTokenRequestIsSignedOverCallbackAndScope() async throws {
        let transport = StubTransport(body: "oauth_token=rt%3D%3D&oauth_token_secret=rs&oauth_callback_confirmed=true")
        let flow = OAuthFlow(http: .stub(transport), signer: fixedSigner())

        let token = try await flow.requestToken(callback: "tenabuku://oauth-callback", scopes: [.readPublic, .writePublic])

        XCTAssertEqual(token, OAuthToken(token: "rt==", secret: "rs"))
        let request = try XCTUnwrap(transport.requests.first)
        XCTAssertEqual(request.httpMethod, "POST")
        XCTAssertEqual(request.url, OAuthFlow.initiateURL)
        XCTAssertEqual(request.formBody.map { $0.1 }, ["read_public,write_public"])
        let header = try XCTUnwrap(request.value(forHTTPHeaderField: "Authorization"))
        XCTAssertTrue(header.hasPrefix("OAuth realm=\"\", "))
        XCTAssertTrue(header.contains("oauth_callback=\"tenabuku%3A%2F%2Foauth-callback\""))
        // Computed independently with Python's hmac module.
        XCTAssertTrue(header.contains("oauth_signature=\"y34iZge%2ByEKe5nW7cNeMrpRAAWQ%3D\""), header)
    }

    func testAuthorizationURLCarriesTheRequestToken() {
        let flow = OAuthFlow(http: .stub(StubTransport(body: "")), signer: fixedSigner())
        XCTAssertEqual(
            flow.authorizationURL(for: OAuthToken(token: "a/b=", secret: "s")).absoluteString,
            "https://www.hatena.ne.jp/touch/oauth/authorize?oauth_token=a%2Fb%3D"
        )
    }

    func testAccessTokenReadsTheUser() async throws {
        let transport = StubTransport(body: "oauth_token=at&oauth_token_secret=as&url_name=sample&display_name=Sample")
        let flow = OAuthFlow(http: .stub(transport), signer: fixedSigner())

        let grant = try await flow.accessToken(for: OAuthToken(token: "rt", secret: "rs"), verifier: "v")

        XCTAssertEqual(grant, AccessGrant(token: OAuthToken(token: "at", secret: "as"), urlName: "sample", displayName: "Sample"))
        let header = try XCTUnwrap(transport.requests.first?.value(forHTTPHeaderField: "Authorization"))
        XCTAssertTrue(header.contains("oauth_token=\"rt\""))
        XCTAssertTrue(header.contains("oauth_verifier=\"v\""))
    }

    func testMissingTokenIsABreakage() async {
        let flow = OAuthFlow(http: .stub(StubTransport(body: "<html>maintenance</html>")), signer: fixedSigner())
        do {
            _ = try await flow.requestToken(callback: "oob", scopes: [.readPublic])
            XCTFail("should throw")
        } catch {
            XCTAssertTrue((error as? HatenaError)?.isBreakage == true)
        }
    }
}

func fixedSigner() -> OAuthSigner {
    OAuthSigner(
        consumer: OAuthConsumer(key: "ck", secret: "cs"),
        nonce: { "n0nce" },
        now: { Date(timeIntervalSince1970: 1_700_000_000) }
    )
}
