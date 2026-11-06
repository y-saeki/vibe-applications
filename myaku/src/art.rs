//! myaku's icon as pixels: a pulse line on a dark rounded square.
//!
//! - `pixels`: the tile with a thin margin, for Windows. build.rs draws it
//!   into the executable's icon.
//! - `mac_pixels`: the same tile inside the margin macOS app icons keep
//!   (824 of 1024), for the app's `.icns`.
//! - `tray_pixels`: the pulse line alone, in white or black, for the Windows
//!   notification area, which does not colour icons itself.
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform, and build.rs includes this file as it is.

const TILE_TOP: Rgb = (0x2A, 0x30, 0x3F);
const TILE_BOTTOM: Rgb = (0x12, 0x15, 0x1D);
const PULSE: Rgb = (0x3D, 0xD9, 0x9B);

type Rgb = (u8, u8, u8);
type Point = (f32, f32);

/// The pulse, as fractions of the square's edge: flat, a peak, a trough, a
/// small rebound and flat again.
const LINE: [Point; 7] = [
    (0.14, 0.56),
    (0.33, 0.56),
    (0.43, 0.27),
    (0.56, 0.76),
    (0.65, 0.47),
    (0.71, 0.56),
    (0.86, 0.56),
];
/// The stroke's width as a fraction of the tile's edge.
const TILE_STROKE: f32 = 0.085;
/// Bolder in the notification area, where the icon is 16 pixels or so.
const TRAY_STROKE: f32 = 0.125;
/// The tile's corner radius as a fraction of its edge.
const TILE_RADIUS: f32 = 0.2333;

/// The icon for Windows: the tile with a margin of 1/32 of `size` on each
/// side. `size` is the edge in pixels; the result is top-down ARGB, not
/// premultiplied.
pub fn pixels(size: usize) -> Vec<u32> {
    tile(size, (size / 32) as f32)
}

/// The icon for the macOS app: the tile 824/1024 of `size` across, centred,
/// as macOS app icons are drawn.
pub fn mac_pixels(size: usize) -> Vec<u32> {
    tile(size, (size as f32 * 100.0 / 1024.0).round())
}

/// The notification-area glyph: the pulse line in white on a dark taskbar,
/// in black on a light one, on transparency.
pub fn tray_pixels(size: usize, dark_taskbar: bool) -> Vec<u32> {
    let s = size as f32;
    // The line's ends reach the edges, less half a stroke.
    let line = scale(&LINE, -0.1 * s, 1.2 * s);
    let half = TRAY_STROKE * s / 2.0;
    let colour = if dark_taskbar {
        (0xFF, 0xFF, 0xFF)
    } else {
        (0, 0, 0)
    };
    let mut out = vec![0u32; size * size];
    for y in 0..size {
        for x in 0..size {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let a = coverage(polyline_distance(p, &line) - half);
            if a > 0.0 {
                out[y * size + x] = argb(a, colour);
            }
        }
    }
    out
}

/// The tile, inset by `margin` pixels on each side.
fn tile(size: usize, margin: f32) -> Vec<u32> {
    let s = size as f32;
    let edge = s - 2.0 * margin;
    let radius = TILE_RADIUS * edge;
    let half = (TILE_STROKE * edge / 2.0).max(0.75);
    let line = scale(&LINE, margin, edge);
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
            c = mix(c, PULSE, coverage(polyline_distance(p, &line) - half));
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

/// ARGB pixels as the bytes R, G, B, A, which is what the macOS app makes
/// its bitmaps from.
pub fn rgba_bytes(pixels: &[u32]) -> Vec<u8> {
    pixels
        .iter()
        .flat_map(|p| {
            let [a, r, g, b] = p.to_be_bytes();
            [r, g, b, a]
        })
        .collect()
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

    fn at(p: &[u32], size: usize, x: f32, y: f32) -> u32 {
        p[(y * size as f32) as usize * size + (x * size as f32) as usize]
    }

    #[test]
    fn a_rounded_dark_tile_with_the_pulse() {
        for size in [16, 32, 256] {
            let p = pixels(size);
            assert_eq!(p.len(), size * size);
            // Rounded corners stay transparent; the edges' middles do not.
            assert_eq!(p[0], 0, "{size}");
            assert_eq!(p[size * size - 1], 0, "{size}");
            assert_eq!(alpha(at(&p, size, 0.5, 0.05)), 255, "{size}");
            // Above the line is the tile; on the flat start of the line is
            // the pulse colour.
            let (r, _, _) = rgb(at(&p, size, 0.2, 0.2));
            assert!((TILE_BOTTOM.0..=TILE_TOP.0).contains(&r), "{size}");
        }
        let p = pixels(256);
        assert_eq!(rgb(at(&p, 256, 0.22, 0.56)), PULSE);
    }

    #[test]
    fn the_mac_icon_keeps_the_app_icon_margin() {
        let size = 1024;
        let mac = mac_pixels(size);
        let win = pixels(size);
        let px = |p: &[u32], x: usize, y: usize| p[y * size + x];
        assert_eq!(alpha(px(&win, 90, 512)), 255);
        assert_eq!(px(&mac, 90, 512), 0);
        assert_eq!(alpha(px(&mac, 110, 512)), 255);
    }

    #[test]
    fn the_tray_glyph_is_one_colour_for_the_taskbar() {
        for (dark, colour) in [(true, 0x00FF_FFFF), (false, 0)] {
            let size = 16;
            let p = tray_pixels(size, dark);
            assert!(
                p.iter()
                    .all(|&px| alpha(px) == 0 || px & 0x00FF_FFFF == colour),
                "{dark}"
            );
            assert_eq!(alpha(at(&p, size, 0.5, 0.05)), 0);
            // The flat start of the line, near the left edge.
            assert!(alpha(at(&p, size, 0.1, 0.57)) > 128, "{dark}");
        }
    }

    #[test]
    fn rgba_bytes_reorders_the_channels() {
        assert_eq!(rgba_bytes(&[0x80_11_22_33]), [0x11, 0x22, 0x33, 0x80]);
    }

    /// The `points` and `stroke-width` of the one polyline in a layer of
    /// `macos/AppIcon.icon`, in its 1024-unit canvas.
    fn layer_polyline(svg: &str) -> (Vec<Point>, f32) {
        let attr = |name: &str| {
            let start = svg.find(&format!(" {name}=\"")).expect(name) + name.len() + 3;
            let end = start + svg[start..].find('"').unwrap();
            &svg[start..end]
        };
        let points = attr("points")
            .split_whitespace()
            .map(|p| {
                let (x, y) = p.split_once(',').unwrap();
                (x.parse().unwrap(), y.parse().unwrap())
            })
            .collect();
        (points, attr("stroke-width").parse().unwrap())
    }

    /// Whether a layer's polyline is `expected` (fractions of the tile's
    /// edge) drawn `stroke` wide, to a tenth of a unit.
    fn assert_layer(svg: &str, expected: &[Point], stroke: f32) {
        let (points, width) = layer_polyline(svg);
        assert_eq!(points.len(), expected.len());
        for (p, e) in points.iter().zip(expected) {
            assert!(
                (p.0 - e.0 * 1024.0).abs() < 0.1 && (p.1 - e.1 * 1024.0).abs() < 0.1,
                "{p:?} {e:?}"
            );
        }
        assert!((width - stroke * 1024.0).abs() < 0.1, "{width}");
    }

    /// The Liquid Glass icon's layer is drawn by hand from the same
    /// geometry; this catches the two drifting apart.
    #[test]
    fn liquid_glass_layer_matches_the_tile() {
        let pulse = include_str!("../macos/AppIcon.icon/Assets/pulse.svg");
        assert_layer(pulse, &LINE, TILE_STROKE);
    }
}
