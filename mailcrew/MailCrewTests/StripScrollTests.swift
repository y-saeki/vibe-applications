import SwiftUI
import XCTest
@testable import MailCrewMac

/// Where a swipe along the strip comes to rest: the next card as soon as the
/// swipe goes a little way into it, not only past its middle.
final class StripScrollTests: XCTestCase {
    private let page: CGFloat = 582
    private var commit: CGFloat { page * StripScrollBehavior.commitFraction }

    private func rest(from start: CGFloat, to proposed: CGFloat) -> CGFloat {
        StripScrollBehavior.committedOffset(start: start, proposed: proposed, page: page)
    }

    func testASmallSwipeStaysOnTheCard() {
        XCTAssertEqual(rest(from: 0, to: 0), 0)
        XCTAssertEqual(rest(from: page, to: page + commit - 1), page)
        XCTAssertEqual(rest(from: page, to: page - commit + 1), page)
    }

    /// Well short of half a card, which is where `.viewAligned` alone moved on.
    func testASwipePastTheCommitPointMovesOneCard() {
        XCTAssertLessThan(commit, page / 2)
        XCTAssertEqual(rest(from: page, to: page + commit + 1), 2 * page)
        XCTAssertEqual(rest(from: page, to: page - commit - 1), 0)
        XCTAssertEqual(rest(from: page, to: page + page / 2), 2 * page)
        XCTAssertEqual(rest(from: page, to: page + page), 2 * page)
    }

    /// A long flick moves on by every card it goes `commitFraction` into.
    func testALongFlickMovesSeveralCards() {
        XCTAssertEqual(rest(from: 0, to: page + commit - 1), page)
        XCTAssertEqual(rest(from: 0, to: page + commit + 1), 2 * page)
        XCTAssertEqual(rest(from: 0, to: 3.5 * page), 4 * page)
        XCTAssertEqual(rest(from: 4 * page, to: page), page)
    }

    func testWithoutAPageTheProposedOffsetStands() {
        XCTAssertEqual(StripScrollBehavior.committedOffset(start: 0, proposed: 40, page: 0), 40)
    }
}
