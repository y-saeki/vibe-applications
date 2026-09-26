//! The settings window: a list of shortcuts on the left, the selected one's
//! fields on the right.
//!
//! Every edit goes into a [`Draft`] straight away; nothing is checked until
//! 保存, which turns the drafts into a config, writes it and closes. The
//! resident part reads the file again when the window closes.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, COLOR_BTNFACE, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_WINDOW, CreateFontIndirectW,
    DeleteObject, EndPaint, FillRect, FrameRect, GetMonitorInfoW, GetSysColorBrush, HBRUSH, HFONT,
    InvalidateRect, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, MonitorFromWindow,
    PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    BST_CHECKED, BST_UNCHECKED, HKM_GETHOTKEY, HKM_SETHOTKEY, HOTKEY_CLASS, WC_COMBOBOXW,
    WC_LISTBOXW,
};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForMonitor, MDT_EFFECTIVE_DPI, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL,
    CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CreateWindowExW, DefWindowProcW, DestroyWindow,
    EN_CHANGE, ES_AUTOHSCROLL, GetCursorPos, GetWindowTextLengthW, GetWindowTextW, HMENU, IDCANCEL,
    IDNO, IDOK, IsDialogMessageW, LB_ADDSTRING, LB_DELETESTRING, LB_GETCURSEL, LB_INSERTSTRING,
    LB_SETCURSEL, LBN_SELCHANGE, LBS_NOINTEGRALHEIGHT, LBS_NOTIFY, MB_ICONWARNING, MB_YESNO, MSG,
    NONCLIENTMETRICSW, PostMessageW, RegisterClassExW, SPI_GETNONCLIENTMETRICS, SW_SHOW,
    SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetForegroundWindow, SetWindowPos, SetWindowTextW,
    ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_DESTROY, WM_DPICHANGED,
    WM_PAINT, WM_SETFONT, WNDCLASSEXW, WS_CAPTION, WS_CHILD, WS_EX_CLIENTEDGE, WS_EX_CONTROLPARENT,
    WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{HSTRING, PCWSTR, w};

use super::{APP_NAME, WM_APP_SETTINGS_CLOSED, autostart, error_box, hiword, loword, message_box};
use crate::config::Config;
use crate::draft::{self, Draft};
use crate::hotkey::MOD_WIN;
use crate::layout::{Anchor, Rect};

const CLASS_NAME: PCWSTR = w!("winwin.settings");

const ID_LIST: i32 = 100;
const ID_ADD: i32 = 101;
const ID_DUPLICATE: i32 = 102;
const ID_DELETE: i32 = 103;
const ID_UP: i32 = 104;
const ID_DOWN: i32 = 105;
const ID_WIN: i32 = 111;
const ID_HOTKEY: i32 = 112;
const ID_ANCHOR: i32 = 113;
const ID_WIDTH: i32 = 114;
const ID_HEIGHT: i32 = 115;
const ID_AUTOSTART: i32 = 120;

/// The client area at 96 DPI; everything below is laid out in these units
/// and scaled to the monitor's DPI.
const CLIENT: (i32, i32) = (720, 458);
const PREVIEW: [i32; 4] = [372, 186, 336, 216];

#[derive(Clone, Copy)]
struct Controls {
    list: HWND,
    duplicate: HWND,
    delete: HWND,
    up: HWND,
    down: HWND,
    win: HWND,
    hotkey: HWND,
    anchor: HWND,
    width: HWND,
    height: HWND,
    autostart: HWND,
}

impl Controls {
    /// The fields that belong to the selected shortcut.
    fn form(&self) -> [HWND; 5] {
        [self.win, self.hotkey, self.anchor, self.width, self.height]
    }
}

struct Settings {
    hwnd: HWND,
    owner: HWND,
    path: PathBuf,
    c: Controls,
    /// Every child window and where it goes, in 96-DPI units.
    layout: Vec<(HWND, [i32; 4])>,
    font: HFONT,
    rows: Vec<Draft>,
    current: Option<usize>,
    autostart_was: bool,
    dirty: bool,
}

thread_local! {
    static STATE: RefCell<Option<Settings>> = const { RefCell::new(None) };
    /// Kept apart from STATE so that the message loop can ask for it while
    /// a handler holds the state.
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
    /// Set while the form is being filled in, so that the change
    /// notifications that causes are not taken for edits.
    static FILLING: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` on the window's state. As in app.rs, `f` must not call anything
/// that sends messages to our own windows (setting a control's text does).
fn with_state<R>(f: impl FnOnce(&mut Settings) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok()?.as_mut().map(f))
}

pub fn is_dialog_message(msg: &MSG) -> bool {
    match WINDOW.get() {
        Some(hwnd) => unsafe { IsDialogMessageW(hwnd, msg) }.as_bool(),
        None => false,
    }
}

pub fn close() {
    if let Some(hwnd) = WINDOW.get() {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
}

fn scale(v: i32, dpi: u32) -> i32 {
    (i64::from(v) * i64::from(dpi) / 96) as i32
}

fn send(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))) }.0
}

fn text_of(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) }.max(0) as usize;
    let mut buf = vec![0u16; len + 1];
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

fn set_text(hwnd: HWND, text: &str) {
    let _ = unsafe { SetWindowTextW(hwnd, &HSTRING::from(text)) };
}

fn is_checked(hwnd: HWND) -> bool {
    send(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::BM_GETCHECK,
        0,
        0,
    ) == BST_CHECKED.0 as isize
}

fn set_checked(hwnd: HWND, checked: bool) {
    let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
    send(
        hwnd,
        windows::Win32::UI::WindowsAndMessaging::BM_SETCHECK,
        state.0 as usize,
        0,
    );
}

/// Opens the window, or brings it forward when it is already open. Returns
/// whether it was newly opened.
pub fn open(owner: HWND, config: &Config, path: &Path) -> bool {
    if let Some(hwnd) = WINDOW.get() {
        let _ = unsafe { SetForegroundWindow(hwnd) };
        return false;
    }
    match create(owner, config, path) {
        Ok(()) => true,
        Err(e) => {
            error_box(None, &format!("設定を開けませんでした。\n{e}"));
            false
        }
    }
}

fn register_class() -> windows::core::Result<()> {
    thread_local! {
        static REGISTERED: Cell<bool> = const { Cell::new(false) };
    }
    if REGISTERED.get() {
        return Ok(());
    }
    let instance = unsafe { GetModuleHandleW(None) }?;
    let class = WNDCLASSEXW {
        cbSize: size_of::<WNDCLASSEXW>() as u32,
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: CLASS_NAME,
        hbrBackground: HBRUSH((COLOR_BTNFACE.0 + 1) as isize as _),
        hCursor: unsafe {
            windows::Win32::UI::WindowsAndMessaging::LoadCursorW(
                None,
                windows::Win32::UI::WindowsAndMessaging::IDC_ARROW,
            )
        }?,
        ..Default::default()
    };
    unsafe { RegisterClassExW(&class) };
    REGISTERED.set(true);
    Ok(())
}

const STYLE: WINDOW_STYLE =
    WINDOW_STYLE(WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_MINIMIZEBOX.0);

fn create(owner: HWND, config: &Config, path: &Path) -> windows::core::Result<()> {
    register_class()?;
    let instance = unsafe { GetModuleHandleW(None) }?;

    // Open on the monitor the pointer is on, centered in its work area and
    // sized for its DPI.
    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let (mut dpi, mut dpi_y) = (96, 96);
    let _ = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi, &mut dpi_y) };
    let mut frame = RECT {
        left: 0,
        top: 0,
        right: scale(CLIENT.0, dpi),
        bottom: scale(CLIENT.1, dpi),
    };
    unsafe { AdjustWindowRectExForDpi(&mut frame, STYLE, false, WS_EX_CONTROLPARENT, dpi) }?;
    let (w, h) = (frame.right - frame.left, frame.bottom - frame.top);
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let _ = unsafe { GetMonitorInfoW(monitor, &mut info) };
    let work = info.rcWork;
    let x = work.left + (work.right - work.left - w) / 2;
    let y = work.top + (work.bottom - work.top - h) / 2;

    let hwnd = unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            CLASS_NAME,
            &HSTRING::from(format!("{APP_NAME} の設定")),
            STYLE,
            x,
            y,
            w,
            h,
            None,
            None,
            Some(instance.into()),
            None,
        )
    }?;

    let mut layout = Vec::new();
    let mut child = |class: PCWSTR,
                     text: &str,
                     style: u32,
                     ex: WINDOW_EX_STYLE,
                     id: i32,
                     rect: [i32; 4]|
     -> windows::core::Result<HWND> {
        let c = unsafe {
            CreateWindowExW(
                ex,
                class,
                &HSTRING::from(text),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(id as isize as _)),
                Some(instance.into()),
                None,
            )
        }?;
        layout.push((c, rect));
        Ok(c)
    };
    let edge = WS_EX_CLIENTEDGE;
    let none = WINDOW_EX_STYLE::default();
    let tab = WS_TABSTOP.0;
    let label = |y: i32| [268, y + 3, 100, 20];
    let button = w!("BUTTON");
    let edit = w!("EDIT");
    let stat = w!("STATIC");
    let text_box = tab | ES_AUTOHSCROLL as u32;

    let list = child(
        WC_LISTBOXW,
        "",
        tab | WS_VSCROLL.0 | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32,
        edge,
        ID_LIST,
        [12, 12, 240, 324],
    )?;
    child(
        button,
        "追加",
        tab | BS_PUSHBUTTON as u32,
        none,
        ID_ADD,
        [12, 344, 76, 28],
    )?;
    let duplicate = child(
        button,
        "複製",
        tab | BS_PUSHBUTTON as u32,
        none,
        ID_DUPLICATE,
        [94, 344, 76, 28],
    )?;
    let delete = child(
        button,
        "削除",
        tab | BS_PUSHBUTTON as u32,
        none,
        ID_DELETE,
        [176, 344, 76, 28],
    )?;
    let up = child(
        button,
        "上へ",
        tab | BS_PUSHBUTTON as u32,
        none,
        ID_UP,
        [12, 376, 117, 28],
    )?;
    let down = child(
        button,
        "下へ",
        tab | BS_PUSHBUTTON as u32,
        none,
        ID_DOWN,
        [135, 376, 117, 28],
    )?;

    child(stat, "ショートカット", 0, none, -1, label(12))?;
    let win = child(
        button,
        "Win +",
        tab | BS_AUTOCHECKBOX as u32,
        none,
        ID_WIN,
        [372, 12, 60, 24],
    )?;
    let hotkey = child(HOTKEY_CLASS, "", tab, edge, ID_HOTKEY, [436, 12, 272, 24])?;
    child(stat, "基準位置", 0, none, -1, label(44))?;
    let anchor = child(
        WC_COMBOBOXW,
        "",
        tab | WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32,
        none,
        ID_ANCHOR,
        [372, 44, 160, 300],
    )?;
    child(stat, "幅", 0, none, -1, label(76))?;
    let width = child(edit, "", text_box, edge, ID_WIDTH, [372, 76, 120, 24])?;
    child(stat, "高さ", 0, none, -1, label(108))?;
    let height = child(edit, "", text_box, edge, ID_HEIGHT, [372, 108, 120, 24])?;
    child(
        stat,
        "画面(タスクバーを除く)に対する比率で指定します。例: 1/2(半分)、2/3、0.75、1(全体)",
        0,
        none,
        -1,
        [372, 140, 336, 40],
    )?;
    child(stat, "プレビュー", 0, none, -1, label(PREVIEW[1]))?;

    let autostart_box = child(
        button,
        "Windows へのサインイン時に winwin を起動する",
        tab | BS_AUTOCHECKBOX as u32,
        none,
        ID_AUTOSTART,
        [12, 420, 400, 24],
    )?;
    child(
        button,
        "保存",
        tab | BS_DEFPUSHBUTTON as u32,
        none,
        IDOK.0,
        [536, 418, 80, 28],
    )?;
    child(
        button,
        "キャンセル",
        tab | BS_PUSHBUTTON as u32,
        none,
        IDCANCEL.0,
        [628, 418, 80, 28],
    )?;

    for a in Anchor::ALL {
        send(
            anchor,
            CB_ADDSTRING,
            0,
            HSTRING::from(a.label()).as_ptr() as isize,
        );
    }

    let c = Controls {
        list,
        duplicate,
        delete,
        up,
        down,
        win,
        hotkey,
        anchor,
        width,
        height,
        autostart: autostart_box,
    };
    let rows: Vec<Draft> = config.shortcuts.iter().map(Draft::from_shortcut).collect();
    let autostart_was = autostart::is_enabled();
    STATE.with(|s| {
        *s.borrow_mut() = Some(Settings {
            hwnd,
            owner,
            path: path.to_path_buf(),
            c,
            layout,
            font: HFONT::default(),
            rows: rows.clone(),
            current: None,
            autostart_was,
            dirty: false,
        })
    });
    WINDOW.set(Some(hwnd));

    apply_dpi(dpi);
    for row in &rows {
        send(
            list,
            LB_ADDSTRING,
            0,
            HSTRING::from(row.list_text()).as_ptr() as isize,
        );
    }
    set_checked(autostart_box, autostart_was);
    select(if rows.is_empty() { None } else { Some(0) });

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

/// Lays the children out for `dpi` and gives them the system's message font
/// at that size.
fn apply_dpi(dpi: u32) {
    let mut metrics = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    let got = unsafe {
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            metrics.cbSize,
            Some((&mut metrics as *mut NONCLIENTMETRICSW).cast()),
            0,
            dpi,
        )
    };
    let font = if got.is_ok() {
        unsafe { CreateFontIndirectW(&metrics.lfMessageFont) }
    } else {
        HFONT::default()
    };
    let Some((old, layout)) =
        with_state(|s| (std::mem::replace(&mut s.font, font), s.layout.clone()))
    else {
        return;
    };
    for (hwnd, [x, y, w, h]) in layout {
        unsafe {
            let _ = SetWindowPos(
                hwnd,
                None,
                scale(x, dpi),
                scale(y, dpi),
                scale(w, dpi),
                scale(h, dpi),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        send(hwnd, WM_SETFONT, font.0 as usize, 1);
    }
    if !old.is_invalid() {
        let _ = unsafe { DeleteObject(old.into()) };
    }
}

/// Shows row `index` in the form, or empties and disables the form.
fn select(index: Option<usize>) {
    let Some((c, row, len)) = with_state(|s| {
        s.current = index;
        (
            s.c,
            index.and_then(|i| s.rows.get(i).cloned()),
            s.rows.len(),
        )
    }) else {
        return;
    };
    if let Some(i) = index {
        send(c.list, LB_SETCURSEL, i, 0);
    }
    FILLING.set(true);
    let blank = Draft::new();
    let d = row.as_ref().unwrap_or(&blank);
    set_checked(c.win, d.modifiers & MOD_WIN != 0);
    send(
        c.hotkey,
        HKM_SETHOTKEY,
        usize::from(draft::to_hotkey_control(d.modifiers, d.vk)),
        0,
    );
    let anchor = Anchor::ALL.iter().position(|a| *a == d.anchor).unwrap_or(0);
    send(c.anchor, CB_SETCURSEL, anchor, 0);
    set_text(c.width, &d.width);
    set_text(c.height, &d.height);
    FILLING.set(false);

    let enabled = row.is_some();
    for hwnd in c.form().into_iter().chain([c.duplicate, c.delete]) {
        let _ = unsafe { EnableWindow(hwnd, enabled) };
    }
    let can_move = |up| index.is_some_and(|i| draft::moved(i, len, up).is_some());
    unsafe {
        let _ = EnableWindow(c.up, can_move(true));
        let _ = EnableWindow(c.down, can_move(false));
    }
    invalidate_preview();
}

fn read_form(c: &Controls) -> Draft {
    let value = send(c.hotkey, HKM_GETHOTKEY, 0, 0) as u16;
    let (mut modifiers, vk) = draft::from_hotkey_control(value);
    if is_checked(c.win) {
        modifiers |= MOD_WIN;
    }
    let anchor = send(c.anchor, CB_GETCURSEL, 0, 0);
    Draft {
        modifiers,
        vk,
        anchor: Anchor::ALL
            .get(usize::try_from(anchor).unwrap_or(0))
            .copied()
            .unwrap_or(Anchor::Center),
        width: text_of(c.width),
        height: text_of(c.height),
    }
}

/// A field of the selected row changed: store it, and refresh its line in
/// the list and the preview.
fn on_form_changed() {
    if FILLING.get() {
        return;
    }
    let Some(c) = with_state(|s| s.c) else {
        return;
    };
    let row = read_form(&c);
    let text = row.list_text();
    let Some(index) = with_state(|s| {
        let i = s.current?;
        s.rows[i] = row;
        s.dirty = true;
        Some(i)
    })
    .flatten() else {
        return;
    };
    send(c.list, LB_DELETESTRING, index, 0);
    send(
        c.list,
        LB_INSERTSTRING,
        index,
        HSTRING::from(text).as_ptr() as isize,
    );
    send(c.list, LB_SETCURSEL, index, 0);
    invalidate_preview();
}

fn insert_row(row: Draft) {
    let text = row.list_text();
    let Some((c, index)) = with_state(|s| {
        let i = s.current.map_or(s.rows.len(), |i| i + 1);
        s.rows.insert(i, row);
        s.dirty = true;
        (s.c, i)
    }) else {
        return;
    };
    send(
        c.list,
        LB_INSERTSTRING,
        index,
        HSTRING::from(text).as_ptr() as isize,
    );
    select(Some(index));
    unsafe {
        let _ = SetFocus(Some(c.hotkey));
    }
}

fn delete_row() {
    let Some((c, removed, remaining)) = with_state(|s| {
        let i = s.current?;
        s.rows.remove(i);
        s.dirty = true;
        Some((s.c, i, s.rows.len()))
    })
    .flatten() else {
        return;
    };
    send(c.list, LB_DELETESTRING, removed, 0);
    select(if remaining == 0 {
        None
    } else {
        Some(removed.min(remaining - 1))
    });
}

/// Moves the selected row one place up or down, which is also its turn
/// among the rows that share its shortcut.
fn move_row(up: bool) {
    let Some((c, from, to, text)) = with_state(|s| {
        let from = s.current?;
        let to = draft::moved(from, s.rows.len(), up)?;
        s.rows.swap(from, to);
        s.dirty = true;
        Some((s.c, from, to, s.rows[to].list_text()))
    })
    .flatten() else {
        return;
    };
    send(c.list, LB_DELETESTRING, from, 0);
    send(
        c.list,
        LB_INSERTSTRING,
        to,
        HSTRING::from(text).as_ptr() as isize,
    );
    select(Some(to));
}

fn save() {
    let Some((hwnd, c, rows, path, autostart_was)) =
        with_state(|s| (s.hwnd, s.c, s.rows.clone(), s.path.clone(), s.autostart_was))
    else {
        return;
    };
    let mut config = Config::default();
    for (i, row) in rows.iter().enumerate() {
        match row.to_shortcut() {
            Ok(s) => config.shortcuts.push(s),
            Err(e) => {
                select(Some(i));
                error_box(Some(hwnd), &format!("{}\n{e}", row.list_text()));
                return;
            }
        }
    }
    if let Err(e) = config.save(&path) {
        error_box(Some(hwnd), &e.to_string());
        return;
    }
    let autostart = is_checked(c.autostart);
    if autostart != autostart_was
        && let Err(e) = autostart::set_enabled(autostart)
    {
        error_box(Some(hwnd), &e);
    }
    let _ = unsafe { DestroyWindow(hwnd) };
}

fn cancel() {
    let Some((hwnd, c, dirty, autostart_was)) =
        with_state(|s| (s.hwnd, s.c, s.dirty, s.autostart_was))
    else {
        return;
    };
    let changed = dirty || is_checked(c.autostart) != autostart_was;
    if changed
        && message_box(
            Some(hwnd),
            "変更を保存せずに閉じますか?",
            MB_YESNO | MB_ICONWARNING,
        ) == IDNO.0
    {
        return;
    }
    let _ = unsafe { DestroyWindow(hwnd) };
}

fn preview_rect(dpi: u32) -> RECT {
    let [x, y, w, h] = PREVIEW;
    RECT {
        left: scale(x, dpi),
        top: scale(y, dpi),
        right: scale(x + w, dpi),
        bottom: scale(y + h, dpi),
    }
}

fn invalidate_preview() {
    let Some(hwnd) = WINDOW.get() else {
        return;
    };
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    let rect = preview_rect(dpi);
    let _ = unsafe { InvalidateRect(Some(hwnd), Some(&rect), true) };
}

/// Draws the work area of the monitor the window is on, scaled into the
/// preview box, and where the selected shortcut would put a window on it.
fn paint(hwnd: HWND) {
    let row = with_state(|s| s.current.and_then(|i| s.rows.get(i).cloned())).flatten();
    let mut ps = PAINTSTRUCT::default();
    let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
    let dpi = unsafe { windows::Win32::UI::HiDpi::GetDpiForWindow(hwnd) };
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    let has_monitor = unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
    let work = info.rcWork;
    let (ww, wh) = (work.right - work.left, work.bottom - work.top);
    if has_monitor && ww > 0 && wh > 0 {
        let bounds = preview_rect(dpi);
        let (bw, bh) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
        let k = (f64::from(bw) / f64::from(ww)).min(f64::from(bh) / f64::from(wh));
        let screen = RECT {
            left: bounds.left,
            top: bounds.top,
            right: bounds.left + (f64::from(ww) * k) as i32,
            bottom: bounds.top + (f64::from(wh) * k) as i32,
        };
        unsafe {
            FillRect(hdc, &screen, GetSysColorBrush(COLOR_WINDOW));
        }
        if let Some(placement) = row.and_then(|r| r.placement().ok()) {
            let local = Rect {
                left: 0,
                top: 0,
                right: ww,
                bottom: wh,
            };
            let r = placement.resolve(local);
            let map_x = |v: i32| screen.left + (f64::from(v) * k).round() as i32;
            let map_y = |v: i32| screen.top + (f64::from(v) * k).round() as i32;
            let window = RECT {
                left: map_x(r.left),
                top: map_y(r.top),
                right: map_x(r.right),
                bottom: map_y(r.bottom),
            };
            unsafe {
                FillRect(hdc, &window, GetSysColorBrush(COLOR_HIGHLIGHT));
            }
        }
        unsafe {
            FrameRect(hdc, &screen, GetSysColorBrush(COLOR_GRAYTEXT));
        }
    }
    let _ = unsafe { EndPaint(hwnd, &ps) };
}

extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = loword(wparam.0) as i32;
            let code = hiword(wparam.0);
            match (id, code) {
                (ID_LIST, LBN_SELCHANGE) => {
                    let Some(c) = with_state(|s| s.c) else {
                        return LRESULT(0);
                    };
                    let sel = send(c.list, LB_GETCURSEL, 0, 0);
                    select(usize::try_from(sel).ok());
                }
                (ID_ADD, BN_CLICKED) => insert_row(Draft::new()),
                (ID_DUPLICATE, BN_CLICKED) => {
                    let row = with_state(|s| s.current.map(|i| s.rows[i].clone())).flatten();
                    if let Some(row) = row {
                        insert_row(row);
                    }
                }
                (ID_DELETE, BN_CLICKED) => delete_row(),
                (ID_UP, BN_CLICKED) => move_row(true),
                (ID_DOWN, BN_CLICKED) => move_row(false),
                (ID_WIDTH | ID_HEIGHT | ID_HOTKEY, EN_CHANGE)
                | (ID_WIN, BN_CLICKED)
                | (ID_ANCHOR, CBN_SELCHANGE) => on_form_changed(),
                (id, BN_CLICKED) if id == IDOK.0 => save(),
                (id, BN_CLICKED) if id == IDCANCEL.0 => cancel(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            cancel();
            LRESULT(0)
        }
        WM_PAINT => {
            paint(hwnd);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            apply_dpi(loword(wparam.0));
            // SAFETY: WM_DPICHANGED carries the suggested window rectangle.
            let r = unsafe { *(lparam.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                let _ = InvalidateRect(Some(hwnd), None, true);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            WINDOW.set(None);
            let state = STATE.with(|s| s.borrow_mut().take());
            if let Some(s) = state {
                if !s.font.is_invalid() {
                    let _ = unsafe { DeleteObject(s.font.into()) };
                }
                let _ = unsafe {
                    PostMessageW(Some(s.owner), WM_APP_SETTINGS_CLOSED, WPARAM(0), LPARAM(0))
                };
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
