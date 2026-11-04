import Foundation
import XCTest

@testable import MailCrewCore

/// The bindings themselves: that a call crosses into Rust and comes back with
/// what the Rust tests already pin down. Nothing here reaches Google or the
/// Keychain; see DEVELOPMENT.md for what the tests cannot reach.
final class MailCrewCoreTests: XCTestCase {
    private var configDir: URL!

    override func setUpWithError() throws {
        configDir = FileManager.default.temporaryDirectory
            .appendingPathComponent("mailcrew-core-tests-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: configDir, withIntermediateDirectories: true)
    }

    override func tearDownWithError() throws {
        try? FileManager.default.removeItem(at: configDir)
    }

    func testOnlyWebAndMailLinksOpen() {
        XCTAssertTrue(isOpenableLink(url: "https://example.com/x"))
        XCTAssertTrue(isOpenableLink(url: "mailto:a@example.com"))
        XCTAssertFalse(isOpenableLink(url: "javascript:alert(1)"))
        XCTAssertFalse(isOpenableLink(url: "file:///etc/passwd"))
    }

    func testOpenedAttachmentsGoUnderTheTemporaryFolder() {
        XCTAssertEqual(openedAttachmentsDir(tempDir: "/tmp"), "/tmp/mailcrew-attachments")
    }

    func testViewsAreSavedAndReadBack() async throws {
        let core = MailCrew(configDir: configDir.path)
        let empty = try await core.listViews()
        XCTAssertEqual(empty, [])
        let view = View(id: "v1", name: "仕事", accounts: ["a@example.com"], query: "is:unread")
        try await core.saveViews(views: [view])
        let saved = try await core.listViews()
        XCTAssertEqual(saved, [view])
    }

    func testImageSendersAreKeptInLowerCaseOnce() async throws {
        let core = MailCrew(configDir: configDir.path)
        let saved = try await core.saveImageSenders(senders: ["A@Example.com", "a@example.com", " "])
        XCTAssertEqual(saved, ["a@example.com"])
        let read = try await core.listImageSenders()
        XCTAssertEqual(read, ["a@example.com"])
    }

    func testPreferencesDefaultToLookingUpLogos() async throws {
        let core = MailCrew(configDir: configDir.path)
        let defaults = try await core.preferences()
        XCTAssertTrue(defaults.brandLogos)
        try await core.savePreferences(preferences: Preferences(brandLogos: false))
        let saved = try await MailCrew(configDir: configDir.path).preferences()
        XCTAssertFalse(saved.brandLogos)
    }

    func testAnErrorComesBackAsMailCrewError() async {
        // A file where the config folder should be: writing under it fails.
        let file = configDir.appendingPathComponent("not-a-folder")
        FileManager.default.createFile(atPath: file.path, contents: Data())
        let core = MailCrew(configDir: file.path)
        do {
            try await core.saveViews(views: [])
            XCTFail("saving under a file succeeded")
        } catch MailCrewError.Internal(let message) {
            XCTAssertFalse(message.isEmpty)
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }
}
