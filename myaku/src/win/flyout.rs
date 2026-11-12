//! The list that opens above the notification-area icon, as Windows 11's own
//! flyouts do: the system's translucent backdrop and rounded corners, the
//! app colours and accent colour Windows is set to, drawn with Direct2D and
//! DirectWrite. Before Windows 11 22H2, which has no such backdrop, the
//! window is opaque.
//!
//! When the list is kept on screen it is the same window, but it stays open
//! when the focus moves elsewhere, moves by its heading, and closes with a
//! button of its own.

use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::{
    D2D_RECT_F, D2D_SIZE_U, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_FIGURE_BEGIN_FILLED,
    D2D1_FIGURE_END_CLOSED, D2D1_PIXEL_FORMAT,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1_ANTIALIAS_MODE_PER_PRIMITIVE, D2D1_CAP_STYLE_ROUND, D2D1_DRAW_TEXT_OPTIONS_CLIP,
    D2D1_FACTORY_TYPE_SINGLE_THREADED, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_LAYER_OPTIONS_NONE,
    D2D1_LAYER_PARAMETERS, D2D1_LINE_JOIN_ROUND, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_ROUNDED_RECT, D2D1_STROKE_STYLE_PROPERTIES,
    D2D1CreateFactory, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1Layer, ID2D1SolidColorBrush,
    ID2D1StrokeStyle,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE,
    DWRITE_FONT_STYLE_ITALIC, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT,
    DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD, DWRITE_MEASURING_MODE_NATURAL,
    DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING,
    DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory,
    IDWriteFactory, IDWriteFontCollection, IDWriteTextFormat,
};
use windows::Win32::Graphics::Dwm::{
    DWMSBT_TRANSIENTWINDOW, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmExtendFrameIntoClientArea,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, GetMonitorInfoW, HMONITOR, InvalidateRect, MONITOR_DEFAULTTONEAREST,
    MONITORINFO, MonitorFromPoint, MonitorFromRect, PAINTSTRUCT, ScreenToClient,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent, VK_ESCAPE, VK_LEFT, VK_RIGHT,
};
use windows::Win32::UI::Shell::{NOTIFYICONIDENTIFIER, Shell_NotifyIconGetRect};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetClientRect, GetCursorPos, GetWindowRect, HTCAPTION,
    HTCLIENT, HWND_TOPMOST, IDC_ARROW, IsWindowVisible, LoadCursorW, RegisterClassExW, SW_HIDE,
    SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SetForegroundWindow, SetWindowPos, ShowWindow,
    WA_INACTIVE, WM_ACTIVATE, WM_DPICHANGED, WM_ERASEBKGND, WM_EXITSIZEMOVE, WM_KEYDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCALCSIZE, WM_NCHITTEST, WM_PAINT, WM_SIZE, WNDCLASSEXW,
    WS_BORDER, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP, WS_THICKFRAME,
};
use windows::core::{HSTRING, PCWSTR, w};
use windows_numerics::{Matrix3x2, Vector2};

use super::app::{TRAY_ID, main_window};
use super::settings;
use super::theme::{self, Colour, Palette};
use super::{APP_NAME, runner};
use myaku::format::LAYERS;
use myaku::monitor::Row;
use myaku::place::{self, Area};
use myaku::resource::Resource;

const CLASS_NAME: PCWSTR = w!("myaku.flyout");

/// The layout, in device-independent pixels.
const WIDTH: f32 = 440.0;
const PAD: f32 = 16.0;
const HEADER: f32 = 48.0;
const ROW: f32 = 22.0;
const ROWS: usize = 20;
const BOTTOM: f32 = 12.0;
const HEIGHT: f32 = HEADER + ROW * ROWS as f32 + BOTTOM;
const NAME_WIDTH: f32 = 150.0;
const VALUE_WIDTH: f32 = 56.0;
const GAP: f32 = 10.0;
const CHART_INSET: f32 = 4.0;
/// Between the window and the taskbar or the screen's edge, as Windows'
/// own flyouts keep.
const MARGIN: f32 = 12.0;
const BUTTON: (f32, f32) = (32.0, 28.0);
/// Between the arrows and the close button of the list kept on screen.
const CLOSE_GAP: f32 = 8.0;

/// How the graph's layers are filled, from the first: the first a little
/// translucent, the middle two lighter over it, and the fourth solid, so
/// that each core's worth reads as a step in shade.
const LAYER_OPACITY: [f32; 4] = [0.7, 0.5, 0.5, 1.0];

/// A click on the icon while the list is open first takes the focus away,
/// which hides it; the click itself arrives after. It should leave the list
/// closed rather than open it again.
const REOPEN_GRACE: Duration = Duration::from_millis(500);

/// The heading's buttons: the arrows, and the close button that only the
/// list kept on screen has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Previous,
    Next,
    Close,
}

impl Button {
    fn shown(pinned: bool) -> &'static [Button] {
        if pinned {
            &[Button::Previous, Button::Next, Button::Close]
        } else {
            &[Button::Previous, Button::Next]
        }
    }

    /// -1 for the left arrow, 1 for the right; 0 for the close button.
    fn step(self) -> i32 {
        match self {
            Button::Previous => -1,
            Button::Next => 1,
            Button::Close => 0,
        }
    }

    fn rect(self, pinned: bool) -> D2D_RECT_F {
        let right = WIDTH - PAD + 6.0;
        let arrows_right = if pinned {
            right - BUTTON.0 - CLOSE_GAP
        } else {
            right
        };
        let left = match self {
            Button::Close => right - BUTTON.0,
            Button::Next => arrows_right - BUTTON.0,
            Button::Previous => arrows_right - 2.0 * BUTTON.0 - 2.0,
        };
        let top = (HEADER - BUTTON.1) / 2.0;
        D2D_RECT_F {
            left,
            top,
            right: left + BUTTON.0,
            bottom: top + BUTTON.1,
        }
    }

    fn hit(x: f32, y: f32, pinned: bool) -> Option<Button> {
        Button::shown(pinned).iter().copied().find(|b| {
            let r = b.rect(pinned);
            (r.left..r.right).contains(&x) && (r.top..r.bottom).contains(&y)
        })
    }
}

struct Texts {
    title: IDWriteTextFormat,
    name: IDWriteTextFormat,
    exited: IDWriteTextFormat,
    value: IDWriteTextFormat,
}

struct Flyout {
    hwnd: HWND,
    factory: ID2D1Factory,
    round: ID2D1StrokeStyle,
    texts: Texts,
    /// Made on first paint and again after the device is lost.
    target: Option<(ID2D1HwndRenderTarget, ID2D1SolidColorBrush)>,
    palette: Palette,
    /// Whether Windows draws the translucent backdrop behind the list.
    backdrop: bool,
    resource: Resource,
    rows: Vec<Row>,
    hover: Option<Button>,
    hidden_at: Option<Instant>,
    /// Kept on screen: stays open when the focus moves elsewhere.
    pinned: bool,
    dpi: u32,
}

thread_local! {
    static FLYOUT: RefCell<Option<Flyout>> = const { RefCell::new(None) };
}

/// Runs `f` on the list's state. Never call anything inside `f` that sends
/// a message to the list's window (SetWindowPos, ShowWindow): the window
/// procedure would try to borrow the state again.
fn with_flyout<R>(f: impl FnOnce(&mut Flyout) -> R) -> Option<R> {
    FLYOUT.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn create() -> windows::core::Result<()> {
    let instance = unsafe { GetModuleHandleW(None) }?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }?,
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    unsafe { RegisterClassExW(&class) };
    let wants_backdrop = theme::windows_build() >= 22621;
    // The backdrop, the shadow and the rounded corners come with a sizing
    // frame, which WM_NCCALCSIZE then folds into the client area. Without
    // the backdrop a plain bordered popup is enough.
    let style = if wants_backdrop {
        WS_POPUP | WS_THICKFRAME
    } else {
        WS_POPUP | WS_BORDER
    };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
            CLASS_NAME,
            &HSTRING::from(APP_NAME),
            style,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }?;
    let palette = Palette::current();
    let backdrop = wants_backdrop && apply_backdrop(hwnd, palette.dark);
    let factory: ID2D1Factory =
        unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;
    let round = unsafe {
        factory.CreateStrokeStyle(
            &D2D1_STROKE_STYLE_PROPERTIES {
                startCap: D2D1_CAP_STYLE_ROUND,
                endCap: D2D1_CAP_STYLE_ROUND,
                lineJoin: D2D1_LINE_JOIN_ROUND,
                ..Default::default()
            },
            None,
        )
    }?;
    let texts = texts()?;
    FLYOUT.with(|s| {
        *s.borrow_mut() = Some(Flyout {
            hwnd,
            factory,
            round,
            texts,
            target: None,
            palette,
            backdrop,
            resource: Resource::Cpu,
            rows: Vec::new(),
            hover: None,
            hidden_at: None,
            pinned: false,
            dpi: 96,
        })
    });
    Ok(())
}

/// Asks DWM for the flyout backdrop, dark or light, with rounded corners.
/// False when this Windows does not draw it.
fn apply_backdrop(hwnd: HWND, dark: bool) -> bool {
    unsafe {
        set_dark(hwnd, dark);
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&raw const corner).cast(),
            size_of_val(&corner) as u32,
        );
        let kind = DWMSBT_TRANSIENTWINDOW;
        let set = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&raw const kind).cast(),
            size_of_val(&kind) as u32,
        );
        if set.is_err() {
            return false;
        }
        // The whole window is "frame", so the backdrop shows through where
        // the list draws nothing.
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        DwmExtendFrameIntoClientArea(hwnd, &margins).is_ok()
    }
}

/// The backdrop's and the border's shade.
fn set_dark(hwnd: HWND, dark: bool) {
    let value = windows::core::BOOL::from(dark);
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&raw const value).cast(),
            size_of_val(&value) as u32,
        )
    };
}

/// Segoe UI Variable where Windows 11 has it, Segoe UI before.
fn font_family(dwrite: &IDWriteFactory, variable: &str) -> HSTRING {
    let mut collection: Option<IDWriteFontCollection> = None;
    let found = unsafe { dwrite.GetSystemFontCollection(&mut collection, false) }
        .ok()
        .and(collection)
        .is_some_and(|c| {
            let mut index = 0;
            let mut exists = windows::core::BOOL::default();
            unsafe { c.FindFamilyName(&HSTRING::from(variable), &mut index, &mut exists) }.is_ok()
                && exists.as_bool()
        });
    HSTRING::from(if found { variable } else { "Segoe UI" })
}

fn texts() -> windows::core::Result<Texts> {
    let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
    let display = font_family(&dwrite, "Segoe UI Variable Display");
    let text = font_family(&dwrite, "Segoe UI Variable Text");
    let format = |family: &HSTRING,
                  weight: DWRITE_FONT_WEIGHT,
                  style: DWRITE_FONT_STYLE,
                  size: f32|
     -> windows::core::Result<IDWriteTextFormat> {
        let f = unsafe {
            dwrite.CreateTextFormat(
                family,
                None,
                weight,
                style,
                DWRITE_FONT_STRETCH_NORMAL,
                size,
                w!("ja-JP"),
            )
        }?;
        unsafe {
            f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
            f.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
            // Names too long for their column end in an ellipsis.
            let sign = dwrite.CreateEllipsisTrimmingSign(&f)?;
            f.SetTrimming(
                &DWRITE_TRIMMING {
                    granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                    ..Default::default()
                },
                &sign,
            )?;
        }
        Ok(f)
    };
    let value = format(
        &text,
        DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_FONT_STYLE_NORMAL,
        12.0,
    )?;
    unsafe { value.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_TRAILING) }?;
    Ok(Texts {
        title: format(
            &display,
            DWRITE_FONT_WEIGHT_SEMI_BOLD,
            DWRITE_FONT_STYLE_NORMAL,
            14.0,
        )?,
        name: format(
            &text,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            12.0,
        )?,
        exited: format(
            &text,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_ITALIC,
            12.0,
        )?,
        value,
    })
}

fn is_visible() -> bool {
    with_flyout(|f| unsafe { IsWindowVisible(f.hwnd) }.as_bool()).unwrap_or(false)
}

fn is_pinned() -> bool {
    with_flyout(|f| f.pinned).unwrap_or(false)
}

/// The icon was clicked: open the list, or leave it closed when that click
/// is what just closed it. The list kept on screen only comes to the front.
pub fn toggle() {
    if is_pinned() {
        show();
        return;
    }
    if is_visible() {
        hide();
        return;
    }
    let just_hidden = with_flyout(|f| {
        f.hidden_at
            .take()
            .is_some_and(|t| t.elapsed() < REOPEN_GRACE)
    });
    if just_hidden != Some(true) {
        show();
    }
}

pub fn show() {
    show_with(true);
}

/// Opens the list, kept on screen where it was last left when it is, and
/// takes the focus when `activate`.
fn show_with(activate: bool) {
    let Some((hwnd, pinned)) = with_flyout(|f| (f.hwnd, f.pinned)) else {
        return;
    };
    let (rect, dpi) = match settings::pinned_origin().filter(|_| pinned) {
        Some(origin) => pinned_placement(origin),
        None => placement(),
    };
    with_flyout(|f| {
        f.dpi = dpi;
        f.hover = None;
        f.rows = runner().top(f.resource, ROWS, settings::memory_scale());
    });
    // Twice: moving onto a monitor of another scale makes Windows send
    // WM_DPICHANGED, after which the size is set again for that scale.
    for flags in [SWP_NOACTIVATE, SWP_SHOWWINDOW] {
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
                flags,
            )
        };
    }
    if activate {
        let _ = unsafe { SetForegroundWindow(hwnd) };
    }
    invalidate();
}

/// Keeps the list on screen, or lets it close again as a flyout does. The
/// choice is kept for the next run.
pub fn set_pinned(on: bool) {
    settings::set_keep_on_screen(on);
    with_flyout(|f| {
        f.pinned = on;
        f.hover = None;
    });
    if on {
        show_with(false);
    } else {
        close();
    }
}

/// At start-up: opens the list kept on screen, without taking the focus
/// from whatever is in front.
pub fn restore_pinned() {
    if settings::keep_on_screen() {
        with_flyout(|f| f.pinned = true);
        show_with(false);
    }
}

/// Closes the list as a flyout closes: the focus moved, Esc, the icon. The
/// list kept on screen stays.
pub fn hide() {
    if !is_pinned() {
        close();
    }
}

fn close() {
    let Some(hwnd) = with_flyout(|f| f.hwnd) else {
        return;
    };
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        with_flyout(|f| f.hidden_at = Some(Instant::now()));
        let _ = unsafe { ShowWindow(hwnd, SW_HIDE) };
    }
}

/// A new reading came in: redraw the list if it is open.
pub fn refresh() {
    if !is_visible() {
        return;
    }
    with_flyout(|f| f.rows = runner().top(f.resource, ROWS, settings::memory_scale()));
    invalidate();
}

/// Windows' colours changed.
pub fn theme_changed() {
    with_flyout(|f| {
        f.palette = Palette::current();
        if f.backdrop {
            set_dark(f.hwnd, f.palette.dark);
        }
    });
    invalidate();
}

fn invalidate() {
    if let Some(hwnd) = with_flyout(|f| f.hwnd) {
        let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
    }
}

fn press(button: Button) {
    if button == Button::Close {
        set_pinned(false);
        return;
    }
    with_flyout(|f| {
        f.resource = f.resource.cycle(button.step(), false);
        f.rows = runner().top(f.resource, ROWS, settings::memory_scale());
    });
    invalidate();
}

/// Where the list goes, in physical pixels, and the scale there: next to
/// the icon on the taskbar's side of the screen, kept clear of the taskbar
/// and the screen's edges.
fn placement() -> (RECT, u32) {
    let anchor = main_window()
        .and_then(|hwnd| {
            let id = NOTIFYICONIDENTIFIER {
                cbSize: size_of::<NOTIFYICONIDENTIFIER>() as u32,
                hWnd: hwnd,
                uID: TRAY_ID,
                ..Default::default()
            };
            unsafe { Shell_NotifyIconGetRect(&id) }.ok()
        })
        .unwrap_or_else(|| {
            // Hidden in the overflow, or Explorer is not answering: the
            // pointer is where the click was.
            let mut p = POINT::default();
            let _ = unsafe { GetCursorPos(&mut p) };
            RECT {
                left: p.x,
                top: p.y,
                right: p.x + 1,
                bottom: p.y + 1,
            }
        });
    let monitor = unsafe { MonitorFromRect(&anchor, MONITOR_DEFAULTTONEAREST) };
    let (info, dpi) = monitor_info(monitor);
    let (w, h, m) = sizes(dpi);
    let (work, screen) = (info.rcWork, info.rcMonitor);
    let centre_x = (anchor.left + anchor.right) / 2;
    let centre_y = (anchor.top + anchor.bottom) / 2;
    let clamp = |v: i32, lo: i32, hi: i32| v.min(hi).max(lo);
    let along_x = clamp(centre_x - w / 2, work.left + m, work.right - w - m);
    let along_y = clamp(centre_y - h / 2, work.top + m, work.bottom - h - m);
    let (x, y) = if work.bottom < screen.bottom {
        (along_x, work.bottom - h - m)
    } else if work.top > screen.top {
        (along_x, work.top + m)
    } else if work.left > screen.left {
        (work.left + m, along_y)
    } else if work.right < screen.right {
        (work.right - w - m, along_y)
    } else {
        // An auto-hiding taskbar takes no room: go above the icon, or below
        // it near the top of the screen.
        let above = anchor.top - h - m;
        let y = if above >= work.top {
            above
        } else {
            anchor.bottom + m
        };
        (along_x, clamp(y, work.top + m, work.bottom - h - m))
    };
    (
        RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        },
        dpi,
    )
}

/// The list kept on screen: where it was last left, on the screen nearest
/// to there, moved back inside should that screen have changed.
fn pinned_placement(origin: (i32, i32)) -> (RECT, u32) {
    let point = POINT {
        x: origin.0,
        y: origin.1,
    };
    let monitor = unsafe { MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST) };
    let (info, dpi) = monitor_info(monitor);
    let (w, h, m) = sizes(dpi);
    let work = info.rcWork;
    let area = Area {
        left: work.left,
        top: work.top,
        right: work.right,
        bottom: work.bottom,
    };
    let (x, y) = place::keep_inside(origin, (w, h), area, m);
    (
        RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        },
        dpi,
    )
}

fn monitor_info(monitor: HMONITOR) -> (MONITORINFO, u32) {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
    let (mut dpi, mut dpi_y) = (96, 96);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) };
    (info, dpi)
}

/// The window's width, height and margin from the screen's edge, in
/// physical pixels at `dpi`.
fn sizes(dpi: u32) -> (i32, i32, i32) {
    let px = |dip: f32| (dip * dpi as f32 / 96.0).round() as i32;
    (px(WIDTH), px(HEIGHT), px(MARGIN))
}

fn colour(c: Colour) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: c.r,
        g: c.g,
        b: c.b,
        a: c.a,
    }
}

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}

fn paint(f: &mut Flyout) {
    if f.target.is_none() {
        f.target = make_target(f).ok();
    }
    let Some((target, brush)) = f.target.clone() else {
        return;
    };
    let result = unsafe {
        target.SetDpi(f.dpi as f32, f.dpi as f32);
        target.BeginDraw();
        let clear = if f.backdrop {
            D2D1_COLOR_F::default()
        } else {
            colour(f.palette.background)
        };
        target.Clear(Some(&clear));
        draw(f, &target, &brush);
        target.EndDraw(None, None)
    };
    if result.is_err() {
        // The device was lost (a driver update, a remote session): start
        // over on the next paint.
        f.target = None;
        invalidate_later(f.hwnd);
    }
}

fn invalidate_later(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}

fn make_target(f: &Flyout) -> windows::core::Result<(ID2D1HwndRenderTarget, ID2D1SolidColorBrush)> {
    let mut client = RECT::default();
    unsafe { GetClientRect(f.hwnd, &mut client) }?;
    let target = unsafe {
        f.factory.CreateHwndRenderTarget(
            &D2D1_RENDER_TARGET_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: f.dpi as f32,
                dpiY: f.dpi as f32,
                ..Default::default()
            },
            &D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: f.hwnd,
                pixelSize: D2D_SIZE_U {
                    width: (client.right - client.left).max(1) as u32,
                    height: (client.bottom - client.top).max(1) as u32,
                },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            },
        )
    }?;
    let brush = unsafe { target.CreateSolidColorBrush(&D2D1_COLOR_F::default(), None) }?;
    Ok((target, brush))
}

fn text(
    target: &ID2D1HwndRenderTarget,
    s: &str,
    format: &IDWriteTextFormat,
    r: D2D_RECT_F,
    brush: &ID2D1SolidColorBrush,
) {
    let units: Vec<u16> = s.encode_utf16().collect();
    unsafe {
        target.DrawText(
            &units,
            format,
            &r,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        )
    };
}

unsafe fn draw(f: &Flyout, target: &ID2D1HwndRenderTarget, brush: &ID2D1SolidColorBrush) {
    let p = f.palette;
    let set = |c: Colour| unsafe { brush.SetColor(&colour(c)) };

    // The heading and the arrows that switch it.
    set(p.text);
    text(
        target,
        f.resource.heading(settings::memory_scale()),
        &f.texts.title,
        rect(PAD, 0.0, Button::Previous.rect(f.pinned).left - 8.0, HEADER),
        brush,
    );
    for &button in Button::shown(f.pinned) {
        let r = button.rect(f.pinned);
        if f.hover == Some(button) {
            set(p.hover);
            unsafe {
                target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: r,
                        radiusX: 4.0,
                        radiusY: 4.0,
                    },
                    brush,
                )
            };
        }
        set(p.secondary);
        let (cx, cy) = ((r.left + r.right) / 2.0, (r.top + r.bottom) / 2.0);
        if button == Button::Close {
            // A cross as wide as an arrow is tall.
            let d = 4.5;
            for (a, b) in [((-d, -d), (d, d)), ((-d, d), (d, -d))] {
                let from = Vector2::new(cx + a.0, cy + a.1);
                let to = Vector2::new(cx + b.0, cy + b.1);
                unsafe { target.DrawLine(from, to, brush, 1.5, &f.round) };
            }
            continue;
        }
        let dx = 2.5 * -button.step() as f32;
        let tip = Vector2::new(cx - dx, cy);
        for end in [
            Vector2::new(cx + dx, cy - 5.0),
            Vector2::new(cx + dx, cy + 5.0),
        ] {
            unsafe { target.DrawLine(tip, end, brush, 1.5, &f.round) };
        }
    }

    // One line per process: the name, the last minute, the latest value.
    let chart_left = PAD + NAME_WIDTH + GAP;
    let chart_right = WIDTH - PAD - VALUE_WIDTH - GAP;
    for (i, row) in f.rows.iter().enumerate() {
        let top = HEADER + ROW * i as f32;
        let (name_format, name_colour) = if row.alive {
            (&f.texts.name, p.text)
        } else {
            (&f.texts.exited, p.text.alpha(0.5))
        };
        set(name_colour);
        text(
            target,
            &row.name,
            name_format,
            rect(PAD, top, PAD + NAME_WIDTH, top + ROW),
            brush,
        );
        let chart = rect(
            chart_left,
            top + CHART_INSET,
            chart_right,
            top + ROW - CHART_INSET,
        );
        unsafe { draw_chart(f, target, brush, chart, &row.chart) };
        set(p.secondary);
        text(
            target,
            &row.value,
            &f.texts.value,
            rect(WIDTH - PAD - VALUE_WIDTH, top, WIDTH - PAD, top + ROW),
            brush,
        );
    }
}

/// The graph: a rounded track, and the values filled in layers over it
/// (see `myaku::format::LAYERS`), clipped to the track's corners.
unsafe fn draw_chart(
    f: &Flyout,
    target: &ID2D1HwndRenderTarget,
    brush: &ID2D1SolidColorBrush,
    r: D2D_RECT_F,
    values: &[f32],
) {
    let rounded = D2D1_ROUNDED_RECT {
        rect: r,
        radiusX: 3.0,
        radiusY: 3.0,
    };
    unsafe {
        brush.SetColor(&colour(f.palette.track));
        target.FillRoundedRectangle(&rounded, brush);
    }
    if values.len() < 2 {
        return;
    }
    let Ok(mask) = (unsafe { f.factory.CreateRoundedRectangleGeometry(&rounded) }) else {
        return;
    };
    let params = D2D1_LAYER_PARAMETERS {
        contentBounds: rect(f32::MIN, f32::MIN, f32::MAX, f32::MAX),
        geometricMask: ManuallyDrop::new(Some(mask.into())),
        maskAntialiasMode: D2D1_ANTIALIAS_MODE_PER_PRIMITIVE,
        maskTransform: Matrix3x2::identity(),
        opacity: 1.0,
        opacityBrush: ManuallyDrop::new(None),
        layerOptions: D2D1_LAYER_OPTIONS_NONE,
    };
    unsafe { target.PushLayer(&params, None::<&ID2D1Layer>) };
    let peak = values.iter().copied().fold(0.0, f32::max);
    let (width, height) = (r.right - r.left, r.bottom - r.top);
    let step = width / (values.len() - 1) as f32;
    for (layer, opacity) in LAYER_OPACITY.iter().enumerate() {
        let layer = layer as f32;
        if layer >= LAYERS || peak <= layer {
            break;
        }
        let Ok(path) = (unsafe { f.factory.CreatePathGeometry() }) else {
            break;
        };
        let Ok(sink) = (unsafe { path.Open() }) else {
            break;
        };
        let mut points: Vec<Vector2> = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let level = (v - layer).clamp(0.0, 1.0);
                Vector2::new(r.left + step * i as f32, r.bottom - level * height)
            })
            .collect();
        points.push(Vector2::new(r.right, r.bottom));
        unsafe {
            sink.BeginFigure(Vector2::new(r.left, r.bottom), D2D1_FIGURE_BEGIN_FILLED);
            sink.AddLines(&points);
            sink.EndFigure(D2D1_FIGURE_END_CLOSED);
            if sink.Close().is_err() {
                break;
            }
            brush.SetColor(&colour(f.palette.accent.alpha(*opacity)));
            target.FillGeometry(&path, brush, None);
        }
    }
    unsafe { target.PopLayer() };
    drop(ManuallyDrop::into_inner(params.geometricMask));
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            with_flyout(paint);
            let _ = unsafe { EndPaint(hwnd, &ps) };
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        // With the sizing frame, which brings the backdrop and the shadow,
        // the whole window is client area and none of it resizes.
        WM_NCCALCSIZE if wparam.0 != 0 => LRESULT(0),
        // The list kept on screen moves by its heading, away from the
        // buttons.
        WM_NCHITTEST => {
            let mut point = POINT {
                x: (lparam.0 & 0xFFFF) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            };
            let _ = unsafe { ScreenToClient(hwnd, &mut point) };
            let (x, y) = to_dips(point.x, point.y);
            let pinned = is_pinned();
            let on_heading = (0.0..HEADER).contains(&y) && Button::hit(x, y, pinned).is_none();
            LRESULT(if pinned && on_heading {
                HTCAPTION
            } else {
                HTCLIENT
            } as isize)
        }
        WM_EXITSIZEMOVE => {
            let mut r = RECT::default();
            if unsafe { GetWindowRect(hwnd, &mut r) }.is_ok() {
                settings::set_pinned_origin(r.left, r.top);
            }
            LRESULT(0)
        }
        WM_SIZE => {
            let (w, h) = (
                (lparam.0 & 0xFFFF) as u32,
                ((lparam.0 >> 16) & 0xFFFF) as u32,
            );
            with_flyout(|f| {
                if let Some((target, _)) = &f.target {
                    let _ = unsafe {
                        target.Resize(&D2D_SIZE_U {
                            width: w.max(1),
                            height: h.max(1),
                        })
                    };
                }
            });
            LRESULT(0)
        }
        WM_DPICHANGED => {
            // Dragged onto a screen of another scale: take the size Windows
            // suggests for it. show() sets the size again itself after.
            with_flyout(|f| f.dpi = super::loword(wparam.0));
            // SAFETY: lParam of WM_DPICHANGED points to the suggested RECT.
            let r = unsafe { *(lparam.0 as *const RECT) };
            let _ = unsafe {
                SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            LRESULT(0)
        }
        WM_ACTIVATE => {
            if super::loword(wparam.0) == WA_INACTIVE {
                hide();
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            match wparam.0 as u16 {
                k if k == VK_LEFT.0 => press(Button::Previous),
                k if k == VK_RIGHT.0 => press(Button::Next),
                k if k == VK_ESCAPE.0 => hide(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            let (x, y) = dips(lparam);
            let changed = with_flyout(|f| {
                let hover = Button::hit(x, y, f.pinned);
                std::mem::replace(&mut f.hover, hover) != hover
            });
            let mut track = TRACKMOUSEEVENT {
                cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = unsafe { TrackMouseEvent(&mut track) };
            if changed == Some(true) {
                invalidate();
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if with_flyout(|f| f.hover.take().is_some()) == Some(true) {
                invalidate();
            }
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let (x, y) = dips(lparam);
            if let Some(button) = Button::hit(x, y, is_pinned()) {
                press(button);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// The pointer position in a mouse message, in device-independent pixels.
fn dips(lparam: LPARAM) -> (f32, f32) {
    to_dips(
        (lparam.0 & 0xFFFF) as i16 as i32,
        ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
    )
}

/// A point in the client area's physical pixels, in device-independent
/// ones.
fn to_dips(x: i32, y: i32) -> (f32, f32) {
    let dpi = with_flyout(|f| f.dpi).unwrap_or(96) as f32;
    (x as f32 * 96.0 / dpi, y as f32 * 96.0 / dpi)
}
