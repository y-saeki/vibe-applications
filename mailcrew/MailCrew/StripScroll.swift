import SwiftUI

/// Where a scroll of the strip comes to rest.
///
/// `.viewAligned` alone settles on the card nearest to where the scroll would
/// end, so a swipe has to carry the strip more than half a card before the
/// next one comes in, and the strip feels heavy. This moves on as soon as the
/// scroll goes `commitFraction` of a card past where it started, then leaves
/// the exact resting place to `.viewAligned`, so the cards stay where they
/// always rested.
struct StripScrollBehavior: ScrollTargetBehavior {
    /// How far one card moves the strip: its width and the gap after it.
    let page: CGFloat

    /// The part of a card a scroll must cover to move on to the next one.
    static let commitFraction: CGFloat = 0.15

    func updateTarget(_ target: inout ScrollTarget, context: TargetContext) {
        let start = context.originalTarget.rect.minX
        target.rect.origin.x = Self.committedOffset(
            start: start,
            proposed: target.rect.minX,
            page: page
        )
        ViewAlignedScrollTargetBehavior().updateTarget(&target, context: context)
    }

    /// The offset a scroll that started at `start` and would end at
    /// `proposed` moves to: a whole number of cards from `start`, one more
    /// each time the scroll goes `commitFraction` into the next card.
    static func committedOffset(start: CGFloat, proposed: CGFloat, page: CGFloat) -> CGFloat {
        guard page > 0 else { return proposed }
        let moved = proposed - start
        let commit = page * commitFraction
        guard abs(moved) >= commit else { return start }
        let steps = ((abs(moved) - commit) / page).rounded(.down) + 1
        return start + (moved < 0 ? -steps : steps) * page
    }
}
