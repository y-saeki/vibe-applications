//! Gestures: the strokes of the pointer while the right button is held,
//! each one of four directions. [`Tracker`] follows the pointer from the
//! button going down and says which gesture it drew; [`Gesture`] is what
//! the config file writes ("↓→").
//!
//! Nothing here touches the operating system, so it is tested on every
//! platform.

use std::fmt;
use std::str::FromStr;

/// The most strokes a gesture has. One drawn with more is no gesture.
pub const MAX_STROKES: usize = 6;

/// How far the pointer has to move from where the button went down before
/// the press is a gesture rather than a click, in points (DIPs on Windows).
pub const START_DISTANCE: f64 = 10.0;

/// How far the pointer has to move in one direction to make a stroke, in
/// points.
pub const STROKE_DISTANCE: f64 = 24.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl Direction {
    pub const ALL: [Direction; 4] = [
        Direction::Left,
        Direction::Up,
        Direction::Right,
        Direction::Down,
    ];

    pub fn arrow(self) -> char {
        match self {
            Direction::Up => '↑',
            Direction::Down => '↓',
            Direction::Left => '←',
            Direction::Right => '→',
        }
    }

    /// The arrow or the letter of the direction: `U`, `D`, `L` or `R`.
    fn from_char(c: char) -> Option<Direction> {
        match c {
            '↑' | 'U' | 'u' => Some(Direction::Up),
            '↓' | 'D' | 'd' => Some(Direction::Down),
            '←' | 'L' | 'l' => Some(Direction::Left),
            '→' | 'R' | 'r' => Some(Direction::Right),
            _ => None,
        }
    }

    /// The direction a move of (`dx`, `dy`) goes in most, y counting down.
    fn of(dx: f64, dy: f64) -> Direction {
        if dx.abs() >= dy.abs() {
            if dx < 0.0 {
                Direction::Left
            } else {
                Direction::Right
            }
        } else if dy < 0.0 {
            Direction::Up
        } else {
            Direction::Down
        }
    }
}

/// The strokes of a gesture, in order. Two strokes in a row never go the
/// same way: a longer stroke is still one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Gesture(Vec<Direction>);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GestureError {
    Empty,
    Unknown(char),
    Repeated,
    TooLong,
}

impl fmt::Display for GestureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GestureError::Empty => write!(f, "ジェスチャーが空です"),
            GestureError::Unknown(c) => write!(f, "「{c}」は方向として認識できません"),
            GestureError::Repeated => write!(f, "同じ方向を続けることはできません"),
            GestureError::TooLong => write!(f, "ジェスチャーは {MAX_STROKES} 方向までです"),
        }
    }
}

impl Gesture {
    pub fn new(strokes: Vec<Direction>) -> Result<Gesture, GestureError> {
        if strokes.is_empty() {
            return Err(GestureError::Empty);
        }
        if strokes.len() > MAX_STROKES {
            return Err(GestureError::TooLong);
        }
        if strokes.windows(2).any(|w| w[0] == w[1]) {
            return Err(GestureError::Repeated);
        }
        Ok(Gesture(strokes))
    }

    pub fn strokes(&self) -> &[Direction] {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds a stroke at the end, unless it repeats the last one or the
    /// gesture is full. Whether it was added.
    pub fn push(&mut self, d: Direction) -> bool {
        if self.0.last() == Some(&d) || self.0.len() >= MAX_STROKES {
            return false;
        }
        self.0.push(d);
        true
    }

    pub fn pop(&mut self) {
        self.0.pop();
    }
}

impl fmt::Display for Gesture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|d| write!(f, "{}", d.arrow()))
    }
}

impl FromStr for Gesture {
    type Err = GestureError;

    fn from_str(s: &str) -> Result<Gesture, GestureError> {
        let strokes = s
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(|c| Direction::from_char(c).ok_or(GestureError::Unknown(c)))
            .collect::<Result<Vec<_>, _>>()?;
        Gesture::new(strokes)
    }
}

/// Follows the pointer from the right button going down to it coming up.
/// Positions are in whatever units the platform gives, `scale` of them to a
/// point; y counts down.
#[derive(Clone, Debug)]
pub struct Tracker {
    start: (f64, f64),
    /// Where the stroke being drawn started.
    anchor: (f64, f64),
    scale: f64,
    strokes: Vec<Direction>,
    started: bool,
    /// More strokes than a gesture has were drawn.
    overflowed: bool,
}

impl Tracker {
    pub fn new(at: (f64, f64), scale: f64) -> Tracker {
        Tracker {
            start: at,
            anchor: at,
            scale: if scale > 0.0 { scale } else { 1.0 },
            strokes: Vec::new(),
            started: false,
            overflowed: false,
        }
    }

    /// The pointer moved to `to`. True when that added a stroke.
    pub fn move_to(&mut self, to: (f64, f64)) -> bool {
        let far = |from: (f64, f64), distance: f64| {
            let (dx, dy) = (to.0 - from.0, to.1 - from.1);
            dx.abs().max(dy.abs()) >= distance * self.scale
        };
        if !self.started && far(self.start, START_DISTANCE) {
            self.started = true;
        }
        if !far(self.anchor, STROKE_DISTANCE) {
            return false;
        }
        let d = Direction::of(to.0 - self.anchor.0, to.1 - self.anchor.1);
        self.anchor = to;
        if self.strokes.last() == Some(&d) {
            return false;
        }
        if self.strokes.len() >= MAX_STROKES {
            self.overflowed = true;
            return false;
        }
        self.strokes.push(d);
        true
    }

    /// Whether the pointer has moved far enough for the press to be a
    /// gesture. A press that is not one is a click, given back to the
    /// application when the button comes up.
    pub fn is_gesture(&self) -> bool {
        self.started
    }

    /// The gesture drawn so far: `None` while no stroke has been, and for
    /// one drawn with too many.
    pub fn gesture(&self) -> Option<Gesture> {
        if self.overflowed {
            return None;
        }
        Gesture::new(self.strokes.clone()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Direction::*;

    fn g(s: &str) -> Gesture {
        s.parse().unwrap()
    }

    /// Draws a path through `points` in small steps, as the pointer reports
    /// it.
    fn draw(scale: f64, points: &[(f64, f64)]) -> Tracker {
        let mut t = Tracker::new(points[0], scale);
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            for i in 1..=10 {
                let f = f64::from(i) / 10.0;
                t.move_to((a.0 + (b.0 - a.0) * f, a.1 + (b.1 - a.1) * f));
            }
        }
        t
    }

    #[test]
    fn reads_and_writes_arrows() {
        assert_eq!(g("↓→").strokes(), [Down, Right]);
        assert_eq!(g("↓→").to_string(), "↓→");
        assert_eq!(g("DR"), g("↓→"));
        assert_eq!(g(" ← ↑ "), g("LU"));
        assert_eq!("".parse::<Gesture>(), Err(GestureError::Empty));
        assert_eq!("↓↓".parse::<Gesture>(), Err(GestureError::Repeated));
        assert_eq!("↓x".parse::<Gesture>(), Err(GestureError::Unknown('x')));
        assert_eq!("LRLRLRL".parse::<Gesture>(), Err(GestureError::TooLong));
    }

    #[test]
    fn push_keeps_a_gesture_valid() {
        let mut gesture = Gesture::default();
        assert!(gesture.push(Down));
        assert!(!gesture.push(Down));
        assert!(gesture.push(Right));
        assert_eq!(gesture, g("↓→"));
        for d in [Left, Right, Left, Right] {
            gesture.push(d);
        }
        assert!(!gesture.push(Up));
        assert_eq!(gesture.strokes().len(), MAX_STROKES);
        gesture.pop();
        assert_eq!(gesture, g("↓→←→←"));
    }

    #[test]
    fn a_small_move_is_a_click() {
        let t = draw(1.0, &[(100.0, 100.0), (105.0, 103.0)]);
        assert!(!t.is_gesture());
        assert_eq!(t.gesture(), None);
    }

    #[test]
    fn follows_strokes_in_four_directions() {
        let t = draw(1.0, &[(100.0, 100.0), (100.0, 200.0), (200.0, 200.0)]);
        assert!(t.is_gesture());
        assert_eq!(t.gesture(), Some(g("↓→")));
        let t = draw(1.0, &[(0.0, 0.0), (-80.0, 0.0), (-80.0, -80.0)]);
        assert_eq!(t.gesture(), Some(g("←↑")));
    }

    #[test]
    fn a_wobbly_stroke_is_one_stroke() {
        let t = draw(
            1.0,
            &[(0.0, 0.0), (60.0, 8.0), (120.0, -6.0), (180.0, 10.0)],
        );
        assert_eq!(t.gesture(), Some(g("→")));
    }

    #[test]
    fn a_move_too_short_for_a_stroke_draws_none() {
        // Far enough to be a gesture, not to make a stroke.
        let t = draw(1.0, &[(0.0, 0.0), (15.0, 0.0)]);
        assert!(t.is_gesture());
        assert_eq!(t.gesture(), None);
    }

    #[test]
    fn distances_follow_the_scale() {
        // At twice the pixels per point, 30 pixels is 15 points.
        let t = draw(2.0, &[(0.0, 0.0), (30.0, 0.0)]);
        assert!(t.is_gesture());
        assert_eq!(t.gesture(), None);
        let t = draw(2.0, &[(0.0, 0.0), (60.0, 0.0)]);
        assert_eq!(t.gesture(), Some(g("→")));
    }

    #[test]
    fn too_many_strokes_is_no_gesture() {
        let mut points = vec![(0.0, 0.0)];
        for i in 1..=MAX_STROKES + 1 {
            points.push(((i % 2) as f64 * 100.0, 0.0));
        }
        let t = draw(1.0, &points);
        assert!(t.is_gesture());
        assert_eq!(t.gesture(), None);
    }
}
