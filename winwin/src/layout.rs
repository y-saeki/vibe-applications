//! Where a shortcut puts a window, and the arithmetic that turns that into a
//! rectangle on a given monitor.
//!
//! Nothing here touches Win32, so it is tested on every platform.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// A length along one axis of the monitor's work area: a share of it, or a
/// fixed size in pixels at 100% display scaling.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Length {
    Percent(f64),
    Pixels(f64),
}

impl Length {
    /// `span` is the work area's extent on this axis in physical pixels and
    /// `scale` the monitor's display scaling (1.0 at 96 DPI).
    fn resolve(self, span: f64, scale: f64) -> f64 {
        match self {
            Length::Percent(p) => span * p / 100.0,
            Length::Pixels(px) => px * scale,
        }
    }

    pub fn value(self) -> f64 {
        match self {
            Length::Percent(v) | Length::Pixels(v) => v,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct LengthError(String);

impl fmt::Display for LengthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "「{}」は長さとして読めません(例: 50%、800px)", self.0)
    }
}

impl FromStr for Length {
    type Err = LengthError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        let (number, make): (&str, fn(f64) -> Length) = if let Some(n) = t.strip_suffix('%') {
            (n, Length::Percent)
        } else if let Some(n) = t.strip_suffix("px") {
            (n, Length::Pixels)
        } else {
            (t, Length::Pixels)
        };
        number
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
            .map(make)
            .ok_or_else(|| LengthError(s.to_string()))
    }
}

/// Up to three decimals, without trailing zeros: 33.333%, 50%, 800px.
fn format_number(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

impl fmt::Display for Length {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Length::Percent(v) => write!(f, "{}%", format_number(*v)),
            Length::Pixels(v) => write!(f, "{}px", format_number(*v)),
        }
    }
}

impl TryFrom<String> for Length {
    type Error = LengthError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Length> for String {
    fn from(l: Length) -> String {
        l.to_string()
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
    pub width: Length,
    pub height: Length,
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
    pub fn resolve(&self, work: Rect, scale: f64) -> Rect {
        let (ww, wh) = (f64::from(work.width()), f64::from(work.height()));
        let (left, right) = place(
            f64::from(work.left),
            ww,
            self.width.resolve(ww, scale),
            self.anchor.horizontal(),
        );
        let (top, bottom) = place(
            f64::from(work.top),
            wh,
            self.height.resolve(wh, scale),
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
    fn reads_and_writes_lengths() {
        assert_eq!("50%".parse(), Ok(Length::Percent(50.0)));
        assert_eq!(" 33.3333 % ".parse(), Ok(Length::Percent(33.3333)));
        assert_eq!("800px".parse(), Ok(Length::Pixels(800.0)));
        assert_eq!("800".parse(), Ok(Length::Pixels(800.0)));
        assert_eq!("-20px".parse(), Ok(Length::Pixels(-20.0)));
        assert!("wide".parse::<Length>().is_err());
        assert!("%".parse::<Length>().is_err());
        assert!("NaN%".parse::<Length>().is_err());
        assert_eq!(Length::Percent(100.0 / 3.0).to_string(), "33.333%");
        assert_eq!(Length::Percent(50.0).to_string(), "50%");
        assert_eq!(Length::Pixels(800.0).to_string(), "800px");
        assert_eq!(Length::Pixels(-0.0001).to_string(), "0px");
    }

    #[test]
    fn halves_and_quarters() {
        assert_eq!(
            p(Anchor::Left, "50%", "100%").resolve(FHD, 1.0),
            r(0, 0, 960, 1040)
        );
        assert_eq!(
            p(Anchor::Right, "50%", "100%").resolve(FHD, 1.0),
            r(960, 0, 1920, 1040)
        );
        assert_eq!(
            p(Anchor::Top, "100%", "50%").resolve(FHD, 1.0),
            r(0, 0, 1920, 520)
        );
        assert_eq!(
            p(Anchor::BottomRight, "50%", "50%").resolve(FHD, 1.0),
            r(960, 520, 1920, 1040)
        );
    }

    #[test]
    fn neighbours_share_an_edge_on_odd_sizes() {
        let work = r(0, 0, 1921, 1041);
        let thirds = [Anchor::Left, Anchor::Center, Anchor::Right]
            .map(|a| p(a, "33.3333333%", "100%").resolve(work, 1.0));
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
            p(Anchor::Right, "50%", "100%").resolve(work, 1.0),
            r(-1280, 200, 0, 1600)
        );
    }

    #[test]
    fn pixels_follow_display_scaling() {
        let work = r(0, 0, 3840, 2100);
        assert_eq!(
            p(Anchor::Center, "1000px", "600px").resolve(work, 1.5),
            r(1170, 600, 2670, 1500)
        );
    }

    #[test]
    fn never_leaves_the_work_area() {
        assert_eq!(p(Anchor::Center, "3000px", "150%").resolve(FHD, 1.0), FHD);
        assert_eq!(p(Anchor::Center, "0px", "-5%").resolve(FHD, 1.0).width(), 1);
    }
}
