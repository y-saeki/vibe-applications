import MailCrewCore
import XCTest

@testable import MailCrewMac

/// The checks under a card's subject, when BIMI is looked up, which logo is
/// shown, and what a card's attachments ask of the Rust side. How WebKit
/// draws the logo, DNS and the mark certificates, and the app a file opens in
/// are left to the manual pass (DEVELOPMENT.md).
@MainActor
final class TrustAndAttachmentsTests: XCTestCase {
    private var core: FakeCore!
    private var openedFiles: [URL] = []
    private var opens = true

    private let downloads = URL(fileURLWithPath: "/Users/me/Downloads", isDirectory: true)
    private let openedDir = URL(fileURLWithPath: "/tmp/mailcrew-attachments", isDirectory: true)

    override func setUp() {
        core = FakeCore()
        core.accounts = ["a@example.com", "b@example.com"]
        openedFiles = []
        opens = true
    }

    private func model(cards: [Card]) async -> AppModel {
        core.inboxAnswers = [.success(InboxPage(cards: cards, failures: []))]
        let files = AppModel.Files(downloads: downloads, opened: openedDir) { [unowned self] url in
            openedFiles.append(url)
            return opens
        }
        let model = AppModel(core: core, openURL: { _ in }, files: files)
        await model.start()
        return model
    }

    private func auth(spf: String? = "pass", dkim: String? = "pass", dmarc: String? = "pass") -> Authentication {
        Authentication(
            spf: spf.map { Check(result: $0, domain: "example.com") },
            dkim: dkim.map { Check(result: $0, domain: "example.com") },
            dmarc: dmarc.map { Check(result: $0, domain: "example.com") }
        )
    }

    private func brand(
        bimi: BimiResult = .pass, mark: MarkResult = .vmc, logo: String? = "data:image/svg+xml;base64,PHN2Zy8+"
    ) -> Brand {
        Brand(bimi: bimi, domain: "example.com", mark: mark, logo: logo)
    }

    private func file(_ id: String, openable: Bool = true) -> Attachment {
        Attachment(id: id, name: "\(id).pdf", mimeType: "application/pdf", size: 2048, openable: openable)
    }

    /// A card whose body, with these checks and attachments, has come in.
    private func fetched(
        _ card: Card, authentication: Authentication? = nil, attachments: [Attachment] = []
    ) async -> AppModel {
        core.messages[card.messageId] = .success(
            content(.text(text: "本文"), attachments: attachments, authentication: authentication ?? auth())
        )
        let model = await model(cards: [card])
        await model.fetchBody(card)
        await model.brandLookup(for: card)?.value
        return model
    }

    // MARK: Checks

    func testNoChecksShowBeforeTheBodyComesIn() async {
        let model = await model(cards: [card("t1")])
        XCTAssertEqual(model.trustItems(of: model.cards[0]), [])
    }

    func testGmailsChecksReadAsTheTauriAppsDo() {
        let authentication = Authentication(
            spf: Check(result: "softfail", domain: "mail.example.com"),
            dkim: Check(result: "pass", domain: ""),
            dmarc: nil
        )
        let items = AppModel.trustItems(authentication: authentication, brand: nil)
        XCTAssertEqual(items, [
            TrustItem(key: "spf", label: "SPF", tone: .fail, detail: "SPF: 不合格（ソフトフェイル）（mail.example.com）"),
            TrustItem(key: "dkim", label: "DKIM", tone: .pass, detail: "DKIM: 合格"),
            TrustItem(key: "dmarc", label: "DMARC", tone: .neutral, detail: "DMARC: Gmail の検証結果がありません"),
        ])
    }

    func testAResultWithoutWordsShowsAsGmailWroteIt() {
        let items = AppModel.trustItems(authentication: auth(spf: "bestguesspass"), brand: nil)
        XCTAssertEqual(items[0].detail, "SPF: bestguesspass（example.com）")
        XCTAssertEqual(items[0].tone, .neutral)
    }

    func testTheBrandChecksFollowGmailsInEachState() {
        let pending = AppModel.trustItems(authentication: auth(), brand: .pending)
        XCTAssertEqual(pending.suffix(2).map(\.tone), [.pending, .pending])
        XCTAssertEqual(pending.suffix(2).map(\.detail), ["BIMI: 確認しています", "VMC: 確認しています"])

        let failed = AppModel.trustItems(authentication: auth(), brand: .failed)
        XCTAssertEqual(failed.suffix(2).map(\.detail), ["BIMI: 確認できませんでした", "VMC: 確認できませんでした"])

        let cmc = AppModel.trustItems(authentication: auth(), brand: .checked(brand(mark: .cmc)))
        XCTAssertEqual(cmc.suffix(2), [
            TrustItem(key: "bimi", label: "BIMI", tone: .pass, detail: "BIMI: 登録あり（example.com）"),
            TrustItem(key: "mark", label: "CMC", tone: .pass, detail: "CMC: 認証局が使用実績を確認したロゴです"),
        ])

        let broken = AppModel.trustItems(authentication: auth(), brand: .checked(brand(bimi: .fail, mark: .fail)))
        XCTAssertEqual(broken.suffix(2).map(\.tone), [.fail, .fail])
    }

    func testDMARCPassAsksTheCardsAccountForTheBrand() async {
        let theirs = card("t1", account: "b@example.com")
        core.brands = ["m-t1": .success(brand())]
        let model = await fetched(theirs)
        XCTAssertEqual(core.brandCalls.map(\.account), ["b@example.com"])
        XCTAssertEqual(core.brandCalls.map(\.messageId), ["m-t1"])
        XCTAssertEqual(model.brands[theirs.bodyKey], .checked(brand()))
        XCTAssertEqual(model.trustItems(of: theirs).map(\.key), ["spf", "dkim", "dmarc", "bimi", "mark"])
    }

    func testWithoutDMARCPassNothingIsLookedUp() async {
        for dmarc in ["fail", "none", nil] as [String?] {
            core = FakeCore()
            core.accounts = ["a@example.com"]
            let model = await fetched(card("t1"), authentication: auth(dmarc: dmarc))
            XCTAssertTrue(core.brandCalls.isEmpty, "dmarc=\(dmarc ?? "nil")")
            let bimi = model.trustItems(of: model.cards[0]).first { $0.key == "bimi" }
            XCTAssertEqual(bimi?.detail, "BIMI: DMARC の条件を満たさないため確認していません")
        }
    }

    func testWithBIMIOffOnlyGmailsChecksShowAndNothingIsAsked() async {
        core.brandLogos = false
        let model = await fetched(card("t1"))
        XCTAssertTrue(core.brandCalls.isEmpty)
        XCTAssertEqual(model.trustItems(of: model.cards[0]).map(\.key), ["spf", "dkim", "dmarc"])
    }

    func testBIMITurnedOffSinceTheSettingsWereReadDropsTheBrandChecks() async {
        core.brands = ["m-t1": .success(nil)]
        let model = await fetched(card("t1"))
        XCTAssertEqual(model.trustItems(of: model.cards[0]).map(\.key), ["spf", "dkim", "dmarc"])
        XCTAssertFalse(model.looksUpBrands)
    }

    func testAFailedLookupSaysSoAndRaisesNoNotice() async {
        let model = await fetched(card("t1"))
        XCTAssertEqual(model.brands[model.cards[0].bodyKey], .failed)
        XCTAssertNil(model.notice)
        XCTAssertNil(model.logo(of: model.cards[0]))
    }

    func testUnreadablePreferencesLeaveBIMIOn() async {
        core.preferencesError = .Internal(message: "読めません")
        core.brands = ["m-t1": .success(brand())]
        _ = await fetched(card("t1"))
        XCTAssertEqual(core.brandCalls.count, 1)
    }

    // MARK: Logo

    func testTheLogoShowsFromAPassingCertificate() async {
        core.brands = ["m-t1": .success(brand())]
        let model = await fetched(card("t1"))
        XCTAssertEqual(model.logo(of: model.cards[0]), BrandLogo(
            url: "data:image/svg+xml;base64,PHN2Zy8+",
            title: "example.com のロゴ（VMC で確認済み）"
        ))
    }

    func testOnlyADataSVGURLIsEverShownAsALogo() {
        XCTAssertNil(AppModel.logo(of: brand(logo: nil)))
        XCTAssertNil(AppModel.logo(of: brand(logo: "https://example.com/logo.svg")))
        XCTAssertNil(AppModel.logo(of: brand(logo: "data:text/html,<script></script>")))
        XCTAssertEqual(AppModel.logo(of: brand(mark: .cmc))?.title, "example.com のロゴ（CMC で確認済み）")
    }

    func testTheLogoDocumentLoadsNothingButTheLogo() {
        let document = LogoFrame.document(logo: "data:image/svg+xml,<svg a=\"b\">&amp;")
        XCTAssertTrue(document.contains("content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\""))
        XCTAssertTrue(document.contains("src=\"data:image/svg+xml,&lt;svg a=&quot;b&quot;>&amp;amp;\""))
        XCTAssertFalse(document.contains("<script"))
        XCTAssertTrue(LogoFrame.allows(URL(string: "about:blank")))
        XCTAssertFalse(LogoFrame.allows(URL(string: "https://example.com/")))
        XCTAssertFalse(LogoFrame.allows(nil))
    }

    func testTheBrandOfAMessageOffTheStripIsForgotten() async {
        core.brands = ["m-t1": .success(brand())]
        let model = await fetched(card("t1"))
        core.inboxAnswers = [.success(InboxPage(cards: [card("t2")], failures: []))]
        await model.load()
        XCTAssertTrue(model.brands.isEmpty)
    }

    // MARK: Attachments

    func testAttachmentsAreKnownOnceTheBodyIsHere() async {
        let model = await model(cards: [card("t1")])
        XCTAssertEqual(model.attachments(of: model.cards[0]), [])
        core.messages["m-t1"] = .success(content(.empty, attachments: [file("a1"), file("a2")]))
        await model.fetchBody(model.cards[0])
        XCTAssertEqual(model.attachments(of: model.cards[0]).map(\.id), ["a1", "a2"])
    }

    func testOpenWritesUnderTheOpenedFolderAndHandsTheFileToTheOS() async {
        let theirs = card("t1", account: "b@example.com")
        let model = await fetched(theirs, attachments: [file("a1")])
        let opened = await model.openAttachment(file("a1"), of: theirs)
        XCTAssertTrue(opened)
        XCTAssertEqual(core.attachmentCalls, [
            .open(account: "b@example.com", messageId: "m-t1", attachmentId: "a1", openedDir: openedDir.path),
        ])
        XCTAssertEqual(openedFiles, [URL(fileURLWithPath: "\(openedDir.path)/0123456789abcdef/a1.pdf")])
        XCTAssertNil(model.notice)
    }

    func testAKindThatDoesNotOpenIsNeverAskedToOpen() async {
        let model = await fetched(card("t1"))
        let opened = await model.openAttachment(file("a1", openable: false), of: model.cards[0])
        XCTAssertFalse(opened)
        XCTAssertTrue(core.attachmentCalls.isEmpty)
        XCTAssertTrue(openedFiles.isEmpty)
    }

    func testAFileTheOSCannotOpenSaysSo() async {
        opens = false
        let model = await fetched(card("t1"))
        let opened = await model.openAttachment(file("a1"), of: model.cards[0])
        XCTAssertFalse(opened)
        XCTAssertEqual(model.notice, "添付ファイルを開けませんでした")
    }

    func testSaveWritesToDownloadsAndSaysUnderWhichName() async {
        let theirs = card("t1", account: "b@example.com")
        let model = await fetched(theirs)
        let saved = await model.saveAttachment(file("a1"), of: theirs)
        XCTAssertTrue(saved)
        XCTAssertEqual(core.attachmentCalls, [
            .save(account: "b@example.com", messageId: "m-t1", attachmentId: "a1", dir: downloads.path),
        ])
        XCTAssertEqual(model.notice, "ダウンロード フォルダに「a1 (1).pdf」を保存しました")
        XCTAssertTrue(openedFiles.isEmpty)
    }

    func testAFailedAttachmentCallShowsWhatWentWrong() async {
        let model = await fetched(card("t1"))
        core.attachmentError = .Network(message: "つながりません")
        let saved = await model.saveAttachment(file("a1"), of: model.cards[0])
        XCTAssertFalse(saved)
        XCTAssertEqual(model.notice, "つながりません")
        core.attachmentError = .Google(message: "開けません")
        let opened = await model.openAttachment(file("a1"), of: model.cards[0])
        XCTAssertFalse(opened)
        XCTAssertEqual(model.notice, "開けません")
        XCTAssertTrue(openedFiles.isEmpty)
    }

    func testSizesReadAsTheTauriAppWritesThem() {
        XCTAssertEqual(AppModel.formatSize(512), "512 B")
        XCTAssertEqual(AppModel.formatSize(1536), "1.5 KB")
        XCTAssertEqual(AppModel.formatSize(20 * 1024), "20 KB")
        XCTAssertEqual(AppModel.formatSize(5 * 1024 * 1024), "5.0 MB")
        XCTAssertEqual(AppModel.formatSize(3 * 1024 * 1024 * 1024 * 1024), "3072 GB")
    }
}
