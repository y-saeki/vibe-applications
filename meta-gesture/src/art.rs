//! meta-gesture's icon as pixels: the gesture ↓→ drawn as one stroke, from
//! a dot where the button went down to an arrowhead where it came up, on an
//! indigo rounded square.
//!
//! - `pixels`: the tile with a thin margin, for Windows. The notification
//!   area and the settings window draw it at run time (win/icon.rs); build.rs
//!   draws it into the executable's icon, which the taskbar shows.
//! - `mac_pixels`: the same tile inside the margin macOS app icons keep
//!   (824 of 1024), for the app's `.icns`.
//! - `menu_bar_pixels`: the stroke alone in black, for the macOS menu bar,
//!   which uses it as a template image and colours it.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform, and build.rs includes this file as it is.

type Rgb = (u8, u8, u8);
type Point = (f32, f32);

const TILE_TOP: Rgb = (0x3A, 0x3F, 0x9E);
const TILE_BOTTOM: Rgb = (0x1C, 0x1F, 0x5C);
const STROKE: Rgb = (0xFF, 0xC8, 0x4A);

/// The tile's corner radius as a fraction of its edge.
const TILE_RADIUS: f32 = 0.2333;

/// The stroke on the tile, as fractions of the tile's edge: down, then
/// right.
const PATH: [Point; 3] = [(0.32, 0.25), (0.32, 0.68), (0.74, 0.68)];
/// The arrowhead's two arms, from the tip at the end of the path.
const HEAD: [Point; 2] = [(0.60, 0.54), (0.60, 0.82)];
const WIDTH: f32 = 0.10;
/// The dot where the stroke starts.
const DOT: f32 = 0.085;

/// The menu-bar glyph fills more of its square, with a bolder stroke.
const GLYPH_INSET: f32 = -0.1;
const GLYPH_WIDTH: f32 = 0.15;

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

/// The menu-bar glyph: the stroke in black on transparency. Only the alpha
/// counts, as the menu bar colours a template image itself.
pub fn menu_bar_pixels(size: usize) -> Vec<u32> {
    let s = size as f32;
    let origin = GLYPH_INSET * s;
    let edge = s - 2.0 * origin;
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let a = stroke_coverage(p, origin, edge, GLYPH_WIDTH);
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
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let a = coverage(rounded_square_distance(p, margin, edge, radius));
            if a <= 0.0 {
                continue;
            }
            let t = ((p.1 - margin) / edge).clamp(0.0, 1.0);
            let c = mix(TILE_TOP, TILE_BOTTOM, t);
            let c = mix(c, STROKE, stroke_coverage(p, margin, edge, WIDTH));
            out[y * size + x] = argb(a, c);
        }
    }
    out
}

/// How much of the pixel at `p` the stroke, its arrowhead and its dot
/// cover, the drawing scaled to a square of `edge` at `origin`.
fn stroke_coverage(p: Point, origin: f32, edge: f32, width: f32) -> f32 {
    let at = |(x, y): Point| (origin + x * edge, origin + y * edge);
    let path = PATH.map(at);
    let tip = path[2];
    let half = width * edge / 2.0;
    let line = polyline_distance(p, &path)
        .min(segment_distance(p, at(HEAD[0]), tip))
        .min(segment_distance(p, at(HEAD[1]), tip));
    let dot = distance(p, path[0]) - DOT * edge;
    coverage((line - half).min(dot))
}

fn coverage(distance: f32) -> f32 {
    (0.5 - distance).clamp(0.0, 1.0)
}

fn rounded_square_distance(p: Point, origin: f32, edge: f32, radius: f32) -> f32 {
    let half = edge / 2.0;
    let center = (origin + half, origin + half);
    let q = (
        (p.0 - center.0).abs() - (half - radius),
        (p.1 - center.1).abs() - (half - radius),
    );
    let outside = (q.0.max(0.0).powi(2) + q.1.max(0.0).powi(2)).sqrt();
    outside + q.0.max(q.1).min(0.0) - radius
}

fn polyline_distance(p: Point, points: &[Point]) -> f32 {
    points
        .windows(2)
        .map(|w| segment_distance(p, w[0], w[1]))
        .fold(f32::INFINITY, f32::min)
}

fn segment_distance(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 == 0.0 {
        0.0
    } else {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0)
    };
    distance(p, (a.0 + t * dx, a.1 + t * dy))
}

fn distance(a: Point, b: Point) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn mix(from: Rgb, to: Rgb, t: f32) -> Rgb {
    let m = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * t).round() as u8;
    (m(from.0, to.0), m(from.1, to.1), m(from.2, to.2))
}

fn argb(alpha: f32, (r, g, b): Rgb) -> u32 {
    let a = (alpha * 255.0).round() as u32;
    (a << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(pixels: &[u32], size: usize, x: f32, y: f32) -> u32 {
        let (x, y) = ((x * size as f32) as usize, (y * size as f32) as usize);
        pixels[y * size + x]
    }

    fn rgb(p: u32) -> Rgb {
        ((p >> 16) as u8, (p >> 8) as u8, p as u8)
    }

    #[test]
    fn the_tile_has_round_corners_and_the_stroke_on_it() {
        for size in [16, 32, 256] {
            let p = pixels(size);
            assert_eq!(p.len(), size * size);
            // Corners are transparent, the middle of the tile opaque.
            assert_eq!(p[0] >> 24, 0, "{size}");
            assert_eq!(at(&p, size, 0.85, 0.2) >> 24, 0xFF, "{size}");
            // The stroke's corner is drawn in its colour, as far as the
            // pixels at 16 allow; open tile is not.
            let (r, _, b) = rgb(at(&p, size, 0.32, 0.68));
            assert!(r > 0xE0 && b < 0x60, "{size}");
            assert_ne!(rgb(at(&p, size, 0.85, 0.2)), STROKE, "{size}");
        }
    }

    #[test]
    fn the_mac_icon_keeps_the_app_icon_margin() {
        let p = mac_pixels(1024);
        assert_eq!(at(&p, 1024, 0.05, 0.5) >> 24, 0);
        assert_eq!(at(&p, 1024, 0.12, 0.5) >> 24, 0xFF);
    }

    #[test]
    fn the_menu_bar_glyph_is_black_on_transparency() {
        let p = menu_bar_pixels(36);
        assert!(p.iter().all(|&px| px & 0x00FF_FFFF == 0));
        assert_eq!(p[0] >> 24, 0);
        assert!(p.iter().any(|&px| px >> 24 == 0xFF));
    }
}
