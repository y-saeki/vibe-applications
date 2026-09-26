//! Where a shortcut puts a window, and the arithmetic that turns that into a
//! rectangle on a given monitor.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A length along one axis of the monitor's work area, as a share of it:
/// 1/2 is half, 1 the whole. Always greater than 0 and at most 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Ratio(f64);

/// Denominators tried when writing a ratio back out; anything else is
/// written as a decimal.
const MAX_DENOMINATOR: u32 = 12;
/// How far a value may be from a fraction and still be written as it. Loose
/// enough to take 33.333% as 1/3, tight enough that no two candidate
/// fractions compete.
const FRACTION_TOLERANCE: f64 = 1e-5;

impl Ratio {
    pub const WHOLE: Ratio = Ratio(1.0);
    pub const HALF: Ratio = Ratio(0.5);

    pub fn new(v: f64) -> Option<Ratio> {
        (v.is_finite() && v > 0.0 && v <= 1.0).then_some(Ratio(v))
    }

    pub fn value(self) -> f64 {
        self.0
    }

    /// `span` is the work area's extent on this axis in physical pixels.
    fn resolve(self, span: f64) -> f64 {
        span * self.0
    }

    /// Reads what an earlier version wrote, which measured in percent
    /// (`"50%"`), as well as a ratio. Pixel lengths have no ratio to become
    /// and are refused.
    pub fn from_config(s: &str) -> Result<Ratio, RatioError> {
        match s.trim().strip_suffix('%') {
            Some(n) => n
                .trim()
                .parse::<f64>()
                .ok()
                .and_then(|p| Ratio::new(p / 100.0))
                .ok_or_else(|| RatioError(s.to_string())),
            None => s.parse(),
        }
    }

    /// The fraction with the smallest denominator that is this value, if
    /// there is one up to [`MAX_DENOMINATOR`].
    fn as_fraction(self) -> Option<(u32, u32)> {
        (1..=MAX_DENOMINATOR).find_map(|d| {
            let n = (self.0 * f64::from(d)).round();
            ((self.0 - n / f64::from(d)).abs() < FRACTION_TOLERANCE).then_some((n as u32, d))
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct RatioError(String);

impl fmt::Display for RatioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "「{}」は画面に対する比率として読めません(0 より大きく 1 以下。例: 1/2、2/3、0.75、1)",
            self.0
        )
    }
}

impl FromStr for Ratio {
    type Err = RatioError;

    /// A fraction (`1/2`) or a decimal (`0.5`, `1`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let number = |t: &str| t.trim().parse::<f64>().ok();
        let value = match s.split_once('/') {
            Some((n, d)) => number(n).zip(number(d)).map(|(n, d)| n / d),
            None => number(s),
        };
        value
            .and_then(Ratio::new)
            .ok_or_else(|| RatioError(s.to_string()))
    }
}

impl fmt::Display for Ratio {
    /// As a fraction when it is one with a small denominator (1/2, 2/3, 1),
    /// otherwise as a decimal of up to four places (0.1429).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_fraction() {
            Some((n, 1)) => write!(f, "{n}"),
            Some((n, d)) => write!(f, "{n}/{d}"),
            None => {
                let s = format!("{:.4}", self.0);
                f.write_str(s.trim_end_matches('0').trim_end_matches('.'))
            }
        }
    }
}

impl TryFrom<String> for Ratio {
    type Error = RatioError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ratio::from_config(&s)
    }
}

impl From<Ratio> for String {
    fn from(r: Ratio) -> String {
        r.to_string()
    }
}

/// Which point of the work area the window is aligned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Anchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Align {
    Start,
    Middle,
    End,
}

impl Anchor {
    pub const ALL: [Anchor; 9] = [
        Anchor::TopLeft,
        Anchor::Top,
        Anchor::TopRight,
        Anchor::Left,
        Anchor::Center,
        Anchor::Right,
        Anchor::BottomLeft,
        Anchor::Bottom,
        Anchor::BottomRight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Anchor::TopLeft => "左上",
            Anchor::Top => "上",
            Anchor::TopRight => "右上",
            Anchor::Left => "左",
            Anchor::Center => "中央",
            Anchor::Right => "右",
            Anchor::BottomLeft => "左下",
            Anchor::Bottom => "下",
            Anchor::BottomRight => "右下",
        }
    }

    fn horizontal(self) -> Align {
        match self {
            Anchor::TopLeft | Anchor::Left | Anchor::BottomLeft => Align::Start,
            Anchor::Top | Anchor::Center | Anchor::Bottom => Align::Middle,
            Anchor::TopRight | Anchor::Right | Anchor::BottomRight => Align::End,
        }
    }

    fn vertical(self) -> Align {
        match self {
            Anchor::TopLeft | Anchor::Top | Anchor::TopRight => Align::Start,
            Anchor::Left | Anchor::Center | Anchor::Right => Align::Middle,
            Anchor::BottomLeft | Anchor::Bottom | Anchor::BottomRight => Align::End,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }

    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
}

/// A window's size and position relative to the work area of the monitor it
/// is on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub anchor: Anchor,
    pub width: Ratio,
    pub height: Ratio,
}

/// Places a span of `size` inside `[start, start + span]` and returns its two
/// edges, unrounded. The span never leaves the area: it is shrunk to fit.
fn place(start: f64, span: f64, size: f64, align: Align) -> (f64, f64) {
    let size = size.clamp(1.0_f64.min(span), span);
    let lead = match align {
        Align::Start => start,
        Align::Middle => start + (span - size) / 2.0,
        Align::End => start + span - size,
    };
    (lead, lead + size)
}

impl Placement {
    /// The rectangle, in physical pixels, that the visible frame of the
    /// window should occupy. Edges are rounded independently, so two
    /// placements that meet at a fraction of a pixel still share an edge.
    pub fn resolve(&self, work: Rect) -> Rect {
        let (ww, wh) = (f64::from(work.width()), f64::from(work.height()));
        let (left, right) = place(
            f64::from(work.left),
            ww,
            self.width.resolve(ww),
            self.anchor.horizontal(),
        );
        let (top, bottom) = place(
            f64::from(work.top),
            wh,
            self.height.resolve(wh),
            self.anchor.vertical(),
        );
        Rect {
            left: left.round() as i32,
            top: top.round() as i32,
            right: right.round() as i32,
            bottom: bottom.round() as i32,
        }
    }
}

/// A work area of `size` drawn as large as it fits inside `bounds` without
/// changing its shape, aligned there to `anchor`. This is the screen a
/// placement is previewed on; resolving the placement against it gives the
/// window. `None` when either has no area.
pub fn miniature(size: (i32, i32), bounds: Rect, anchor: Anchor) -> Option<Rect> {
    let (sw, sh) = (f64::from(size.0), f64::from(size.1));
    let (bw, bh) = (f64::from(bounds.width()), f64::from(bounds.height()));
    if sw <= 0.0 || sh <= 0.0 || bw <= 0.0 || bh <= 0.0 {
        return None;
    }
    let k = (bw / sw).min(bh / sh);
    let (left, right) = place(f64::from(bounds.left), bw, sw * k, anchor.horizontal());
    let (top, bottom) = place(f64::from(bounds.top), bh, sh * k, anchor.vertical());
    Some(Rect {
        left: left.round() as i32,
        top: top.round() as i32,
        right: right.round() as i32,
        bottom: bottom.round() as i32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FHD: Rect = Rect {
        left: 0,
        top: 0,
        right: 1920,
        bottom: 1040,
    };

    fn p(anchor: Anchor, width: &str, height: &str) -> Placement {
        Placement {
            anchor,
            width: width.parse().unwrap(),
            height: height.parse().unwrap(),
        }
    }

    fn r(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn reads_ratios() {
        let r = |v: f64| Ok(Ratio::new(v).unwrap());
        assert_eq!("1/2".parse(), r(0.5));
        assert_eq!(" 2 / 3 ".parse(), r(2.0 / 3.0));
        assert_eq!("0.75".parse(), r(0.75));
        assert_eq!("1".parse(), r(1.0));
        for bad in [
            "", "wide", "1/", "/2", "1/0", "0", "-0.5", "3/2", "1.01", "NaN", "50%", "800px",
        ] {
            assert!(bad.parse::<Ratio>().is_err(), "{bad}");
        }
    }

    #[test]
    fn reads_percent_only_from_the_config() {
        assert_eq!(Ratio::from_config("50%"), Ok(Ratio::new(0.5).unwrap()));
        assert_eq!(Ratio::from_config(" 100 % "), Ok(Ratio::WHOLE));
        assert_eq!(Ratio::from_config("1/4"), Ok(Ratio::new(0.25).unwrap()));
        assert!(Ratio::from_config("150%").is_err());
        assert!(Ratio::from_config("800px").is_err());
    }

    #[test]
    fn writes_fractions_where_it_can() {
        let show = |v: f64| Ratio::new(v).unwrap().to_string();
        assert_eq!(show(0.5), "1/2");
        assert_eq!(show(1.0), "1");
        assert_eq!(show(0.6), "3/5");
        assert_eq!(show(0.33333), "1/3");
        assert_eq!(show(0.66667), "2/3");
        assert_eq!(show(5.0 / 12.0), "5/12");
        assert_eq!(show(1.0 / 7.0), "1/7");
        assert_eq!(show(1.0 / 13.0), "0.0769");
        assert_eq!(show(0.123), "0.123");
    }

    #[test]
    fn halves_and_quarters() {
        assert_eq!(p(Anchor::Left, "1/2", "1").resolve(FHD), r(0, 0, 960, 1040));
        assert_eq!(
            p(Anchor::Right, "1/2", "1").resolve(FHD),
            r(960, 0, 1920, 1040)
        );
        assert_eq!(p(Anchor::Top, "1", "1/2").resolve(FHD), r(0, 0, 1920, 520));
        assert_eq!(
            p(Anchor::BottomRight, "1/2", "1/2").resolve(FHD),
            r(960, 520, 1920, 1040)
        );
    }

    #[test]
    fn neighbours_share_an_edge_on_odd_sizes() {
        let work = r(0, 0, 1921, 1041);
        let thirds =
            [Anchor::Left, Anchor::Center, Anchor::Right].map(|a| p(a, "1/3", "1").resolve(work));
        assert_eq!(thirds[0].left, 0);
        assert_eq!(thirds[0].right, thirds[1].left);
        assert_eq!(thirds[1].right, thirds[2].left);
        assert_eq!(thirds[2].right, 1921);
    }

    #[test]
    fn follows_the_work_area_of_other_monitors() {
        // A monitor to the left of the primary one, below its top edge.
        let work = r(-2560, 200, 0, 1600);
        assert_eq!(
            p(Anchor::Right, "1/2", "1").resolve(work),
            r(-1280, 200, 0, 1600)
        );
    }

    #[test]
    fn keeps_at_least_a_pixel() {
        assert_eq!(p(Anchor::Center, "0.0001", "1").resolve(FHD).width(), 1);
    }

    #[test]
    fn miniatures_keep_the_screen_shape() {
        // A 16:9 screen in a wide box is limited by the height.
        let wide = r(10, 20, 110, 38);
        assert_eq!(
            miniature((1920, 1080), wide, Anchor::TopLeft),
            Some(r(10, 20, 42, 38))
        );
        assert_eq!(
            miniature((1920, 1080), wide, Anchor::Center),
            Some(r(44, 20, 76, 38))
        );
        // In a tall box, by the width.
        let tall = r(0, 0, 32, 100);
        assert_eq!(
            miniature((1920, 1080), tall, Anchor::TopLeft),
            Some(r(0, 0, 32, 18))
        );
        assert_eq!(
            miniature((1080, 1920), tall, Anchor::Center),
            Some(r(0, 22, 32, 78))
        );
        assert_eq!(miniature((0, 1080), wide, Anchor::Center), None);
        assert_eq!(
            miniature((1920, 1080), r(5, 5, 5, 40), Anchor::Center),
            None
        );
    }

    #[test]
    fn placements_resolve_on_a_miniature() {
        let screen = miniature((1920, 1080), r(0, 0, 32, 18), Anchor::TopLeft).unwrap();
        assert_eq!(p(Anchor::Left, "1/2", "1").resolve(screen), r(0, 0, 16, 18));
        assert_eq!(
            p(Anchor::BottomRight, "1/2", "1/2").resolve(screen),
            r(16, 9, 32, 18)
        );
        // However small, the window stays visible.
        assert_eq!(p(Anchor::Center, "0.01", "1").resolve(screen).width(), 1);
    }
}
