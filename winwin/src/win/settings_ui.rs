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
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GetClassNameW, GetCursorPos, ICON_BIG, ICON_SMALL, SendMessageW, WM_CLOSE,
    WM_SETICON,
};
use windows::core::BOOL;
use windows_reactor::*;

use super::{autostart, config_path, error_box, icon, keyhook};
use crate::config::{Config, Theme};
use crate::draft::{Draft, Editor};
use crate::hotkey::{self, Arrow, Keycap, Recorder};
use crate::layout::{Anchor, Rect};

/// The client area in DIPs, which is also as small as the window goes: the
/// fields and the whole preview fit in it.
const CLIENT: (f64, f64) = (900.0, 690.0);
/// The picture of a placement at the left of each line in the list, and the
/// larger one under the fields.
const THUMBNAIL: (f64, f64) = (40.0, 24.0);
const PREVIEW: (f64, f64) = (360.0, 200.0);
/// Pictures are worked out in whole units this many times finer than a DIP,
/// so that a small one does not snap to whole DIPs.
const SUBPIXEL: f64 = 4.0;

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";

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
        theme: config.theme,
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
    theme: Theme,
    work: (i32, i32),
}

struct Settings {
    editor: Editor,
    autostart: bool,
    autostart_was: bool,
    /// Applied to the window as soon as it is chosen; saved with the rest.
    theme: Theme,
    theme_was: Theme,
    /// Shown above everything when 保存 fails.
    error: Option<String>,
    /// The 変更を破棄しますか dialog is open.
    confirming: bool,
    /// The ショートカットを設定 dialog is open, with what it has recorded.
    recording: Option<Recorder>,
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
    /// 変更 beside the shortcut: opens the dialog that records one.
    Record,
    /// A key went down or up while recording.
    Key(u32, bool),
    RecordReset,
    RecordClear,
    RecordClosed(ContentDialogResult),
    Anchor(Option<usize>),
    Width(String),
    Height(String),
    Autostart(bool),
    Theme(Option<usize>),
    Save,
    /// キャンセル, or the window's close button.
    Cancel,
    Confirmed(ContentDialogResult),
    DismissError,
}

thread_local! {
    /// How the window's close button reaches the component (see
    /// `adopt_window`).
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
            theme: input.theme,
            theme_was: input.theme,
            error: None,
            confirming: false,
            recording: None,
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
            Msg::Record => self.start_recording(context),

            Msg::Key(vk, down) => {
                if let Some(r) = &mut self.recording {
                    if down {
                        r.key_down(vk);
                    } else {
                        r.key_up(vk);
                    }
                }
            }
            Msg::RecordReset => {
                if let (Some(r), Some(d)) = (&mut self.recording, self.editor.selected()) {
                    *r = Recorder::showing(d.modifiers, d.vk);
                }
            }
            Msg::RecordClear => {
                if let Some(r) = &mut self.recording {
                    *r = Recorder::default();
                }
            }
            Msg::RecordClosed(result) => {
                keyhook::stop();
                let recorded = self.recording.take().and_then(|r| r.result());
                if result == ContentDialogResult::Primary
                    && let Some(Ok(keys)) = recorded
                {
                    self.editor.edit(|d| {
                        d.modifiers = keys.modifiers;
                        d.vk = keys.vk;
                    });
                }
            }
            Msg::Anchor(Some(i)) => self.editor.edit(|d| {
                if let Some(&a) = Anchor::ALL.get(i) {
                    d.anchor = a;
                }
            }),
            Msg::Anchor(None) => {}
            Msg::Width(text) => self.editor.edit(|d| d.width = text),
            Msg::Height(text) => self.editor.edit(|d| d.height = text),
            Msg::Autostart(on) => self.autostart = on,
            Msg::Theme(i) => {
                if let Some(&theme) = i.and_then(|i| Theme::ALL.get(i)) {
                    self.theme = theme;
                }
            }
            Msg::Save => self.save(context),
            // The close button while the recording dialog is open: WinUI
            // shows one dialog at a time, and that one has its own キャンセル.
            Msg::Cancel if self.recording.is_some() => {}
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
                .theme(match self.theme {
                    Theme::System => WindowTheme::System,
                    Theme::Light => WindowTheme::Light,
                    Theme::Dark => WindowTheme::Dark,
                })
                .client_size(CLIENT.0, CLIENT.1)
                .constraints(WindowConstraints {
                    min_width: Some(CLIENT.0),
                    min_height: Some(CLIENT.1),
                    ..Default::default()
                }),
        );
        context.use_effect("adopt-window", (), || {
            adopt_window();
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
            .margin(Thickness::new(
                0.0,
                0.0,
                0.0,
                if self.error.is_some() { 16.0 } else { 0.0 },
            ));

        let body = Grid::new()
            .columns([GridLength::Pixel(420.0), GridLength::STAR])
            .column_spacing(24.0)
            .grid_row(1)
            .children((self.list(input, context), self.form(input, context)));

        let footer = Grid::new()
            .columns([GridLength::STAR, GridLength::Auto])
            .grid_row(2)
            .margin(Thickness::new(0.0, 16.0, 0.0, 0.0))
            .children((
                StackPanel::new().spacing(12.0).children((
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(8.0)
                        .children((
                            TextBlock::new()
                                .text("テーマ")
                                .vertical_alignment(VerticalAlignment::Center),
                            ComboBox::new()
                                .items_source(Theme::ALL.map(Theme::label))
                                .selected_index(Theme::ALL.iter().position(|t| *t == self.theme))
                                .on_selection_changed(context.callback(Msg::Theme)),
                        )),
                    CheckBox::new()
                        .is_checked(self.autostart)
                        .on_is_checked_changed(context.callback(Msg::Autostart))
                        .content("Windows へのサインイン時に winwin を起動する"),
                )),
                StackPanel::new()
                    .orientation(Orientation::Horizontal)
                    .spacing(8.0)
                    .grid_column(1)
                    .vertical_alignment(VerticalAlignment::Bottom)
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
            .children((error, body, footer, confirm, self.recorder(context)))
    }
}

impl Settings {
    fn start_recording(&mut self, context: &ComponentContext<Self>) {
        let Some(d) = self.editor.selected() else {
            return;
        };
        let sender = context.sender();
        if let Err(e) = keyhook::start(move |vk, down| {
            sender.send(Msg::Key(vk, down));
        }) {
            self.error = Some(format!("キー入力を受け取れませんでした。{e}"));
            return;
        }
        self.recording = Some(Recorder::showing(d.modifiers, d.vk));
    }

    /// The dialog that records a shortcut: it shows the keys as they are
    /// pressed, and saves only a combination that can be a shortcut.
    fn recorder(&self, context: &mut ViewContext<Self>) -> View {
        let r = self.recording.unwrap_or_default();
        let result = r.result();
        let caps = hotkey::keycaps(r.modifiers, r.vk);
        let keys: View = if caps.is_empty() {
            TextBlock::new()
                .text("キーを押してください")
                .opacity(0.6)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .into()
        } else {
            keycaps(caps, true)
        };
        let problem = match &result {
            Some(Err(e)) => e.to_string(),
            _ => String::new(),
        };
        let link = |label: &str, msg: Msg| {
            HyperlinkButton::new()
                .on_click(context.message(msg))
                .content(label.to_string())
        };
        ContentDialog::new()
            .is_open(self.recording.is_some())
            .title("ショートカットを設定")
            .primary_button_text("保存")
            .close_button_text("キャンセル")
            .is_primary_button_enabled(matches!(result, Some(Ok(_))))
            .on_closed(context.callback(Msg::RecordClosed))
            .content(
                StackPanel::new().spacing(16.0).width(400.0).children((
                    TextBlock::new()
                        .text(
                            "設定するキーの組み合わせを押してください。\nCtrl・Alt・Win のいずれかを含める必要があります。",
                        )
                        .text_wrapping(TextWrapping::Wrap),
                    Border::new()
                        .background(ThemeBrush::CardBackground)
                        .border_brush(ThemeBrush::CardStroke)
                        .border_thickness(Thickness::uniform(1.0))
                        .corner_radius(CornerRadius::uniform(8.0))
                        .padding(Thickness::uniform(24.0))
                        .min_height(112.0)
                        .content(keys),
                    TextBlock::new()
                        .text(problem)
                        .foreground(ThemeBrush::SystemCritical)
                        .horizontal_alignment(HorizontalAlignment::Center),
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(24.0)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .children((
                            link("リセット", Msg::RecordReset),
                            link("クリア", Msg::RecordClear),
                        )),
                )),
            )
    }

    fn is_changed(&self) -> bool {
        self.editor.is_dirty()
            || self.autostart != self.autostart_was
            || self.theme != self.theme_was
    }

    fn save(&mut self, context: &ComponentContext<Self>) {
        let mut config = match self.editor.build_config() {
            Ok(config) => config,
            Err(e) => {
                self.error = Some(e);
                return;
            }
        };
        config.theme = self.theme;
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
            // The keys at the left; where they put the window, in words and
            // as a picture, at the right.
            let line = Grid::new()
                .columns([GridLength::STAR, GridLength::Auto, GridLength::Auto])
                .column_spacing(12.0)
                .margin(Thickness::xy(0.0, 6.0))
                .children((
                    Border::new()
                        .vertical_alignment(VerticalAlignment::Center)
                        .content(shortcut_view(row)),
                    TextBlock::new()
                        .text(row.placement_text())
                        .vertical_alignment(VerticalAlignment::Center)
                        .grid_column(1),
                    Border::new()
                        .vertical_alignment(VerticalAlignment::Center)
                        .grid_column(2)
                        .content(picture(row, input.work, THUMBNAIL, Anchor::Center)),
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

        let shortcut = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(12.0)
            .children((
                shortcut_view(d),
                Button::new()
                    .is_enabled(enabled)
                    .on_click(context.message(Msg::Record))
                    .content(
                        StackPanel::new()
                            .orientation(Orientation::Horizontal)
                            .spacing(8.0)
                            .children((FontIcon::new().glyph("\u{E70F}"), "変更")),
                    ),
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
            field("プレビュー", preview),
        ))
    }
}

/// A row's shortcut as keycaps, or 未設定.
fn shortcut_view(d: &Draft) -> View {
    if d.vk == 0 {
        TextBlock::new()
            .text("未設定")
            .opacity(0.6)
            .vertical_alignment(VerticalAlignment::Center)
            .into()
    } else {
        keycaps(hotkey::keycaps(d.modifiers, d.vk), false)
    }
}

/// Keys drawn as keycaps in a row, accent colored as Windows' own settings
/// draw shortcuts. `large` is the recording dialog's, big enough to read at
/// a glance. The caps are pictures, not buttons: they do nothing when
/// clicked.
fn keycaps(caps: Vec<Keycap>, large: bool) -> View {
    let (height, min_width, text_size, radius) = if large {
        (56.0, 64.0, 18.0, 6.0)
    } else {
        (32.0, 36.0, 14.0, 4.0)
    };
    let caps = caps.into_iter().enumerate().map(|(i, cap)| {
        let face: View = match cap {
            Keycap::Arrow(arrow) => chevron(arrow, text_size),
            _ => TextBlock::new()
                .text(cap.label().unwrap_or_default())
                .font_size(text_size)
                .foreground(ON_ACCENT)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        };
        let cap = Border::new()
            .background(ThemeBrush::Accent)
            .corner_radius(CornerRadius::uniform(radius))
            .padding(Thickness::xy(10.0, 0.0))
            .height(height)
            .min_width(min_width)
            .content(face);
        (i, cap)
    });
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(if large { 12.0 } else { 4.0 })
        .horizontal_alignment(if large {
            HorizontalAlignment::Center
        } else {
            HorizontalAlignment::Left
        })
        .vertical_alignment(VerticalAlignment::Center)
        .keyed_children(caps)
}

/// Text on an accent-colored cap. The window's base background is dark in
/// the dark theme and light in the light one, the opposite of the accent
/// fill, which is what text on it needs.
const ON_ACCENT: ThemeBrush = ThemeBrush::SolidBackground;

/// An arrow key's cap face: a chevron as tall as the text on the other caps,
/// drawn as two strokes so that it takes the same color as that text.
fn chevron(arrow: Arrow, text_size: f64) -> View {
    // Half the chevron's length along the way it points, and its reach
    // across that.
    let size = text_size * 0.7;
    let (depth, reach) = (size * 0.25, size * 0.5);
    let (cx, cy) = (size / 2.0, size / 2.0);
    // The tip, then the two ends, for a chevron pointing right.
    let points = [(depth, 0.0), (-depth, -reach), (-depth, reach)];
    let turn = |(x, y): (f64, f64)| match arrow {
        Arrow::Right => (cx + x, cy + y),
        Arrow::Left => (cx - x, cy + y),
        Arrow::Down => (cx + y, cy + x),
        Arrow::Up => (cx + y, cy - x),
    };
    let [tip, a, b] = points.map(turn);
    let stroke = |from: (f64, f64)| {
        Line::new()
            .x1(from.0)
            .y1(from.1)
            .x2(tip.0)
            .y2(tip.1)
            .stroke(ON_ACCENT)
            .stroke_thickness(text_size / 9.0)
    };
    Canvas::new()
        .width(size)
        .height(size)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .children((stroke(a), stroke(b)))
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

/// Does to the window what Reactor has no way to say. It gets winwin's icon,
/// the one in the notification area, on its title bar and taskbar button.
/// And its close button goes through [`Msg::Cancel`], so that closing with
/// unsaved changes asks first, as キャンセル does. Reactor does not hand out
/// its window, so this finds it among the thread's windows by the class
/// WinUI 3 gives a desktop window. Should that fail, the window keeps the
/// default icon and the close button simply closes.
fn adopt_window() {
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
    if hwnd.is_invalid() {
        return;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    for (kind, icon) in [
        (ICON_SMALL, icon::create(dpi)),
        (ICON_BIG, icon::create_large(dpi)),
    ] {
        unsafe {
            SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(kind as usize)),
                Some(LPARAM(icon.0 as isize)),
            )
        };
    }
    let _ = unsafe { SetWindowSubclass(hwnd, Some(subclass), 1, 0) };
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
