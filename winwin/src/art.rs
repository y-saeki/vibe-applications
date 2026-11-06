//! winwin's icon as pixels: a two-tone W on an off-white rounded square.
//! From `DETAILED_FROM` pixels up the tile is shaded: the tile and the W's
//! halves have soft gradients, both halves cast a light shadow, and the
//! teal half is frosted glass through which the ink half shows blurred.
//! Smaller sizes are drawn flat, as shading would only blur them.
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

/// From this edge in pixels up, the tile is drawn shaded rather than flat.
pub const DETAILED_FROM: usize = 48;

// The shaded tile's colours: each gradient runs top to bottom.
const SHADED_TILE: (Rgb, Rgb) = ((0xFC, 0xFA, 0xF6), (0xE3, 0xDD, 0xD0));
const SHADED_INK: (Rgb, Rgb) = ((0x34, 0x30, 0x7A), (0x17, 0x14, 0x3D));
const SHADED_TEAL: (Rgb, Rgb) = ((0x3F, 0xE0, 0xC8), (0x0A, 0x9C, 0x8A));
/// Where the W's gradients start and end, as fractions of the tile's edge.
const STROKE_GRADIENT: (f32, f32) = (0.2167, 0.8);
const INK_SHADOW: Rgb = (0x3B, 0x2F, 0x1A);
const TEAL_SHADOW: Rgb = (0x0B, 0x15, 0x30);
/// How opaque the teal glass is over what it frosts.
const GLASS_OPACITY: f32 = 0.75;

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
    if size >= DETAILED_FROM {
        shaded_tile(size, margin)
    } else {
        flat_tile(size, margin)
    }
}

/// The tile in flat colours, for small sizes.
fn flat_tile(size: usize, margin: f32) -> Vec<u32> {
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

/// A pixel's colour with its alpha, the colour premultiplied by it.
type Px = [f32; 4];

/// The tile shaded: lengths below are in 240ths of the tile's edge, the
/// units the design was drawn in.
fn shaded_tile(size: usize, margin: f32) -> Vec<u32> {
    let s = size as f32;
    let edge = s - 2.0 * margin;
    let u = edge / 240.0;
    let radius = TILE_RADIUS * edge;
    let half = TILE_STROKE * edge / 2.0;
    let left = scale(&TILE_LEFT, margin, edge);
    let right = scale(&TILE_RIGHT, margin, edge);
    let along = |y: f32, (from, to): (f32, f32)| {
        ((((y - margin) / edge) - from) / (to - from)).clamp(0.0, 1.0)
    };
    let mask = |f: &dyn Fn(Point) -> f32| -> Vec<f32> {
        (0..size * size)
            .map(|i| f(((i % size) as f32 + 0.5, (i / size) as f32 + 0.5)))
            .collect()
    };
    let stroke = |points: &[Point; 3], dx: f32, dy: f32| {
        mask(&|p| coverage(polyline_distance((p.0 - dx, p.1 - dy), points) - half))
    };
    let tile = mask(&|p| coverage(rounded_square_distance(p, margin, edge, radius)));
    let ink = stroke(&left, 0.0, 0.0);
    let teal = stroke(&right, 0.0, 0.0);
    let ink_shadow = blur_mask(stroke(&left, 0.0, 4.0 * u), size, 4.0 * u);
    let teal_shadow = blur_mask(stroke(&right, -3.0 * u, 5.0 * u), size, 5.0 * u);

    // Everything the glass lies on: the tile, the ink half and both shadows.
    let mut under: Vec<Px> = (0..size * size)
        .map(|i| {
            let y = (i / size) as f32 + 0.5;
            let t = ((y - margin) / edge).clamp(0.0, 1.0);
            let mut px = solid(mix(SHADED_TILE.0, SHADED_TILE.1, t), tile[i]);
            px = over(px, solid(INK_SHADOW, 0.16 * ink_shadow[i] * tile[i]));
            px = over(
                px,
                solid(gradient(SHADED_INK, along(y, STROKE_GRADIENT)), ink[i]),
            );
            over(px, solid(TEAL_SHADOW, 0.35 * teal_shadow[i] * tile[i]))
        })
        .collect();

    // The glass: what lies under it, blurred and whitened, with the teal
    // over it and a sheen that fades from its top.
    let frosted = blur(&under, size, 6.0 * u);
    let rim_half = 16.5 * u;
    for (i, px) in under.iter_mut().enumerate() {
        if teal[i] <= 0.0 {
            continue;
        }
        let p = ((i % size) as f32 + 0.5, (i / size) as f32 + 0.5);
        let mut glass = over(frosted[i], solid((0xFF, 0xFF, 0xFF), 0.2));
        let colour = gradient(SHADED_TEAL, along(p.1, STROKE_GRADIENT));
        glass = over(glass, solid(colour, GLASS_OPACITY));
        let sheen = sheen(((p.1 - margin) / edge - 0.2) / 0.6);
        let rim = coverage(polyline_distance(p, &right) - rim_half);
        glass = over(glass, solid((0xFF, 0xFF, 0xFF), sheen * rim));
        *px = lerp(*px, glass, teal[i]);
    }
    under.into_iter().map(to_argb).collect()
}

/// The sheen's opacity `t` of the way down the W: strong at its top, gone
/// by 40%, and faint again at the bottom.
fn sheen(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    if t < 0.4 {
        0.45 * (1.0 - t / 0.4)
    } else {
        0.12 * (t - 0.4) / 0.6
    }
}

fn gradient((from, to): (Rgb, Rgb), t: f32) -> Rgb {
    mix(from, to, t)
}

fn solid((r, g, b): Rgb, alpha: f32) -> Px {
    let c = |v: u8| v as f32 / 255.0 * alpha;
    [c(r), c(g), c(b), alpha]
}

/// `top` laid over `bottom`.
fn over(bottom: Px, top: Px) -> Px {
    let k = 1.0 - top[3];
    [0, 1, 2, 3].map(|i| top[i] + bottom[i] * k)
}

fn lerp(a: Px, b: Px, t: f32) -> Px {
    [0, 1, 2, 3].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn to_argb(px: Px) -> u32 {
    let a = px[3].clamp(0.0, 1.0);
    if a <= 0.0 {
        return 0;
    }
    let c = |v: f32| ((v / a).clamp(0.0, 1.0) * 255.0).round() as u8;
    argb(a, (c(px[0]), c(px[1]), c(px[2])))
}

fn blur_mask(mask: Vec<f32>, size: usize, sigma: f32) -> Vec<f32> {
    let px: Vec<Px> = mask.into_iter().map(|a| [0.0, 0.0, 0.0, a]).collect();
    blur(&px, size, sigma).into_iter().map(|p| p[3]).collect()
}

/// A Gaussian blur of a `size` square, approximated by three box blurs;
/// outside the square counts as transparent.
fn blur(pixels: &[Px], size: usize, sigma: f32) -> Vec<Px> {
    let mut out = pixels.to_vec();
    for radius in box_radii(sigma) {
        out = box_blur(&out, size, radius, true);
        out = box_blur(&out, size, radius, false);
    }
    out
}

/// The radii of three box blurs that together approximate a Gaussian of
/// `sigma` (Kovesi, "Fast almost-Gaussian filtering").
fn box_radii(sigma: f32) -> [usize; 3] {
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut lower = ideal.floor() as i32;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let wl = lower as f32;
    let m = ((12.0 * sigma * sigma - n * wl * wl - 4.0 * n * wl - 3.0 * n) / (-4.0 * wl - 4.0))
        .round() as i32;
    [0, 1, 2].map(|i| {
        let width = if i < m { lower } else { lower + 2 };
        (width as usize - 1) / 2
    })
}

fn box_blur(pixels: &[Px], size: usize, radius: usize, horizontal: bool) -> Vec<Px> {
    let mut out = vec![[0.0; 4]; pixels.len()];
    if radius == 0 {
        return pixels.to_vec();
    }
    let width = (2 * radius + 1) as f32;
    let index = |line: usize, k: usize| {
        if horizontal {
            line * size + k
        } else {
            k * size + line
        }
    };
    for line in 0..size {
        let mut sum = [0.0f32; 4];
        for k in 0..radius.min(size) {
            let p = pixels[index(line, k)];
            (0..4).for_each(|c| sum[c] += p[c]);
        }
        for k in 0..size {
            if k + radius < size {
                let p = pixels[index(line, k + radius)];
                (0..4).for_each(|c| sum[c] += p[c]);
            }
            if k > radius {
                let p = pixels[index(line, k - radius - 1)];
                (0..4).for_each(|c| sum[c] -= p[c]);
            }
            out[index(line, k)] = sum.map(|v| v / width);
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

    fn luminance(p: u32) -> f32 {
        let (r, g, b) = rgb(p);
        0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32
    }

    #[test]
    fn small_tiles_are_a_flat_two_tone_w() {
        for size in [16, 32] {
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
    fn large_tiles_are_shaded_with_a_glass_right_half() {
        let size = 256;
        let p = pixels(size);
        // Fractions of the tile, which starts 8 pixels in at this size.
        let at = |x: f32, y: f32| {
            let px = |f: f32| (8.0 + f * 240.0) as usize;
            p[px(y) * size + px(x)]
        };
        assert_eq!(p[0], 0);
        assert_eq!(alpha(at(0.5, 0.0)), 255);
        // The tile is lighter at the top than at the bottom.
        assert!(luminance(at(0.5, 0.05)) > luminance(at(0.5, 0.95)) + 10.0);
        // The ink half is dark and blue, darker at its foot than at its top.
        let foot = at(0.3417, 0.72);
        let (r, _, b) = rgb(foot);
        assert!(b > r && luminance(foot) < 40.0);
        assert!(luminance(at(0.175, 0.28)) > luminance(foot));
        // The teal half is green-blue.
        let teal = at(0.6583, 0.72);
        let (r, g, b) = rgb(teal);
        assert!(g > r + 60 && g > b, "{:?}", rgb(teal));
        // Through the glass the ink half shows, darker than the teal over
        // the tile at the same height.
        let over_ink = at(0.476, 0.4207);
        let over_tile = at(0.771, 0.4207);
        assert!(luminance(over_ink) + 15.0 < luminance(over_tile));
        // The ink half casts a shadow below its foot.
        assert!(luminance(at(0.3417, 0.815)) + 5.0 < luminance(at(0.1, 0.815)));
        // Everything is opaque inside the tile.
        assert!([foot, teal, over_ink].iter().all(|&px| alpha(px) == 255));
    }

    #[test]
    fn the_mac_icon_keeps_the_app_icon_margin() {
        let size = 512;
        let mac = mac_pixels(size);
        let win = pixels(size);
        // 50 pixels in from the middle of the left edge: inside on Windows,
        // the margin on macOS.
        let at = |p: &[u32], x: usize, y: usize| p[y * size + x];
        assert_eq!(alpha(at(&win, 45, 256)), 255);
        assert_eq!(at(&mac, 45, 256), 0);
        assert_eq!(alpha(at(&mac, 55, 256)), 255);
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

    /// The Liquid Glass icon's layers are drawn by hand from the same
    /// geometry; this catches the two drifting apart.
    #[test]
    fn liquid_glass_layers_match_the_tile() {
        let ink = include_str!("../macos/AppIcon.icon/Assets/ink.svg");
        let teal = include_str!("../macos/AppIcon.icon/Assets/teal.svg");
        assert_layer(ink, &TILE_LEFT, TILE_STROKE);
        assert_layer(teal, &TILE_RIGHT, TILE_STROKE);
    }
}
