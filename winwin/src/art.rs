//! winwin's icon as pixels: a window outline on a rounded blue square, its
//! left half filled, as a window snapped to one side. The notification area
//! and the settings window draw it at run time (win/icon.rs); build.rs draws
//! it into the executable's icon, which the taskbar shows.
//!
//! Nothing here touches Win32, so it is tested on every platform, and
//! build.rs includes this file as it is.

const BACKGROUND: u32 = 0xFF_2F_6F_EB;
const FOREGROUND: u32 = 0xFF_FF_FF_FF;

/// A window outline on a rounded blue square, its left half filled: a window
/// snapped to one side. `size` is the edge in pixels; the result is
/// top-down ARGB.
pub fn pixels(size: usize) -> Vec<u32> {
    let s = size as f32;
    let u = s / 16.0;
    let radius = 3.0 * u;
    let stroke = u.round().max(1.0);
    let (fl, fr, ft, fb) = (3.0 * u, 13.0 * u, 4.0 * u, 12.0 * u);
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
            let inside_frame = px >= fl && px < fr && py >= ft && py < fb;
            let on_stroke = inside_frame
                && (px < fl + stroke || px >= fr - stroke || py < ft + stroke || py >= fb - stroke);
            let in_left_half = inside_frame && px < (fl + fr) / 2.0;
            out[y * size + x] = if on_stroke || in_left_half {
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
    fn a_rounded_square_with_a_window_on_it() {
        for size in [16, 32, 256] {
            let p = pixels(size);
            let at = |x: usize, y: usize| p[y * size + x];
            assert_eq!(p.len(), size * size);
            // Rounded corners stay transparent; the edges' middles do not.
            assert_eq!(at(0, 0), 0, "{size}");
            assert_eq!(at(size - 1, size - 1), 0, "{size}");
            assert_eq!(at(size / 2, 0), BACKGROUND, "{size}");
            // The left half of the window is filled, the right half is not.
            assert_eq!(at(size * 5 / 16, size / 2), FOREGROUND, "{size}");
            assert_eq!(at(size * 10 / 16, size / 2), BACKGROUND, "{size}");
        }
    }
}
