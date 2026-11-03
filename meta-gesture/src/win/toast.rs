//! The name of the command a gesture stood for, shown for a moment beside
//! the pointer. The window never takes the focus or a click: it is layered,
//! transparent to the mouse, and shown without being activated.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateRoundRectRgn,
    CreateSolidBrush, DEFAULT_CHARSET, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    DeleteObject, DrawTextW, EndPaint, FW_SEMIBOLD, FillRect, GetDC, GetMonitorInfoW,
    GetTextExtentPoint32W, HFONT, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
    OUT_DEFAULT_PRECIS, PAINTSTRUCT, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    SetWindowRgn, TRANSPARENT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetCursorPos, HWND_TOPMOST, KillTimer, LWA_ALPHA,
    RegisterClassExW, SW_HIDE, SWP_NOACTIVATE, SWP_SHOWWINDOW, SetLayeredWindowAttributes,
    SetTimer, SetWindowPos, ShowWindow, WM_PAINT, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::w;

/// How long the name stays, in milliseconds.
const SHOWN_FOR: u32 = 900;
const TIMER: usize = 1;
/// In DIPs: the text's size, the space around it, and the gap between the
/// pointer and the window.
const TEXT_SIZE: i32 = 15;
const PADDING: (i32, i32) = (14, 8);
const OFFSET: i32 = 18;
const BACKGROUND: COLORREF = COLORREF(0x0020_2020);
const TEXT: COLORREF = COLORREF(0x00FF_FFFF);

thread_local! {
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    static FONT: Cell<Option<(HFONT, u32)>> = const { Cell::new(None) };
    static SHOWN: RefCell<Vec<u16>> = const { RefCell::new(Vec::new()) };
}

fn window() -> Option<HWND> {
    if let Some(hwnd) = WINDOW.get() {
        return Some(hwnd);
    }
    let instance = unsafe { GetModuleHandleW(None) }.ok()?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: w!("meta-gesture.toast"),
        ..Default::default()
    };
    unsafe { RegisterClassExW(&class) };
    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT,
            w!("meta-gesture.toast"),
            w!(""),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }
    .ok()?;
    let _ = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 235, LWA_ALPHA) };
    WINDOW.set(Some(hwnd));
    Some(hwnd)
}

/// The font for `dpi`, made once for each DPI met in a row.
fn font(dpi: u32) -> HFONT {
    if let Some((font, at)) = FONT.get() {
        if at == dpi {
            return font;
        }
        let _ = unsafe { DeleteObject(font.into()) };
    }
    let height = -(TEXT_SIZE * dpi as i32 / 96);
    let font = unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            FW_SEMIBOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Yu Gothic UI"),
        )
    };
    FONT.set(Some((font, dpi)));
    font
}

/// Shows `text` beside the pointer, kept on the monitor it is on.
pub fn show(text: &str) {
    let Some(hwnd) = window() else {
        return;
    };
    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let (mut dpi, mut _y) = (96, 96);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut _y) };
    let px = |dips: i32| dips * dpi as i32 / 96;

    let wide: Vec<u16> = text.encode_utf16().collect();
    let font = font(dpi);
    let mut size = SIZE::default();
    unsafe {
        let dc = GetDC(None);
        let old = SelectObject(dc, font.into());
        let _ = GetTextExtentPoint32W(dc, &wide, &mut size);
        SelectObject(dc, old);
        ReleaseDC(None, dc);
    }
    let (w, h) = (size.cx + 2 * px(PADDING.0), size.cy + 2 * px(PADDING.1));
    let mut x = cursor.x + px(OFFSET);
    let mut y = cursor.y + px(OFFSET);
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        let work = info.rcWork;
        x = x.min(work.right - w).max(work.left);
        y = y.min(work.bottom - h).max(work.top);
    }
    SHOWN.with(|s| *s.borrow_mut() = wide);
    unsafe {
        let radius = px(8);
        let region = CreateRoundRectRgn(0, 0, w + 1, h + 1, radius, radius);
        SetWindowRgn(hwnd, Some(region), false);
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
        let _ = windows::Win32::Graphics::Gdi::InvalidateRect(Some(hwnd), None, true);
        SetTimer(Some(hwnd), TIMER, SHOWN_FOR, None);
    }
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe {
                let dc = BeginPaint(hwnd, &mut ps);
                let mut rect = RECT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect);
                let brush = CreateSolidBrush(BACKGROUND);
                FillRect(dc, &rect, brush);
                let _ = DeleteObject(brush.into());
                if let Some((font, _)) = FONT.get() {
                    SelectObject(dc, font.into());
                }
                SetBkMode(dc, TRANSPARENT);
                SetTextColor(dc, TEXT);
                SHOWN.with(|s| {
                    let mut text = s.borrow().clone();
                    DrawTextW(
                        dc,
                        &mut text,
                        &mut rect,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                    );
                });
                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        WM_TIMER if wparam.0 == TIMER => {
            unsafe {
                let _ = KillTimer(Some(hwnd), TIMER);
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
