//! The settings window, in WinUI 3. It runs in a process of its own
//! (`winwin.exe --settings`, see settings.rs): a list of shortcuts on the
//! left, the selected one's fields on the right.
//!
//! Every edit goes into the [`Editor`] straight away; nothing is checked
//! until 保存, which turns the rows into a config, writes it and closes. The
//! resident part reads the file again when this process ends.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint,
};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GetClassNameW, GetCursorPos, WM_CLOSE,
};
use windows::core::BOOL;
use windows_reactor::*;

use super::{autostart, config_path, error_box};
use crate::config::Config;
use crate::draft::{Draft, Editor};
use crate::hotkey::{self, MOD_ALT, MOD_CONTROL, MOD_SHIFT, MOD_WIN};
use crate::layout::{Anchor, Rect};

/// The client area in DIPs.
const CLIENT: (f64, f64) = (820.0, 560.0);
/// The picture of a placement at the left of each line in the list, and the
/// larger one under the fields.
const THUMBNAIL: (f64, f64) = (40.0, 24.0);
const PREVIEW: (f64, f64) = (360.0, 200.0);
/// Pictures are worked out in whole units this many times finer than a DIP,
/// so that a small one does not snap to whole DIPs.
const SUBPIXEL: f64 = 4.0;

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";

const MODIFIERS: [(u32, &str); 4] = [
    (MOD_CONTROL, "Ctrl"),
    (MOD_ALT, "Alt"),
    (MOD_SHIFT, "Shift"),
    (MOD_WIN, "Win"),
];

/// Runs the settings window until it closes.
pub fn run() {
    let path = config_path();
    // The resident part opens this window only after reading the file, so
    // it reads; if it has changed since, the error is worth showing.
    let config = match Config::load_or_create(&path) {
        Ok(config) => config,
        Err(e) => {
            error_box(None, &e.to_string());
            return;
        }
    };
    let input = Input {
        rows: config.shortcuts.iter().map(Draft::from_shortcut).collect(),
        autostart: autostart::is_enabled(),
        work: work_size(),
    };
    if let Err(e) = App::run_component::<Settings>(input) {
        error_box(None, &format!("設定を開けませんでした。\n{e}"));
    }
}

/// The size of the work area of the monitor the pointer is on, which
/// previews are drawn to the shape of. That is where the window opens.
fn work_size() -> (i32, i32) {
    let mut cursor = POINT::default();
    let _ = unsafe { GetCursorPos(&mut cursor) };
    let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool() {
        let w = info.rcWork;
        (w.right - w.left, w.bottom - w.top)
    } else {
        (16, 9)
    }
}

#[derive(Clone, PartialEq)]
struct Input {
    rows: Vec<Draft>,
    autostart: bool,
    work: (i32, i32),
}

struct Settings {
    editor: Editor,
    autostart: bool,
    autostart_was: bool,
    /// Shown above everything when 保存 fails.
    error: Option<String>,
    /// The 変更を破棄しますか dialog is open.
    confirming: bool,
}

#[derive(Clone)]
enum Msg {
    Select(Option<usize>),
    Add,
    Duplicate,
    Delete,
    Move {
        up: bool,
    },
    Modifier(u32, bool),
    Key(Option<usize>),
    Anchor(Option<usize>),
    Width(String),
    Height(String),
    Autostart(bool),
    Save,
    /// キャンセル, or the window's close button.
    Cancel,
    Confirmed(ContentDialogResult),
    DismissError,
}

thread_local! {
    /// How the window's close button reaches the component (see
    /// `intercept_close`).
    static SENDER: RefCell<Option<LocalSender<Msg>>> = const { RefCell::new(None) };
    /// Set once the component has decided to close, so that the close it
    /// asks for is let through.
    static CLOSING: Cell<bool> = const { Cell::new(false) };
}

impl Component for Settings {
    type Input = Input;
    type Message = Msg;

    fn create(input: &Input, context: &ComponentContext<Self>) -> Self {
        SENDER.with(|s| *s.borrow_mut() = Some(context.sender()));
        Settings {
            editor: Editor::new(input.rows.clone()),
            autostart: input.autostart,
            autostart_was: input.autostart,
            error: None,
            confirming: false,
        }
    }

    fn update(&mut self, msg: Msg, context: &ComponentContext<Self>) {
        match msg {
            Msg::Select(Some(i)) => self.editor.select(i),
            Msg::Select(None) => {}
            Msg::Add => self.editor.add(),
            Msg::Duplicate => self.editor.duplicate(),
            Msg::Delete => self.editor.delete(),
            Msg::Move { up } => self.editor.move_selected(up),
            Msg::Modifier(m, on) => self.editor.edit(|d| d.set_modifier(m, on)),
            Msg::Key(Some(i)) => self.editor.edit(|d| {
                if let Some(&vk) = d.key_choices().get(i) {
                    d.vk = vk;
                }
            }),
            Msg::Anchor(Some(i)) => self.editor.edit(|d| {
                if let Some(&a) = Anchor::ALL.get(i) {
                    d.anchor = a;
                }
            }),
            Msg::Key(None) | Msg::Anchor(None) => {}
            Msg::Width(text) => self.editor.edit(|d| d.width = text),
            Msg::Height(text) => self.editor.edit(|d| d.height = text),
            Msg::Autostart(on) => self.autostart = on,
            Msg::Save => self.save(context),
            Msg::Cancel => {
                if self.is_changed() {
                    self.confirming = true;
                } else {
                    close(context);
                }
            }
            Msg::Confirmed(result) => {
                self.confirming = false;
                if result == ContentDialogResult::Primary {
                    close(context);
                }
            }
            Msg::DismissError => self.error = None,
        }
    }

    fn view(&self, input: &Input, context: &mut ViewContext<Self>) -> View {
        context.window_title("winwin の設定");
        context.window_visuals(
            WindowVisuals::new()
                .backdrop(WindowBackdrop::Mica)
                .client_size(CLIENT.0, CLIENT.1)
                .constraints(WindowConstraints {
                    min_width: Some(CLIENT.0),
                    min_height: Some(CLIENT.1),
                    ..Default::default()
                }),
        );
        context.use_effect("intercept-close", (), || {
            intercept_close();
            None
        });

        let error = InfoBar::new()
            .severity(InfoBarSeverity::Error)
            .title("保存できませんでした")
            .message(self.error.clone().unwrap_or_default())
            .is_open(self.error.is_some())
            .is_closable(true)
            .on_closed(context.message(Msg::DismissError))
            .grid_row(0)
            .margin(Thickness::new(0.0, 0.0, 0.0, 16.0));

        let body = Grid::new()
            .columns([GridLength::Pixel(340.0), GridLength::STAR])
            .column_spacing(24.0)
            .grid_row(1)
            .children((self.list(input, context), self.form(input, context)));

        let footer = Grid::new()
            .columns([GridLength::STAR, GridLength::Auto])
            .grid_row(2)
            .margin(Thickness::new(0.0, 16.0, 0.0, 0.0))
            .children((
                CheckBox::new()
                    .is_checked(self.autostart)
                    .on_is_checked_changed(context.callback(Msg::Autostart))
                    .vertical_alignment(VerticalAlignment::Center)
                    .content("Windows へのサインイン時に winwin を起動する"),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .grid_column(1)
                    .children((
                        Button::new()
                            .style(ButtonStyle::Accent)
                            .min_width(96.0)
                            .on_click(context.message(Msg::Save))
                            .content("保存"),
                        Button::new()
                            .min_width(96.0)
                            .on_click(context.message(Msg::Cancel))
                            .content("キャンセル"),
                    )),
            ));

        let confirm = ContentDialog::new()
            .is_open(self.confirming)
            .title("変更を保存せずに閉じますか?")
            .primary_button_text("閉じる")
            .close_button_text("戻る")
            .on_closed(context.callback(Msg::Confirmed))
            .content("保存していない変更は失われます。");

        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR, GridLength::Auto])
            .margin(Thickness::uniform(24.0))
            .children((error, body, footer, confirm))
    }
}

impl Settings {
    fn is_changed(&self) -> bool {
        self.editor.is_dirty() || self.autostart != self.autostart_was
    }

    fn save(&mut self, context: &ComponentContext<Self>) {
        let config = match self.editor.build_config() {
            Ok(config) => config,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        if let Err(e) = config.save(&config_path()) {
            self.error = Some(e.to_string());
            return;
        }
        if self.autostart != self.autostart_was
            && let Err(e) = autostart::set_enabled(self.autostart)
        {
            self.error = Some(e);
            return;
        }
        close(context);
    }

    /// The shortcuts, each with a picture of where it puts a window, and the
    /// buttons that change the list.
    fn list(&self, input: &Input, context: &mut ViewContext<Self>) -> View {
        let items = self.editor.rows().map(|(id, row)| {
            let line = Grid::new()
                .columns([GridLength::Auto, GridLength::STAR])
                .column_spacing(12.0)
                .margin(Thickness::xy(0.0, 6.0))
                .children((
                    Border::new()
                        .vertical_alignment(VerticalAlignment::Center)
                        .content(picture(row, input.work, THUMBNAIL, Anchor::Center)),
                    TextBlock::new()
                        .text(row.list_text())
                        .text_trimming(TextTrimming::CharacterEllipsis)
                        .vertical_alignment(VerticalAlignment::Center)
                        .grid_column(1),
                ));
            (id, ListViewItem::new().content(line))
        });
        let list = Border::new()
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(Thickness::uniform(1.0))
            .corner_radius(CornerRadius::uniform(8.0))
            .padding(Thickness::uniform(4.0))
            .content(
                ListView::new()
                    .selection_mode(ListViewSelectionMode::Single)
                    .selected_index(self.editor.current())
                    .on_selection_changed(context.callback(Msg::Select))
                    .collection_slot(ListViewSlot::Items, items),
            );

        let selected = self.editor.current().is_some();
        let button = |label: &str, msg: Msg, enabled: bool| {
            Button::new()
                .is_enabled(enabled)
                .horizontal_alignment(HorizontalAlignment::Stretch)
                .on_click(context.message(msg))
                .content(label.to_string())
        };
        let buttons = Grid::new()
            .columns([GridLength::STAR; 5])
            .column_spacing(6.0)
            .grid_row(1)
            .margin(Thickness::new(0.0, 12.0, 0.0, 0.0))
            .children((
                button("追加", Msg::Add, true),
                Border::new()
                    .grid_column(1)
                    .content(button("複製", Msg::Duplicate, selected)),
                Border::new()
                    .grid_column(2)
                    .content(button("削除", Msg::Delete, selected)),
                Border::new().grid_column(3).content(button(
                    "上へ",
                    Msg::Move { up: true },
                    self.editor.can_move(true),
                )),
                Border::new().grid_column(4).content(button(
                    "下へ",
                    Msg::Move { up: false },
                    self.editor.can_move(false),
                )),
            ));

        Grid::new()
            .rows([GridLength::STAR, GridLength::Auto])
            .children((list, buttons))
    }

    /// The selected shortcut's fields and a preview of where it puts a
    /// window. With nothing selected the fields are empty and disabled.
    fn form(&self, input: &Input, context: &mut ViewContext<Self>) -> View {
        let blank = Draft::new();
        let enabled = self.editor.selected().is_some();
        let d = self.editor.selected().unwrap_or(&blank);

        let modifiers = MODIFIERS.map(|(m, label)| {
            ToggleButton::new()
                .is_enabled(enabled)
                .is_checked(d.modifiers & m != 0)
                .min_width(56.0)
                .on_is_checked_changed(context.callback(move |on| Msg::Modifier(m, on)))
                .content(label)
        });
        let keys = d.key_choices();
        let key = ComboBox::new()
            .is_enabled(enabled)
            .placeholder_text("キー")
            .items_source(keys.iter().map(|&vk| hotkey::key_name(vk)))
            .selected_index(keys.iter().position(|&vk| vk == d.vk))
            .min_width(120.0)
            .on_selection_changed(context.callback(Msg::Key));
        let shortcut = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(4.0)
            .children((
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(4.0)
                    .children(modifiers),
                TextBlock::new()
                    .text("+")
                    .vertical_alignment(VerticalAlignment::Center)
                    .margin(Thickness::xy(4.0, 0.0)),
                key,
            ));

        let anchor = ComboBox::new()
            .is_enabled(enabled)
            .items_source(Anchor::ALL.map(Anchor::label))
            .selected_index(Anchor::ALL.iter().position(|a| *a == d.anchor))
            .min_width(160.0)
            .on_selection_changed(context.callback(Msg::Anchor));

        let ratio = |value: &str, on_change: fn(String) -> Msg| {
            TextBox::new()
                .is_enabled(enabled)
                .text(value.to_string())
                .placeholder_text("1/2")
                .width(120.0)
                .on_text_changed(context.callback(on_change))
        };
        let size = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(24.0)
            .children((
                field("幅", ratio(&d.width, Msg::Width)),
                field("高さ", ratio(&d.height, Msg::Height)),
            ));

        let preview = Border::new()
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(Thickness::uniform(1.0))
            .corner_radius(CornerRadius::uniform(8.0))
            .padding(Thickness::uniform(12.0))
            .horizontal_alignment(HorizontalAlignment::Left)
            .content(if enabled {
                picture(d, input.work, PREVIEW, Anchor::TopLeft)
            } else {
                Canvas::new().width(PREVIEW.0).height(PREVIEW.1).into()
            });

        StackPanel::new().spacing(16.0).grid_column(1).children((
            field("ショートカット", shortcut),
            field("基準位置", anchor),
            size,
            TextBlock::new()
                .text(
                    "幅と高さは、画面(タスクバーを除く)に対する比率で指定します。\
                         例: 1/2(半分)、2/3、0.75、1(全体)",
                )
                .text_wrapping(TextWrapping::Wrap)
                .opacity(0.7),
            field("プレビュー", preview),
        ))
    }
}

/// A label above a control.
fn field(label: &str, control: impl Into<View>) -> View {
    StackPanel::new()
        .spacing(6.0)
        .children((TextBlock::new().text(label.to_string()), control.into()))
}

/// A screen with the shape of the work area, shrunk to `size` DIPs, and on
/// it where `row` puts a window. A row whose fields do not make a placement
/// yet gets the screen alone.
fn picture(row: &Draft, work: (i32, i32), size: (f64, f64), anchor: Anchor) -> View {
    let bounds = Rect {
        left: 0,
        top: 0,
        right: (size.0 * SUBPIXEL) as i32,
        bottom: (size.1 * SUBPIXEL) as i32,
    };
    let canvas = Canvas::new().width(size.0).height(size.1);
    let Some((screen, window)) = row.picture(work, bounds, anchor) else {
        return canvas.into();
    };
    let rect = |r: Rect, fill: ThemeBrush| {
        Border::new()
            .background(fill)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(Thickness::uniform(1.0))
            .corner_radius(CornerRadius::uniform(2.0))
            .width(f64::from(r.width()) / SUBPIXEL)
            .height(f64::from(r.height()) / SUBPIXEL)
            .canvas_left(f64::from(r.left) / SUBPIXEL)
            .canvas_top(f64::from(r.top) / SUBPIXEL)
    };
    let window: View = match window {
        Some(w) => rect(w, ThemeBrush::Accent).into(),
        None => View::empty(),
    };
    canvas.children((rect(screen, ThemeBrush::SolidBackground), window))
}

fn close(context: &ComponentContext<Settings>) {
    CLOSING.set(true);
    let _ = context.window().request_close();
}

/// Routes the window's close button through [`Msg::Cancel`], so that closing
/// with unsaved changes asks first, as キャンセル does. Reactor does not
/// hand out its window, so this finds it among the thread's windows by the
/// class WinUI 3 gives a desktop window. Should that fail, the close button
/// simply closes.
fn intercept_close() {
    unsafe extern "system" fn find(hwnd: HWND, found: LPARAM) -> BOOL {
        let mut class = [0u16; 64];
        let len = unsafe { GetClassNameW(hwnd, &mut class) }.max(0) as usize;
        if String::from_utf16_lossy(&class[..len]) == WINUI_WINDOW_CLASS {
            unsafe { *(found.0 as *mut HWND) = hwnd };
            return false.into();
        }
        true.into()
    }
    let mut hwnd = HWND::default();
    let _ = unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(find),
            LPARAM(&mut hwnd as *mut HWND as isize),
        )
    };
    if !hwnd.is_invalid() {
        let _ = unsafe { SetWindowSubclass(hwnd, Some(subclass), 1, 0) };
    }
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if msg == WM_CLOSE && !CLOSING.get() {
        let sent = SENDER.with(|s| s.borrow().as_ref().is_some_and(|s| s.send(Msg::Cancel)));
        if sent {
            return LRESULT(0);
        }
    }
    unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
}
