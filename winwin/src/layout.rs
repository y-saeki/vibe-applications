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
                .ok_or(RatioError),
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
pub struct RatioError;

impl fmt::Display for RatioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("画面に対する比率を入力してください（例: 2/3, 0.75, 1）")
    }
}

/// A ratio the config file could not read. Unlike [`RatioError`], which the
/// settings window shows next to the field, it names the value, since the
/// parser points only at the `[[shortcut]]` table it is in.
#[derive(Debug, PartialEq, Eq)]
pub struct FileRatioError(String);

impl fmt::Display for FileRatioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "「{}」は画面に対する比率として読めません（例: 2/3, 0.75, 1）",
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
        value.and_then(Ratio::new).ok_or(RatioError)
    }
}

impl fmt::Display for Ratio {
    /// As a fraction when it is one with a small denominator (1/2, 2/3, 1),
    /// otherwise rounded to a decimal of up to three places (0.077). One too
    /// small to show in three places is written as the smallest that is
    /// (0.001), so that the file still reads.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_fraction() {
            Some((n, 1)) => write!(f, "{n}"),
            Some((n, d)) => write!(f, "{n}/{d}"),
            None => {
                let s = format!("{:.3}", self.0.max(0.001));
                f.write_str(s.trim_end_matches('0').trim_end_matches('.'))
            }
        }
    }
}

impl TryFrom<String> for Ratio {
    type Error = FileRatioError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ratio::from_config(&s).map_err(|_| FileRatioError(s))
    }
}

impl From<Ratio> for String {
    fn from(r: Ratio) -> String {
        r.to_string()
    }
}

/// A width or height: a share of the work area, or the window's own length
/// on that axis as it is when the shortcut is pressed.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum Size {
    Ratio(Ratio),
    Keep,
}

/// How the file spells [`Size::Keep`].
const KEEP: &str = "keep";

impl Size {
    /// `span` is the work area's extent on this axis and `current` the
    /// window's, both in physical pixels.
    fn resolve(self, span: f64, current: f64) -> f64 {
        match self {
            Size::Ratio(r) => span * r.value(),
            Size::Keep => current,
        }
    }
}

impl From<Ratio> for Size {
    fn from(r: Ratio) -> Size {
        Size::Ratio(r)
    }
}

impl TryFrom<String> for Size {
    type Error = FileRatioError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if s.trim().eq_ignore_ascii_case(KEEP) {
            Ok(Size::Keep)
        } else {
            Ratio::try_from(s).map(Size::Ratio)
        }
    }
}

impl fmt::Display for Size {
    /// As the file spells it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Size::Ratio(r) => r.fmt(f),
            Size::Keep => f.write_str(KEEP),
        }
    }
}

impl From<Size> for String {
    fn from(s: Size) -> String {
        s.to_string()
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

    /// Whether a placement that fits can be anchored here: anywhere but the
    /// center, which has no edge for the window to reach out from.
    pub fn can_fit(self) -> bool {
        self != Anchor::Center
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

    /// Where the two overlap, if they do.
    fn intersection(&self, other: Rect) -> Option<Rect> {
        let r = Rect {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (r.left < r.right && r.top < r.bottom).then_some(r)
    }
}

/// A window's size and position relative to the work area of the monitor it
/// is on.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PlacementFields", into = "PlacementFields")]
pub struct Placement {
    pub anchor: Anchor,
    pub sizing: Sizing,
}

/// How large a placement makes the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    /// A width and a height of its own.
    Given { width: Size, height: Size },
    /// As large as the empty space at the anchor: see [`fit`].
    Fit,
}

/// A placement as the file writes it: a width and a height, or `fit = true`
/// in their place.
#[derive(Serialize, Deserialize)]
struct PlacementFields {
    anchor: Anchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<Size>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<Size>,
    #[serde(default, skip_serializing_if = "is_false")]
    fit: bool,
}

fn is_false(b: &bool) -> bool {
    !b
}

/// A placement in the file that does not make one.
#[derive(Debug, PartialEq, Eq)]
pub enum PlacementError {
    /// Neither a size nor `fit`.
    NoSize,
    /// `fit` at the center.
    FitAtCenter,
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PlacementError::NoSize => "width と height を書くか、fit = true にしてください",
            PlacementError::FitAtCenter => FIT_AT_CENTER,
        })
    }
}

/// Why a placement that fits cannot be at the center.
pub const FIT_AT_CENTER: &str = "フィットでは基準位置に中央を選べません";

impl TryFrom<PlacementFields> for Placement {
    type Error = PlacementError;

    /// With `fit`, a width or height written as well is ignored.
    fn try_from(p: PlacementFields) -> Result<Self, Self::Error> {
        let sizing = match (p.fit, p.width, p.height) {
            (true, _, _) if !p.anchor.can_fit() => return Err(PlacementError::FitAtCenter),
            (true, _, _) => Sizing::Fit,
            (false, Some(width), Some(height)) => Sizing::Given { width, height },
            _ => return Err(PlacementError::NoSize),
        };
        Ok(Placement {
            anchor: p.anchor,
            sizing,
        })
    }
}

impl From<Placement> for PlacementFields {
    fn from(p: Placement) -> PlacementFields {
        let (width, height) = p.sizes().unzip();
        PlacementFields {
            anchor: p.anchor,
            width,
            height,
            fit: p.sizing == Sizing::Fit,
        }
    }
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
    pub const fn given(anchor: Anchor, width: Size, height: Size) -> Placement {
        Placement {
            anchor,
            sizing: Sizing::Given { width, height },
        }
    }

    /// The width and the height, unless the placement fits.
    pub fn sizes(&self) -> Option<(Size, Size)> {
        match self.sizing {
            Sizing::Given { width, height } => Some((width, height)),
            Sizing::Fit => None,
        }
    }

    /// The rectangle, in physical pixels, that the visible frame of the
    /// window should occupy, or `None` when a placement that fits finds no
    /// room. `current` is the size of the window's visible frame now, which a
    /// [`Size::Keep`] carries over; `others` the visible frames of the other
    /// windows, which only a placement that fits asks for (see [`fit`]).
    pub fn resolve(
        &self,
        work: Rect,
        current: (i32, i32),
        others: impl FnOnce() -> Vec<Rect>,
    ) -> Option<Rect> {
        match self.sizing {
            Sizing::Given { width, height } => {
                Some(sized(self.anchor, width, height, work, current))
            }
            Sizing::Fit => fit(work, self.anchor, &others()),
        }
    }
}

/// The rectangle of a window `width` by `height` aligned to `anchor` in
/// `work`, in physical pixels. Edges are rounded independently, so two
/// placements that meet at a fraction of a pixel still share an edge.
/// `current` is the size of the window's visible frame now, which a
/// [`Size::Keep`] carries over.
pub fn sized(anchor: Anchor, width: Size, height: Size, work: Rect, current: (i32, i32)) -> Rect {
    let (ww, wh) = (f64::from(work.width()), f64::from(work.height()));
    let (left, right) = place(
        f64::from(work.left),
        ww,
        width.resolve(ww, f64::from(current.0)),
        anchor.horizontal(),
    );
    let (top, bottom) = place(
        f64::from(work.top),
        wh,
        height.resolve(wh, f64::from(current.1)),
        anchor.vertical(),
    );
    Rect {
        left: left.round() as i32,
        top: top.round() as i32,
        right: right.round() as i32,
        bottom: bottom.round() as i32,
    }
}

/// Where a window fits at `anchor`: it reaches out from the anchor's edges
/// up to the frontmost window. `others` are the visible frames of the other
/// windows from the top of the z-order down; the frontmost is the first that
/// lies on `work` at all, and the windows behind it do not count.
///
/// Along each axis the anchor has an edge on, the window stops at the near
/// side of the front window. Where the front window reaches that edge
/// itself, the window lies beside it instead, as long as it is along that
/// axis. Along an axis the anchor has no edge on, it takes the whole work
/// area. So a window at the left is as tall as the work area, and one at the
/// top left of a window in the bottom right two thirds by two thirds is a
/// third by a third, while one at the bottom left is a third by two thirds.
/// `None` when the window would lie beside the front window every way, as
/// it does when the front window takes the anchor's edge or its corner, and
/// for the center, which has no edge to reach out from.
pub fn fit(work: Rect, anchor: Anchor, others: &[Rect]) -> Option<Rect> {
    let (horizontal, vertical) = (anchor.horizontal(), anchor.vertical());
    if horizontal == Align::Middle && vertical == Align::Middle {
        return None;
    }
    let Some(front) = others.iter().find_map(|o| o.intersection(work)) else {
        return Some(work);
    };
    // The span along one axis, and whether it stops short of the front
    // window rather than lying beside it.
    let span = |start: i32, end: i32, near: i32, far: i32, align: Align| match align {
        Align::Start if near > start => ((start, near), true),
        Align::Start => ((start, far), false),
        Align::End if far < end => ((far, end), true),
        Align::End => ((near, end), false),
        Align::Middle => ((start, end), false),
    };
    let ((left, right), short_across) =
        span(work.left, work.right, front.left, front.right, horizontal);
    let ((top, bottom), short_down) =
        span(work.top, work.bottom, front.top, front.bottom, vertical);
    (short_across || short_down).then_some(Rect {
        left,
        top,
        right,
        bottom,
    })
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
        let size = |s: &str| Size::try_from(s.to_string()).unwrap();
        Placement::given(anchor, size(width), size(height))
    }

    impl Placement {
        /// Where a placement that does not fit puts a window.
        fn rect(&self, work: Rect, current: (i32, i32)) -> Rect {
            self.resolve(work, current, || unreachable!()).unwrap()
        }
    }

    /// A window's size, for placements that do not keep either.
    const ANY: (i32, i32) = (0, 0);

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
        assert_eq!(show(1.0 / 13.0), "0.077");
        assert_eq!(show(0.123), "0.123");
        assert_eq!(show(0.12351), "0.124");
        assert_eq!(show(0.0001), "0.001");
        assert_eq!(show(0.0001).parse::<Ratio>().map(Ratio::value), Ok(0.001));
    }

    #[test]
    fn reads_and_writes_keep() {
        assert_eq!(Size::try_from(" Keep ".to_string()), Ok(Size::Keep));
        assert_eq!(String::from(Size::Keep), "keep");
        assert_eq!(
            Size::try_from("50%".to_string()),
            Ok(Size::Ratio(Ratio::HALF))
        );
        assert!(Size::try_from("kept".to_string()).is_err());
    }

    #[test]
    fn keeps_the_current_width_or_height() {
        let current = (800, 600);
        // The width stays, the height becomes a third, from the top left.
        assert_eq!(
            p(Anchor::TopLeft, "keep", "1/3").rect(FHD, current),
            r(0, 0, 800, 347)
        );
        assert_eq!(
            p(Anchor::Center, "1/2", "keep").rect(FHD, current),
            r(480, 220, 1440, 820)
        );
        assert_eq!(
            p(Anchor::BottomRight, "keep", "keep").rect(FHD, current),
            r(1120, 440, 1920, 1040)
        );
        // A window larger than the work area is shrunk to fit it.
        assert_eq!(
            p(Anchor::Left, "keep", "1").rect(FHD, (2400, 600)),
            r(0, 0, 1920, 1040)
        );
    }

    #[test]
    fn halves_and_quarters() {
        assert_eq!(
            p(Anchor::Left, "1/2", "1").rect(FHD, ANY),
            r(0, 0, 960, 1040)
        );
        assert_eq!(
            p(Anchor::Right, "1/2", "1").rect(FHD, ANY),
            r(960, 0, 1920, 1040)
        );
        assert_eq!(
            p(Anchor::Top, "1", "1/2").rect(FHD, ANY),
            r(0, 0, 1920, 520)
        );
        assert_eq!(
            p(Anchor::BottomRight, "1/2", "1/2").rect(FHD, ANY),
            r(960, 520, 1920, 1040)
        );
    }

    #[test]
    fn neighbours_share_an_edge_on_odd_sizes() {
        let work = r(0, 0, 1921, 1041);
        let thirds =
            [Anchor::Left, Anchor::Center, Anchor::Right].map(|a| p(a, "1/3", "1").rect(work, ANY));
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
            p(Anchor::Right, "1/2", "1").rect(work, ANY),
            r(-1280, 200, 0, 1600)
        );
    }

    #[test]
    fn keeps_at_least_a_pixel() {
        assert_eq!(p(Anchor::Center, "0.0001", "1").rect(FHD, ANY).width(), 1);
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
        assert_eq!(
            p(Anchor::Left, "1/2", "1").rect(screen, ANY),
            r(0, 0, 16, 18)
        );
        assert_eq!(
            p(Anchor::BottomRight, "1/2", "1/2").rect(screen, ANY),
            r(16, 9, 32, 18)
        );
        // However small, the window stays visible.
        assert_eq!(p(Anchor::Center, "0.01", "1").rect(screen, ANY).width(), 1);
    }

    fn fits(anchor: Anchor, others: &[Rect]) -> Option<Rect> {
        let placement = Placement {
            anchor,
            sizing: Sizing::Fit,
        };
        placement.resolve(FHD, ANY, || others.to_vec())
    }

    #[test]
    fn fits_the_whole_work_area_when_it_is_empty() {
        for anchor in Anchor::ALL.into_iter().filter(|a| a.can_fit()) {
            assert_eq!(fits(anchor, &[]), Some(FHD), "{anchor:?}");
        }
        assert_eq!(fits(Anchor::Center, &[]), None);
    }

    #[test]
    fn only_the_frontmost_window_counts() {
        // The right half in front of the top left two thirds by a half.
        let others = [r(960, 0, 1920, 1040), r(0, 0, 1280, 520)];
        assert_eq!(fits(Anchor::Left, &others), Some(r(0, 0, 960, 1040)));
        // The front window reaches the right edge.
        assert_eq!(fits(Anchor::Right, &others), None);
        assert_eq!(fits(Anchor::TopRight, &others), None);
    }

    #[test]
    fn sides_take_the_whole_height_or_width() {
        // A small window in the middle.
        let middle = [r(640, 260, 1280, 780)];
        assert_eq!(fits(Anchor::Left, &middle), Some(r(0, 0, 640, 1040)));
        assert_eq!(fits(Anchor::Right, &middle), Some(r(1280, 0, 1920, 1040)));
        assert_eq!(fits(Anchor::Top, &middle), Some(r(0, 0, 1920, 260)));
        assert_eq!(fits(Anchor::Bottom, &middle), Some(r(0, 780, 1920, 1040)));
    }

    #[test]
    fn corners_stop_at_the_front_window_or_lie_beside_it() {
        // The bottom right two thirds by two thirds.
        let others = [r(640, 347, 1920, 1040)];
        assert_eq!(fits(Anchor::TopLeft, &others), Some(r(0, 0, 640, 347)));
        // The corner itself is taken.
        assert_eq!(fits(Anchor::BottomRight, &others), None);
        // Where the window reaches an edge, as long as it is that way.
        assert_eq!(
            fits(Anchor::BottomLeft, &others),
            Some(r(0, 347, 640, 1040))
        );
        assert_eq!(fits(Anchor::TopRight, &others), Some(r(640, 0, 1920, 347)));
        // As long as the work area, where the window reaches both edges.
        let right_half = [r(960, 0, 1920, 1040)];
        assert_eq!(fits(Anchor::TopLeft, &right_half), Some(r(0, 0, 960, 1040)));
        // A band along the top takes both top corners.
        let band = [r(0, 0, 1920, 300)];
        assert_eq!(fits(Anchor::TopLeft, &band), None);
        assert_eq!(fits(Anchor::BottomLeft, &band), Some(r(0, 300, 1920, 1040)));
    }

    #[test]
    fn a_window_over_the_whole_work_area_leaves_no_room() {
        // A maximized window's frame may reach past the work area.
        let others = [r(-8, -8, 1928, 1048), r(0, 0, 960, 1040)];
        assert_eq!(fits(Anchor::Right, &others), None);
    }

    #[test]
    fn windows_on_other_monitors_do_not_count() {
        // In front, but on the monitor to the left.
        let others = [r(-2560, 0, 0, 1440), r(-500, 0, 480, 1040)];
        assert_eq!(fits(Anchor::Right, &others), Some(r(480, 0, 1920, 1040)));
    }
}
