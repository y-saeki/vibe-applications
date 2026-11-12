//! Where a window that the user moved goes when it opens again: where it
//! was left, moved back inside the screen when that screen has since shrunk,
//! changed its taskbar or gone away.

/// A rectangle in screen pixels, right and bottom exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Area {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// The top-left corner for a window of `size` that was left at `origin`,
/// kept at least `margin` inside `area`. A window larger than the area is
/// kept at its top-left, so that its heading stays reachable.
pub fn keep_inside(origin: (i32, i32), size: (i32, i32), area: Area, margin: i32) -> (i32, i32) {
    let along = |v: i32, len: i32, lo: i32, hi: i32| {
        let (lo, hi) = (lo + margin, hi - len - margin);
        v.min(hi).max(lo)
    };
    (
        along(origin.0, size.0, area.left, area.right),
        along(origin.1, size.1, area.top, area.bottom),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Area = Area {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };

    #[test]
    fn a_window_inside_stays_where_it_was() {
        assert_eq!(keep_inside((300, 200), (440, 500), SCREEN, 12), (300, 200));
    }

    #[test]
    fn a_window_over_an_edge_comes_back_inside_by_the_margin() {
        assert_eq!(
            keep_inside((1700, 900), (440, 500), SCREEN, 12),
            (1468, 528)
        );
        assert_eq!(keep_inside((-100, -50), (440, 500), SCREEN, 12), (12, 12));
    }

    #[test]
    fn a_window_on_a_screen_that_is_gone_comes_onto_this_one() {
        let right_of_it = (2500, 300);
        assert_eq!(
            keep_inside(right_of_it, (440, 500), SCREEN, 12),
            (1468, 300)
        );
    }

    #[test]
    fn a_screen_away_from_the_origin_is_measured_from_its_own_corner() {
        let left_screen = Area {
            left: -1280,
            top: 100,
            right: 0,
            bottom: 1060,
        };
        assert_eq!(
            keep_inside((-2000, 0), (440, 500), left_screen, 12),
            (-1268, 112)
        );
    }

    #[test]
    fn a_window_larger_than_the_screen_keeps_its_heading_on_it() {
        let small = Area {
            left: 0,
            top: 0,
            right: 400,
            bottom: 300,
        };
        assert_eq!(keep_inside((50, 80), (440, 500), small, 12), (12, 12));
    }
}
