import Foundation
import HatenaFeed
import HomeShelves
import XCTest

final class ShelfPreferencesTests: XCTestCase {
    func testDefaultShowsEveryShelfWithMyHotEntryLast() {
        let shelves = ShelfPreferences.default.shownShelves
        XCTAssertEqual(shelves.count, HotEntryCategory.allCases.count + 1)
        XCTAssertEqual(shelves.first, .hotEntry(.all))
        XCTAssertEqual(shelves.last, .myHotEntry)
    }

    func testHiddenShelfLeavesTheShownOnesButKeepsItsPlace() {
        var preferences = ShelfPreferences.default
        preferences.setShown(false, for: .hotEntry(.general))
        XCTAssertFalse(preferences.shownShelves.contains(.hotEntry(.general)))
        XCTAssertEqual(preferences.entries[1].shelf, .hotEntry(.general))
        preferences.setShown(true, for: .hotEntry(.general))
        XCTAssertEqual(preferences, .default)
    }

    func testMoveFollowsOnMove() {
        var preferences = ShelfPreferences.default
        preferences.move(fromOffsets: [10], toOffset: 0)
        XCTAssertEqual(preferences.shownShelves.prefix(2), [.myHotEntry, .hotEntry(.all)])
        preferences.move(fromOffsets: [0], toOffset: 11)
        XCTAssertEqual(preferences, .default)
        preferences.move(fromOffsets: [0, 2], toOffset: 4)
        XCTAssertEqual(
            preferences.shownShelves.prefix(5),
            [.hotEntry(.general), .hotEntry(.economics), .hotEntry(.all), .hotEntry(.social), .hotEntry(.life)]
        )
    }

    func testRoundTrip() throws {
        var preferences = ShelfPreferences.default
        preferences.move(fromOffsets: [6], toOffset: 0)
        preferences.setShown(false, for: .hotEntry(.game))
        let data = try JSONEncoder().encode(preferences)
        XCTAssertEqual(try JSONDecoder().decode(ShelfPreferences.self, from: data), preferences)
    }

    func testStoresTheFeedNames() throws {
        let data = try JSONEncoder().encode(ShelfPreferences.default)
        let text = String(decoding: data, as: UTF8.self)
        XCTAssertTrue(text.contains("\"it\""))
        XCTAssertTrue(text.contains("\"myhotentry\""))
    }

    func testUnknownShelvesAreSkippedAndMissingOnesAppendedShown() throws {
        let json = """
        {"entries": [
            {"shelf": "it", "isShown": true},
            {"shelf": "someday", "isShown": true},
            {"shelf": "all", "isShown": false},
            {"shelf": "it", "isShown": false}
        ]}
        """
        let preferences = try JSONDecoder().decode(ShelfPreferences.self, from: Data(json.utf8))
        XCTAssertEqual(preferences.entries.count, HomeShelf.allCases.count)
        XCTAssertEqual(preferences.entries[0], .init(shelf: .hotEntry(.it), isShown: true))
        XCTAssertEqual(preferences.entries[1], .init(shelf: .hotEntry(.all), isShown: false))
        XCTAssertTrue(preferences.entries.dropFirst(2).allSatisfy(\.isShown))
        XCTAssertEqual(preferences.shownShelves.first, .hotEntry(.it))
    }
}
