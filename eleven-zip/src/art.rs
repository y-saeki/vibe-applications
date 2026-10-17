//! eleven-zip's icon as pixels: a zipper on a rounded orange square, its teeth
//! running down from the top and its pull tab below them. The settings
//! window draws it at run time (win/icon.rs); build.rs draws it into the
//! executable's icon, which Explorer, the taskbar and the context menu show.
//!
//! Nothing here touches Win32, so it is tested on every platform, and
//! build.rs includes this file as it is.

const BACKGROUND: u32 = 0xFF_E8_6A_1E;
const FOREGROUND: u32 = 0xFF_FF_FF_FF;

/// A zipper on a rounded orange square. `size` is the edge in pixels; the
/// result is top-down ARGB.
pub fn pixels(size: usize) -> Vec<u32> {
    let s = size as f32;
    let u = s / 16.0;
    let radius = 3.0 * u;
    let center = s / 2.0;
    // The teeth: one unit tall each, alternating to the left and the right
    // of the middle, from the top edge down to the tab.
    let (teeth_top, teeth_bottom) = (2.0 * u, 9.0 * u);
    let tooth = 2.0 * u;
    // The pull tab, with a slot in it.
    let (tab_left, tab_right, tab_top, tab_bottom) =
        (center - 2.0 * u, center + 2.0 * u, 9.0 * u, 14.0 * u);
    let (slot_top, slot_bottom) = (10.5 * u, 12.5 * u);
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            // Distance outside the rounded square's straight part.
            let dx = (radius - px).max(px - (s - radius)).max(0.0);
            let dy = (radius - py).max(py - (s - radius)).max(0.0);
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let on_tooth = py >= teeth_top && py < teeth_bottom && {
                let row = ((py - teeth_top) / u).floor() as i32;
                if row % 2 == 0 {
                    px >= center - tooth && px < center
                } else {
                    px >= center && px < center + tooth
                }
            };
            let on_tab = px >= tab_left
                && px < tab_right
                && py >= tab_top
                && py < tab_bottom
                && !(px >= center - u && px < center + u && py >= slot_top && py < slot_bottom);
            out[y * size + x] = if on_tooth || on_tab {
                FOREGROUND
            } else {
                BACKGROUND
            };
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rounded_square_with_a_zipper_on_it() {
        for size in [16, 32, 256] {
            let p = pixels(size);
            let at = |x: usize, y: usize| p[y * size + x];
            let u = size / 16;
            assert_eq!(p.len(), size * size);
            // Rounded corners stay transparent; the edges' middles do not.
            assert_eq!(at(0, 0), 0, "{size}");
            assert_eq!(at(size - 1, size - 1), 0, "{size}");
            assert_eq!(at(size / 2, 0), BACKGROUND, "{size}");
            // The first tooth is left of the middle, the second right of it.
            assert_eq!(at(size / 2 - u, 2 * u), FOREGROUND, "{size}");
            assert_eq!(at(size / 2, 2 * u), BACKGROUND, "{size}");
            assert_eq!(at(size / 2, 3 * u), FOREGROUND, "{size}");
            // The tab is solid at its top and has a slot in its middle.
            assert_eq!(at(size / 2 + u, 9 * u), FOREGROUND, "{size}");
            assert_eq!(at(size / 2 + u, 11 * u), FOREGROUND, "{size}");
            assert_eq!(at(size / 2, 11 * u), BACKGROUND, "{size}");
            // Beside the zipper is background.
            assert_eq!(at(3 * u, size / 2), BACKGROUND, "{size}");
        }
    }
}
