//! winwin's icon as pixels: a two-tone W on an off-white rounded square.
//!
//! - `pixels`: the tile with a thin margin, for Windows. The notification
//!   area and the settings window draw it at run time (win/icon.rs); build.rs
//!   draws it into the executable's icon, which the taskbar shows.
//! - `mac_pixels`: the same tile inside the margin macOS app icons keep
//!   (824 of 1024), for the app's `.icns`.
//! - `menu_bar_pixels`: the W alone in black, its right half fainter, for
//!   the macOS menu bar, which uses it as a template image and colours it.
//!
//! Nothing here touches Win32, so it is tested on every platform, and
//! build.rs includes this file as it is.

const TILE_TOP: Rgb = (0xF7, 0xF4, 0xEE);
const TILE_BOTTOM: Rgb = (0xE6, 0xE1, 0xD6);
const INK: Rgb = (0x1E, 0x1B, 0x4B);
const TEAL: Rgb = (0x0F, 0xB5, 0xA0);

/// How opaque the menu-bar glyph's right half is, against 255 for the left.
const MENU_BAR_FAINT: f32 = 0.45;

type Rgb = (u8, u8, u8);
type Point = (f32, f32);

/// The W on the tile, as fractions of the tile's edge: the left V, then the
/// right one, sharing the middle peak.
const TILE_LEFT: [Point; 3] = [(0.175, 0.275), (0.3417, 0.725), (0.5, 0.3667)];
const TILE_RIGHT: [Point; 3] = [(0.5, 0.3667), (0.6583, 0.725), (0.825, 0.275)];
/// The stroke's width as a fraction of the tile's edge.
const TILE_STROKE: f32 = 0.15;
/// The tile's corner radius as a fraction of its edge.
const TILE_RADIUS: f32 = 0.2333;

/// The menu-bar W, bolder and filling its square, as fractions of the edge.
const GLYPH_LEFT: [Point; 3] = [(0.1016, 0.2266), (0.3047, 0.8047), (0.5, 0.3516)];
const GLYPH_RIGHT: [Point; 3] = [(0.5, 0.3516), (0.6953, 0.8047), (0.8984, 0.2266)];
const GLYPH_STROKE: f32 = 0.1953;

/// The icon for Windows: the tile with a margin of 1/32 of `size` on each
/// side, rounded down to whole pixels so the tile's edges stay sharp. `size`
/// is the edge in pixels; the result is top-down ARGB, not premultiplied.
pub fn pixels(size: usize) -> Vec<u32> {
    tile(size, (size / 32) as f32)
}

/// The icon for the macOS app: the tile 824/1024 of `size` across, centred,
/// as macOS app icons are drawn.
pub fn mac_pixels(size: usize) -> Vec<u32> {
    tile(size, (size as f32 * 100.0 / 1024.0).round())
}

/// The menu-bar glyph: the W in black, its left half opaque and its right
/// half at `MENU_BAR_FAINT`, on transparency. Only the alpha counts, as the
/// menu bar colours a template image itself.
pub fn menu_bar_pixels(size: usize) -> Vec<u32> {
    let s = size as f32;
    let half = GLYPH_STROKE * s / 2.0;
    let left = scale(&GLYPH_LEFT, 0.0, s);
    let right = scale(&GLYPH_RIGHT, 0.0, s);
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let l = coverage(polyline_distance(p, &left) - half);
            let r = coverage(polyline_distance(p, &right) - half) * MENU_BAR_FAINT;
            // The left half over the right, as it lies on the app icon.
            let a = l + r * (1.0 - l);
            out[y * size + x] = argb(a, (0, 0, 0));
        }
    }
    out
}

/// The tile, inset by `margin` pixels on each side.
fn tile(size: usize, margin: f32) -> Vec<u32> {
    let s = size as f32;
    let edge = s - 2.0 * margin;
    let radius = TILE_RADIUS * edge;
    let half = TILE_STROKE * edge / 2.0;
    let left = scale(&TILE_LEFT, margin, edge);
    let right = scale(&TILE_RIGHT, margin, edge);
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let a = coverage(rounded_square_distance(p, margin, edge, radius));
            if a <= 0.0 {
                continue;
            }
            let t = ((p.1 - margin) / edge).clamp(0.0, 1.0);
            let mut c = mix(TILE_TOP, TILE_BOTTOM, t);
            c = mix(c, INK, coverage(polyline_distance(p, &left) - half));
            c = mix(c, TEAL, coverage(polyline_distance(p, &right) - half));
            out[y * size + x] = argb(a, c);
        }
    }
    out
}

/// `points`, given as fractions, placed on a square `edge` across whose top
/// left corner is at (`origin`, `origin`).
fn scale<const N: usize>(points: &[Point; N], origin: f32, edge: f32) -> [Point; N] {
    points.map(|(x, y)| (origin + x * edge, origin + y * edge))
}

/// How much of a pixel a shape covers, from the signed distance of the
/// pixel's centre to the shape's edge (negative inside).
fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

/// Signed distance from `p` to a rounded square.
fn rounded_square_distance(p: Point, origin: f32, edge: f32, radius: f32) -> f32 {
    let centre = origin + edge / 2.0;
    let inner = edge / 2.0 - radius;
    let dx = (p.0 - centre).abs() - inner;
    let dy = (p.1 - centre).abs() - inner;
    let outside = dx.max(0.0).hypot(dy.max(0.0));
    outside + dx.max(dy).min(0.0) - radius
}

/// Distance from `p` to the nearest point of a polyline.
fn polyline_distance(p: Point, points: &[Point]) -> f32 {
    points
        .windows(2)
        .map(|w| segment_distance(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let (abx, aby) = (b.0 - a.0, b.1 - a.1);
    let (apx, apy) = (p.0 - a.0, p.1 - a.1);
    let t = ((apx * abx + apy * aby) / (abx * abx + aby * aby)).clamp(0.0, 1.0);
    (apx - t * abx).hypot(apy - t * aby)
}

fn mix(from: Rgb, to: Rgb, t: f32) -> Rgb {
    let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
    (m(from.0, to.0), m(from.1, to.1), m(from.2, to.2))
}

fn argb(alpha: f32, (r, g, b): Rgb) -> u32 {
    let a = (alpha * 255.0).round() as u32;
    a << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(p: u32) -> Rgb {
        ((p >> 16) as u8, (p >> 8) as u8, p as u8)
    }

    fn alpha(p: u32) -> u32 {
        p >> 24
    }

    #[test]
    fn a_rounded_tile_with_a_two_tone_w() {
        for size in [16, 32, 256] {
            let p = pixels(size);
            let at =
                |x: f32, y: f32| p[(y * size as f32) as usize * size + (x * size as f32) as usize];
            assert_eq!(p.len(), size * size);
            // Rounded corners stay transparent; the edges' middles do not.
            assert_eq!(p[0], 0, "{size}");
            assert_eq!(p[size * size - 1], 0, "{size}");
            assert_eq!(alpha(at(0.5, 0.05)), 255, "{size}");
            // The tile between the W's arms is off-white, from the top's
            // shade to the bottom's.
            let (r, g, b) = rgb(at(0.5, 0.1));
            assert!((TILE_BOTTOM.0..=TILE_TOP.0).contains(&r), "{size}");
            assert!((TILE_BOTTOM.1..=TILE_TOP.1).contains(&g), "{size}");
            assert!((TILE_BOTTOM.2..=TILE_TOP.2).contains(&b), "{size}");
            // The bottom of the left V is ink, of the right V teal.
            assert_eq!(rgb(at(0.34, 0.72)), INK, "{size}");
            assert_eq!(rgb(at(0.66, 0.72)), TEAL, "{size}");
        }
    }

    #[test]
    fn the_mac_icon_keeps_the_app_icon_margin() {
        let size = 1024;
        let mac = mac_pixels(size);
        let win = pixels(size);
        // 100 pixels in from the middle of the left edge: inside on Windows,
        // the margin on macOS.
        let at = |p: &[u32], x: usize, y: usize| p[y * size + x];
        assert_eq!(alpha(at(&win, 90, 512)), 255);
        assert_eq!(at(&mac, 90, 512), 0);
        assert_eq!(alpha(at(&mac, 110, 512)), 255);
        // The W is the same drawing, only smaller.
        assert_eq!(rgb(at(&mac, 512 - 132, 690)), INK);
        assert_eq!(rgb(at(&mac, 512 + 132, 690)), TEAL);
    }

    #[test]
    fn the_menu_bar_glyph_is_black_with_a_fainter_right_half() {
        for size in [18, 36] {
            let p = menu_bar_pixels(size);
            let at =
                |x: f32, y: f32| p[(y * size as f32) as usize * size + (x * size as f32) as usize];
            // Black throughout; only the alpha says where the W is.
            assert!(p.iter().all(|&px| px & 0x00FF_FFFF == 0), "{size}");
            assert_eq!(alpha(at(0.0, 0.0)), 0, "{size}");
            assert_eq!(alpha(at(0.5, 0.9)), 0, "{size}");
            assert_eq!(alpha(at(0.3, 0.75)), 255, "{size}");
            let faint = alpha(at(0.7, 0.75));
            assert_eq!(faint, (MENU_BAR_FAINT * 255.0).round() as u32, "{size}");
        }
    }
}
