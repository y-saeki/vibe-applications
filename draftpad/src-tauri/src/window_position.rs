//! Where the main window goes: back where it was left when draftpad starts,
//! and into the middle of the display under the pointer when the user asks for
//! it to be put back (the Windows jump list task, the macOS Window menu).
//!
//! Positions are kept in the platform's own screen coordinates, the ones that
//! stay put when the window crosses onto a display of another scale: points on
//! macOS, pixels everywhere else. Tauri reports everything in pixels, and on
//! macOS it converts with whichever scale the window or display happens to be
//! on, so every value is taken back to points here before it is compared or
//! stored. `src/main.ts` records the position in the same coordinates.

use tauri::{LogicalPosition, Monitor, PhysicalPosition, PhysicalRect, Position, WebviewWindow};

/// How much of the top of the window has to be on a display's usable area for
/// a saved position to be restored: enough of the title bar to take hold of
/// and drag the window back. Points on macOS, pixels elsewhere.
const GRIP_WIDTH: f64 = 100.0;
const GRIP_HEIGHT: f64 = 24.0;

/// A rectangle in the platform's own screen coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn right(&self) -> f64 {
        self.x + self.width
    }

    fn bottom(&self) -> f64 {
        self.y + self.height
    }

    fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }
}

/// What one unit of the platform's own coordinates is in pixels, at `scale`.
fn units(scale: f64) -> f64 {
    if cfg!(target_os = "macos") {
        scale
    } else {
        1.0
    }
}

fn to_native(rect: &PhysicalRect<i32, u32>, scale: f64) -> Rect {
    let unit = units(scale);
    Rect {
        x: f64::from(rect.position.x) / unit,
        y: f64::from(rect.position.y) / unit,
        width: f64::from(rect.size.width) / unit,
        height: f64::from(rect.size.height) / unit,
    }
}

fn work_area(monitor: &Monitor) -> Rect {
    to_native(monitor.work_area(), monitor.scale_factor())
}

fn display(monitor: &Monitor) -> Rect {
    let rect = PhysicalRect {
        position: *monitor.position(),
        size: *monitor.size(),
    };
    to_native(&rect, monitor.scale_factor())
}

fn native_position(x: f64, y: f64) -> Position {
    if cfg!(target_os = "macos") {
        LogicalPosition::new(x, y).into()
    } else {
        PhysicalPosition::new(x.round() as i32, y.round() as i32).into()
    }
}

/// The window's outer size in the platform's own coordinates.
fn outer_size(window: &WebviewWindow) -> tauri::Result<(f64, f64)> {
    let size = window.outer_size()?;
    let unit = units(window.scale_factor()?);
    Ok((f64::from(size.width) / unit, f64::from(size.height) / unit))
}

/// Whether a window whose outer top-left corner is at `(x, y)` and which is
/// `width` wide leaves enough of its title bar on one of `areas` to be dragged.
pub fn is_reachable(x: f64, y: f64, width: f64, areas: &[Rect]) -> bool {
    areas.iter().any(|area| {
        let left = x.max(area.x);
        let right = (x + width).min(area.right());
        let top = y.max(area.y);
        let bottom = (y + GRIP_HEIGHT).min(area.bottom());
        right - left >= GRIP_WIDTH.min(width) && bottom - top >= GRIP_HEIGHT
    })
}

/// Where a window of `width` × `height` goes to sit in the middle of `area`.
/// One larger than the area keeps its top-left corner on it, so that the title
/// bar is never the part left outside.
pub fn centered(width: f64, height: f64, area: &Rect) -> (f64, f64) {
    let x = area.x + (area.width - width) / 2.0;
    let y = area.y + (area.height - height) / 2.0;
    (x.max(area.x), y.max(area.y))
}

/// Moves the still hidden window back to where it was left, if that is still
/// somewhere it can be reached; otherwise centers it, as a first launch does.
pub fn restore(window: &WebviewWindow, x: Option<f64>, y: Option<f64>) {
    if let (Some(x), Some(y)) = (x, y) {
        if x.is_finite() && y.is_finite() && restore_at(window, x, y).unwrap_or(false) {
            return;
        }
    }
    if let Err(err) = window.center() {
        eprintln!("draftpad: failed to center window: {err}");
    }
}

fn restore_at(window: &WebviewWindow, x: f64, y: f64) -> tauri::Result<bool> {
    let areas: Vec<Rect> = window.available_monitors()?.iter().map(work_area).collect();
    let (width, _) = outer_size(window)?;
    if !is_reachable(x, y, width, &areas) {
        return Ok(false);
    }
    window.set_position(native_position(x, y))?;
    Ok(true)
}

/// Puts the window in the middle of the display the pointer is on — the one
/// the user is looking at, having just clicked the menu or the taskbar there.
pub fn reset(window: &WebviewWindow) {
    if let Err(err) = reset_to_pointer(window) {
        eprintln!("draftpad: failed to reset the window position: {err}");
    }
}

fn reset_to_pointer(window: &WebviewWindow) -> tauri::Result<()> {
    if window.is_minimized()? {
        window.unminimize()?;
    }
    let Some(monitor) = pointer_monitor(window)?
        .or(window.primary_monitor()?)
        .or(window.current_monitor()?)
    else {
        return window.center();
    };
    let (width, height) = outer_size(window)?;
    let (x, y) = centered(width, height, &work_area(&monitor));
    window.set_position(native_position(x, y))
}

fn pointer_monitor(window: &WebviewWindow) -> tauri::Result<Option<Monitor>> {
    let pointer = window.cursor_position()?;
    // macOS reports the pointer in pixels at the main display's scale, whatever
    // display it is over.
    let unit = match window.primary_monitor()? {
        Some(primary) => units(primary.scale_factor()),
        None => 1.0,
    };
    let (x, y) = (pointer.x / unit, pointer.y / unit);
    Ok(window
        .available_monitors()?
        .into_iter()
        .find(|monitor| display(monitor).contains(x, y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEFT: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1040.0,
    };
    /// A second display to the right of the first, a little lower.
    const RIGHT: Rect = Rect {
        x: 1920.0,
        y: 200.0,
        width: 1280.0,
        height: 1000.0,
    };

    #[test]
    fn a_window_left_on_a_display_is_reachable() {
        assert!(is_reachable(100.0, 100.0, 600.0, &[LEFT, RIGHT]));
        assert!(is_reachable(2000.0, 300.0, 600.0, &[LEFT, RIGHT]));
    }

    #[test]
    fn a_window_on_a_display_that_is_gone_is_not() {
        assert!(!is_reachable(2000.0, 300.0, 600.0, &[LEFT]));
    }

    #[test]
    fn most_of_a_window_may_hang_off_a_display_if_its_title_bar_can_be_held() {
        // All but 150 of 600 pixels past the left edge.
        assert!(is_reachable(-450.0, 100.0, 600.0, &[LEFT]));
        // All but 50: too little to take hold of.
        assert!(!is_reachable(-550.0, 100.0, 600.0, &[LEFT]));
    }

    #[test]
    fn a_title_bar_above_the_usable_area_or_below_it_is_not_reachable() {
        assert!(!is_reachable(100.0, -20.0, 600.0, &[LEFT]));
        assert!(!is_reachable(100.0, 1030.0, 600.0, &[LEFT]));
    }

    #[test]
    fn a_window_across_two_displays_is_reachable_on_either() {
        assert!(is_reachable(1700.0, 300.0, 600.0, &[LEFT, RIGHT]));
    }

    #[test]
    fn a_window_is_centered_on_the_area_it_is_given() {
        assert_eq!(centered(600.0, 400.0, &LEFT), (660.0, 320.0));
        assert_eq!(centered(600.0, 400.0, &RIGHT), (2260.0, 500.0));
    }

    #[test]
    fn a_window_larger_than_the_area_keeps_its_title_bar_on_it() {
        assert_eq!(centered(1400.0, 1200.0, &RIGHT), (1920.0, 200.0));
    }
}
