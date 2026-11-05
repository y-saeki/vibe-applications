//! The settings window, in WinUI 3. It runs in a process of its own
//! (`meta-gesture.exe --settings`, see settings.rs): the sets at the left,
//! the selected set's commands beside them, and at the right the set's
//! applications and the selected command's gesture and keys.
//!
//! Every edit goes into the [`Editor`] straight away; nothing is checked
//! until 保存して閉じる, which writes the file and closes. The resident part
//! reads the file again when this process ends.

use std::cell::{Cell, RefCell};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GetClassNameW, ICON_BIG, ICON_SMALL, SendMessageW, WM_CLOSE, WM_SETICON,
};
use windows::core::BOOL;
use windows_reactor::*;

use super::filedialog::{self, Choose};
use super::{autostart, config_path, error_box, icon, keyhook, process};
use meta_gesture::config::{Action, App as Application, Config};
use meta_gesture::editor::Editor;
use meta_gesture::gesture::Direction;
use meta_gesture::keys::{Arrow, Keycap, Look, Recorder};
use meta_gesture::preset;
use meta_gesture::vk;

/// The client area in DIPs, which is also as small as the window goes.
const CLIENT: (f64, f64) = (1120.0, 720.0);
const SETS_WIDTH: f64 = 240.0;
const COMMANDS_WIDTH: f64 = 280.0;
/// The column of a line of keys that names the application.
const KEY_LABEL_WIDTH: f64 = 170.0;

const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";

/// The titles of the bar that says why something failed.
const SAVE_FAILED: &str = "保存できませんでした";
const EXPORT_FAILED: &str = "エクスポートできませんでした";
const IMPORT_FAILED: &str = "インポートできませんでした";
const ADD_FAILED: &str = "アプリを追加できませんでした";

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
        config,
        autostart: autostart::is_enabled(),
        running: process::running(),
    };
    if let Err(e) = App::run_component::<Settings>(input) {
        error_box(None, &format!("設定を開けませんでした。\n{e}"));
    }
}

#[derive(Clone, PartialEq)]
struct Input {
    config: Config,
    autostart: bool,
    /// The applications with a window open when the settings opened, to
    /// add to a set.
    running: Vec<Application>,
}

struct Settings {
    editor: Editor,
    autostart: bool,
    autostart_was: bool,
    /// Shown above everything when something fails: what failed, and why.
    error: Option<(&'static str, String)>,
    /// The 変更を保存せずに閉じますか dialog is open.
    confirming: bool,
    /// The キーを設定 dialog is open for the line of keys `.0`, with what
    /// it has recorded.
    recording: Option<(usize, Recorder)>,
    /// The applications with a window open when the settings opened.
    running: Vec<Application>,
}

#[derive(Clone)]
enum Msg {
    SelectSet(Option<usize>),
    AddSet,
    /// One of `preset::ALL` was chosen to add as a set.
    AddPreset(Option<usize>),
    DeleteSet,
    MoveSet(bool),
    RenameSet(String),
    /// One of the running applications was chosen to add.
    AddRunning(Option<usize>),
    /// 参照...: asks for an executable to add.
    Browse,
    Browsed(Result<Option<Application>, String>),
    RemoveApp(usize),
    SelectCommand(Option<usize>),
    AddCommand,
    DeleteCommand,
    MoveCommand(bool),
    RenameCommand(String),
    Push(Direction),
    Pop,
    ClearGesture,
    /// 変更 on a line of keys: opens the dialog that records them.
    Record(usize),
    /// A key went down or up while recording: its virtual-key code.
    Key(u32, bool),
    RecordClear,
    RecordClosed(ContentDialogResult),
    ClearKeys(usize),
    Autostart(bool),
    Save,
    Export,
    Exported(Result<(), String>),
    Import,
    Imported(Result<Option<Config>, String>),
    /// キャンセル, or the window's close button.
    Cancel,
    /// The 変更を保存せずに閉じますか dialog was answered: true to close
    /// without saving, false to go back.
    Discard(bool),
    DismissError,
}

thread_local! {
    /// How the window's close button reaches the component (see
    /// `adopt_window`).
    static SENDER: RefCell<Option<LocalSender<Msg>>> = const { RefCell::new(None) };
    /// Set once the component has decided to close, so that the close it
    /// asks for is let through.
    static CLOSING: Cell<bool> = const { Cell::new(false) };
    /// The settings window, once `adopt_window` has found it.
    static WINDOW: Cell<Option<HWND>> = const { Cell::new(None) };
}

impl Component for Settings {
    type Input = Input;
    type Message = Msg;

    fn create(input: &Input, context: &ComponentContext<Self>) -> Self {
        SENDER.with(|s| *s.borrow_mut() = Some(context.sender()));
        Settings {
            editor: Editor::new(input.config.clone()),
            autostart: input.autostart,
            autostart_was: input.autostart,
            error: None,
            confirming: false,
            recording: None,
            running: input.running.clone(),
        }
    }

    fn update(&mut self, msg: Msg, context: &ComponentContext<Self>) {
        let e = &mut self.editor;
        match msg {
            Msg::SelectSet(Some(i)) => e.select_set_row(i),
            Msg::SelectSet(None) => {}
            Msg::AddSet => e.add_set(),
            Msg::AddPreset(Some(i)) => {
                if let Some(p) = preset::ALL.get(i) {
                    e.add_preset(p, &self.running);
                }
            }
            Msg::AddPreset(None) => {}
            Msg::DeleteSet => e.delete_set(),
            Msg::MoveSet(up) => e.move_set(up),
            Msg::RenameSet(name) => e.rename_set(&name),
            Msg::AddRunning(Some(i)) => {
                if let Some(app) = self.running.get(i).cloned()
                    && let Err(why) = e.add_app(app)
                {
                    self.error = Some((ADD_FAILED, why));
                }
            }
            Msg::AddRunning(None) => {}
            Msg::Browse => {
                let owner = window_number();
                context.spawn_background(move |_| {
                    Msg::Browsed(match filedialog::choose(owner, Choose::Executable) {
                        Ok(Some(path)) => process::app(&path)
                            .map(Some)
                            .ok_or_else(|| format!("{} はアプリとして使えません", path.display())),
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    })
                });
            }
            Msg::Browsed(Ok(Some(app))) => {
                if let Err(why) = e.add_app(app) {
                    self.error = Some((ADD_FAILED, why));
                }
            }
            Msg::Browsed(Ok(None)) => {}
            Msg::Browsed(Err(why)) => self.error = Some((ADD_FAILED, why)),
            Msg::RemoveApp(i) => e.remove_app(i),
            Msg::SelectCommand(Some(i)) => e.select_command(Some(i)),
            Msg::SelectCommand(None) => {}
            Msg::AddCommand => e.add_command(),
            Msg::DeleteCommand => e.delete_command(),
            Msg::MoveCommand(up) => e.move_command(up),
            Msg::RenameCommand(name) => e.rename_command(&name),
            Msg::Push(d) => e.push_direction(d),
            Msg::Pop => e.pop_direction(),
            Msg::ClearGesture => e.clear_gesture(),
            Msg::Record(row) => self.start_recording(row, context),
            Msg::Key(code, down) => {
                if let (Some((_, r)), Some(press)) = (&mut self.recording, vk::press(code)) {
                    if down {
                        r.key_down(press);
                    } else {
                        r.key_up(press);
                    }
                }
            }
            Msg::RecordClear => {
                if let Some((_, r)) = &mut self.recording {
                    *r = Recorder::default();
                }
            }
            Msg::RecordClosed(result) => {
                keyhook::stop();
                if let Some((row, r)) = self.recording.take()
                    && result == ContentDialogResult::Primary
                    && let Some(combo) = r.result()
                {
                    self.editor.set_action(row, Some(Action::Keys(combo)));
                }
            }
            Msg::ClearKeys(row) => e.set_action(row, None),
            Msg::Autostart(on) => self.autostart = on,
            Msg::Save => self.save(context),
            Msg::Export => self.export(context),
            Msg::Exported(Err(why)) => self.error = Some((EXPORT_FAILED, why)),
            Msg::Exported(Ok(())) => {}
            Msg::Import => {
                let owner = window_number();
                context.spawn_background(move |_| {
                    Msg::Imported(match filedialog::choose(owner, Choose::Import) {
                        Ok(Some(path)) => Config::read(&path).map(Some).map_err(|e| e.to_string()),
                        Ok(None) => Ok(None),
                        Err(e) => Err(e),
                    })
                });
            }
            Msg::Imported(Ok(Some(config))) => {
                self.editor.replace(config);
                self.error = None;
            }
            Msg::Imported(Ok(None)) => {}
            Msg::Imported(Err(why)) => self.error = Some((IMPORT_FAILED, why)),
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
            Msg::Discard(discard) => {
                self.confirming = false;
                if discard {
                    close(context);
                }
            }
            Msg::DismissError => self.error = None,
        }
    }

    fn view(&self, input: &Input, context: &mut ViewContext<Self>) -> View {
        context.window_title(concat!(
            "meta-gesture の設定 - v",
            env!("CARGO_PKG_VERSION")
        ));
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
        context.use_effect("adopt-window", (), || {
            adopt_window();
            None
        });

        let error = InfoBar::new()
            .severity(InfoBarSeverity::Error)
            .title(self.error.as_ref().map_or("", |(title, _)| *title))
            .message(
                self.error
                    .as_ref()
                    .map(|(_, e)| e.clone())
                    .unwrap_or_default(),
            )
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
            .columns([
                GridLength::Pixel(SETS_WIDTH),
                GridLength::Pixel(COMMANDS_WIDTH),
                GridLength::STAR,
            ])
            .column_spacing(20.0)
            .grid_row(1)
            .children((
                self.sets(context),
                Border::new().grid_column(1).content(self.commands(context)),
                ScrollViewer::new()
                    .vertical_scroll_bar_visibility(ScrollBarVisibility::Auto)
                    .grid_column(2)
                    .content(self.details(input, context)),
            ));

        let footer = Grid::new()
            .columns([GridLength::STAR, GridLength::Auto])
            .grid_row(2)
            .margin(Thickness::new(0.0, 16.0, 0.0, 0.0))
            .children((
                StackPanel::new().spacing(12.0).children((
                    CheckBox::new()
                        .is_checked(self.autostart)
                        .on_is_checked_changed(context.callback(Msg::Autostart))
                        .content("Windows へのサインイン時に meta-gesture を起動する"),
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(8.0)
                        .children((
                            Button::new()
                                .on_click(context.message(Msg::Export))
                                .content("エクスポート..."),
                            Button::new()
                                .on_click(context.message(Msg::Import))
                                .content("インポート..."),
                        )),
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
                            .content("保存して閉じる"),
                        Button::new()
                            .min_width(96.0)
                            .on_click(context.message(Msg::Cancel))
                            .content("キャンセル"),
                    )),
            ));

        // The dialog's own buttons cannot be colored, so it has none and
        // draws these in their place. Esc still closes it, as 戻る.
        let confirm = ContentDialog::new()
            .is_open(self.confirming)
            .title("変更を保存せずに閉じますか?")
            .on_closed(context.callback(|_| Msg::Discard(false)))
            .content(
                StackPanel::new().spacing(24.0).children((
                    TextBlock::new()
                        .text("保存していない変更は失われます。")
                        .text_wrapping(TextWrapping::Wrap),
                    Grid::new()
                        .columns([GridLength::STAR, GridLength::STAR])
                        .column_spacing(8.0)
                        .children((
                            Button::new()
                                .resource_overrides(critical_button())
                                .horizontal_alignment(HorizontalAlignment::Stretch)
                                .on_click(context.message(Msg::Discard(true)))
                                .content("閉じる"),
                            Button::new()
                                .grid_column(1)
                                .horizontal_alignment(HorizontalAlignment::Stretch)
                                .on_click(context.message(Msg::Discard(false)))
                                .content("戻る"),
                        )),
                )),
            );

        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR, GridLength::Auto])
            .margin(Thickness::uniform(24.0))
            .children((error, body, footer, confirm, self.recorder(context)))
    }
}

impl Settings {
    fn is_changed(&self) -> bool {
        self.editor.is_changed() || self.autostart != self.autostart_was
    }

    fn start_recording(&mut self, row: usize, context: &ComponentContext<Self>) {
        let Some(keys) = self
            .editor
            .key_rows()
            .get(row)
            .map(|r| r.action.as_ref().and_then(Action::keys))
        else {
            return;
        };
        let sender = context.sender();
        if let Err(e) = keyhook::start(move |vk, down| {
            sender.send(Msg::Key(vk, down));
        }) {
            self.error = Some((SAVE_FAILED, format!("キー入力を受け取れませんでした。{e}")));
            return;
        }
        self.recording = Some((row, Recorder::showing(keys)));
    }

    fn save(&mut self, context: &ComponentContext<Self>) {
        let config = match self.editor.check() {
            Ok(config) => config,
            Err(problem) => {
                self.error = Some((SAVE_FAILED, problem.message));
                return;
            }
        };
        if let Err(e) = config.save(&config_path()) {
            self.error = Some((SAVE_FAILED, e.to_string()));
            return;
        }
        self.editor.saved();
        if self.autostart != self.autostart_was {
            if let Err(e) = autostart::set_enabled(self.autostart) {
                self.error = Some((SAVE_FAILED, e));
                return;
            }
            self.autostart_was = self.autostart;
        }
        close(context);
    }

    /// Checks everything as saving does, then asks where to write it.
    fn export(&mut self, context: &ComponentContext<Self>) {
        let config = match self.editor.check() {
            Ok(config) => config,
            Err(problem) => {
                self.error = Some((EXPORT_FAILED, problem.message));
                return;
            }
        };
        let owner = window_number();
        context.spawn_background(move |_| {
            Msg::Exported(match filedialog::choose(owner, Choose::Export) {
                Ok(Some(path)) => config.save(&path).map_err(|e| e.to_string()),
                Ok(None) => Ok(()),
                Err(e) => Err(e),
            })
        });
    }

    /// The list of sets, the common set first, and the buttons that change
    /// it.
    fn sets(&self, context: &mut ViewContext<Self>) -> View {
        let selected = self.editor.selected_set_row();
        let rows = self
            .editor
            .set_rows()
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let line = StackPanel::new().spacing(2.0).children((
                    TextBlock::new()
                        .text(row.name)
                        .font_weight(FontWeight::SEMI_BOLD)
                        .text_trimming(TextTrimming::CharacterEllipsis),
                    TextBlock::new()
                        .text(row.apps)
                        .opacity(0.7)
                        .font_size(12.0)
                        .text_trimming(TextTrimming::CharacterEllipsis),
                ));
                (i, list_item(i, i == selected, line))
            });
        let common = self.editor.is_common();
        let buttons = Grid::new()
            .columns([GridLength::STAR; 4])
            .column_spacing(6.0)
            .grid_row(2)
            .margin(Thickness::new(0.0, 12.0, 0.0, 0.0))
            .children((
                button("追加", Msg::AddSet, true, context),
                Border::new().grid_column(1).content(button(
                    "削除",
                    Msg::DeleteSet,
                    !common,
                    context,
                )),
                Border::new().grid_column(2).content(button(
                    "上へ",
                    Msg::MoveSet(true),
                    self.editor.can_move_set(true),
                    context,
                )),
                Border::new().grid_column(3).content(button(
                    "下へ",
                    Msg::MoveSet(false),
                    self.editor.can_move_set(false),
                    context,
                )),
            ));
        // A list that adds the chosen preset and keeps no choice of its
        // own, as the one of running applications does.
        let presets = ComboBox::new()
            .items_source(preset::ALL.map(|p| p.name))
            .selected_index(None)
            .placeholder_text("プリセットから追加")
            .horizontal_alignment(HorizontalAlignment::Stretch)
            .grid_row(3)
            .on_selection_changed(context.callback(Msg::AddPreset));
        Grid::new()
            .rows([
                GridLength::Auto,
                GridLength::STAR,
                GridLength::Auto,
                GridLength::Auto,
            ])
            .row_spacing(8.0)
            .children((
                heading("セット"),
                Border::new()
                    .grid_row(1)
                    .content(list(rows, context.callback(Msg::SelectSet))),
                buttons,
                presets,
            ))
    }

    /// The selected set's commands, each with its gesture, and the buttons
    /// that change them.
    fn commands(&self, context: &mut ViewContext<Self>) -> View {
        let selected = self.editor.selected_command_row();
        let rows = self
            .editor
            .command_rows()
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let gesture = if row.gesture.is_empty() {
                    "—".to_string()
                } else {
                    row.gesture
                };
                let line = Grid::new()
                    .columns([GridLength::Pixel(84.0), GridLength::STAR])
                    .column_spacing(8.0)
                    .children((
                        TextBlock::new()
                            .text(gesture)
                            .font_size(18.0)
                            .vertical_alignment(VerticalAlignment::Center),
                        TextBlock::new()
                            .text(row.name)
                            .grid_column(1)
                            .vertical_alignment(VerticalAlignment::Center)
                            .text_trimming(TextTrimming::CharacterEllipsis),
                    ));
                (i, list_item(i, Some(i) == selected, line))
            });
        let chosen = selected.is_some();
        let buttons = Grid::new()
            .columns([GridLength::STAR; 4])
            .column_spacing(6.0)
            .grid_row(2)
            .margin(Thickness::new(0.0, 12.0, 0.0, 0.0))
            .children((
                button("追加", Msg::AddCommand, true, context),
                Border::new().grid_column(1).content(button(
                    "削除",
                    Msg::DeleteCommand,
                    chosen,
                    context,
                )),
                Border::new().grid_column(2).content(button(
                    "上へ",
                    Msg::MoveCommand(true),
                    self.editor.can_move_command(true),
                    context,
                )),
                Border::new().grid_column(3).content(button(
                    "下へ",
                    Msg::MoveCommand(false),
                    self.editor.can_move_command(false),
                    context,
                )),
            ));
        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR, GridLength::Auto])
            .row_spacing(8.0)
            .children((
                heading("コマンド"),
                Border::new()
                    .grid_row(1)
                    .content(list(rows, context.callback(Msg::SelectCommand))),
                buttons,
            ))
    }

    /// The selected set's name and applications, then the selected
    /// command's name, gesture and keys.
    fn details(&self, input: &Input, context: &mut ViewContext<Self>) -> View {
        let common = self.editor.is_common();
        let name = TextBox::new()
            .is_enabled(!common)
            .text(self.editor.set_name().to_string())
            .width(320.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .on_text_changed(context.callback(Msg::RenameSet));
        let set: View = if common {
            field("セット名", name)
        } else {
            StackPanel::new()
                .spacing(16.0)
                .children((field("セット名", name), self.apps(input, context)))
        };
        let command: View = match self.editor.command() {
            Some(_) => self.command(context),
            None => View::empty(),
        };
        StackPanel::new()
            .spacing(24.0)
            .margin(Thickness::new(0.0, 0.0, 16.0, 0.0))
            .children((set, command))
    }

    fn apps(&self, input: &Input, context: &mut ViewContext<Self>) -> View {
        let apps = self.editor.apps().iter().enumerate().map(|(i, app)| {
            let line = Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(8.0)
                .children((
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(8.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((
                            TextBlock::new().text(app.name.clone()),
                            TextBlock::new().text(app.id.clone()).opacity(0.6),
                        )),
                    Button::new()
                        .grid_column(1)
                        .on_click(context.message(Msg::RemoveApp(i)))
                        .content(FontIcon::new().glyph("\u{E711}")),
                ));
            (i, line)
        });
        let list = StackPanel::new().spacing(6.0).keyed_children(apps);
        let names: Vec<String> = input.running.iter().map(|a| a.name.clone()).collect();
        let add = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(8.0)
            .children((
                ComboBox::new()
                    .items_source(names)
                    .selected_index(None)
                    .placeholder_text("起動中のアプリから追加")
                    .min_width(240.0)
                    .on_selection_changed(context.callback(Msg::AddRunning)),
                Button::new()
                    .on_click(context.message(Msg::Browse))
                    .content("参照..."),
            ));
        field(
            "アプリ",
            StackPanel::new().spacing(10.0).children((list, add)),
        )
    }

    fn command(&self, context: &mut ViewContext<Self>) -> View {
        let Some(c) = self.editor.command() else {
            return View::empty();
        };
        let name = TextBox::new()
            .text(c.name.clone())
            .width(320.0)
            .horizontal_alignment(HorizontalAlignment::Left)
            .on_text_changed(context.callback(Msg::RenameCommand));

        let shown = if c.gesture.is_empty() {
            "未設定".to_string()
        } else {
            c.gesture.to_string()
        };
        let arrows = Direction::ALL.into_iter().enumerate().map(|(i, d)| {
            (
                i,
                Button::new()
                    .width(44.0)
                    .on_click(context.message(Msg::Push(d)))
                    .content(d.arrow().to_string()),
            )
        });
        let conflict: View = match self.editor.gesture_conflict() {
            Some(text) => TextBlock::new()
                .text(text)
                .foreground(ThemeBrush::SystemCritical)
                .into(),
            None => View::empty(),
        };
        let gesture = StackPanel::new().spacing(10.0).children((
            TextBlock::new()
                .text(shown)
                .font_size(28.0)
                .opacity(if c.gesture.is_empty() { 0.6 } else { 1.0 }),
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(6.0)
                .children((
                    StackPanel::new()
                        .orientation(Orientation::Horizontal)
                        .spacing(6.0)
                        .keyed_children(arrows),
                    Button::new()
                        .is_enabled(!c.gesture.is_empty())
                        .on_click(context.message(Msg::Pop))
                        .content("1 つ戻す"),
                    Button::new()
                        .is_enabled(!c.gesture.is_empty())
                        .on_click(context.message(Msg::ClearGesture))
                        .content("クリア"),
                )),
            conflict,
        ));

        let rows = self
            .editor
            .key_rows()
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let caps: View = match row.action.as_ref().and_then(Action::keys) {
                    Some(combo) => keycaps(combo.caps(Look::Windows), Caps::SMALL),
                    None => TextBlock::new()
                        .text(if i == 0 {
                            "未設定"
                        } else {
                            "セットの既定"
                        })
                        .opacity(0.6)
                        .vertical_alignment(VerticalAlignment::Center)
                        .into(),
                };
                let line = Grid::new()
                    .columns([
                        GridLength::Pixel(KEY_LABEL_WIDTH),
                        GridLength::STAR,
                        GridLength::Auto,
                    ])
                    .column_spacing(12.0)
                    .children((
                        TextBlock::new()
                            .text(row.label)
                            .vertical_alignment(VerticalAlignment::Center)
                            .text_trimming(TextTrimming::CharacterEllipsis),
                        Border::new()
                            .grid_column(1)
                            .vertical_alignment(VerticalAlignment::Center)
                            .content(caps),
                        StackPanel::new()
                            .grid_column(2)
                            .orientation(Orientation::Horizontal)
                            .spacing(6.0)
                            .children((
                                Button::new()
                                    .on_click(context.message(Msg::Record(i)))
                                    .content(
                                        StackPanel::new()
                                            .orientation(Orientation::Horizontal)
                                            .spacing(8.0)
                                            .children((FontIcon::new().glyph("\u{E70F}"), "変更")),
                                    ),
                                Button::new()
                                    .is_enabled(row.action.is_some())
                                    .on_click(context.message(Msg::ClearKeys(i)))
                                    .content("クリア"),
                            )),
                    ));
                (i, line)
            });
        let keys = StackPanel::new().spacing(8.0).keyed_children(rows);

        Border::new()
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(Thickness::uniform(1.0))
            .corner_radius(CornerRadius::uniform(8.0))
            .padding(Thickness::uniform(20.0))
            .content(StackPanel::new().spacing(20.0).children((
                field("コマンド名", name),
                field("ジェスチャー", gesture),
                field("送るキー", keys),
            )))
    }

    /// The dialog that records keys: it shows them as they are pressed.
    fn recorder(&self, context: &mut ViewContext<Self>) -> View {
        let r = self
            .recording
            .as_ref()
            .map(|(_, r)| r.clone())
            .unwrap_or_default();
        let caps = r.caps(Look::Windows);
        let keys: View = if caps.is_empty() {
            TextBlock::new()
                .text("キーを押してください")
                .opacity(0.6)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .into()
        } else {
            keycaps(caps, Caps::LARGE)
        };
        ContentDialog::new()
            .is_open(self.recording.is_some())
            .title("送るキーを設定")
            .primary_button_text("決定")
            .close_button_text("キャンセル")
            .is_primary_button_enabled(r.result().is_some())
            .on_closed(context.callback(Msg::RecordClosed))
            .content(
                StackPanel::new().spacing(16.0).width(400.0).children((
                    Border::new()
                        .background(ThemeBrush::CardBackground)
                        .border_brush(ThemeBrush::CardStroke)
                        .border_thickness(Thickness::uniform(1.0))
                        .corner_radius(CornerRadius::uniform(8.0))
                        .padding(Thickness::uniform(24.0))
                        .min_height(112.0)
                        .content(keys),
                    HyperlinkButton::new()
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .on_click(context.message(Msg::RecordClear))
                        .content("クリア"),
                )),
            )
    }
}

/// A list that keeps no selection of its own: the line shown as selected is
/// the editor's, and the one a click selects is taken away again straight
/// after, so that every click is reported. It also keeps WinUI from being
/// handed an index past the lines it has, which it refuses fatally.
fn list(rows: impl Iterator<Item = (usize, View)>, on_select: Callback<Option<usize>>) -> View {
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::uniform(1.0))
        .corner_radius(CornerRadius::uniform(8.0))
        .padding(Thickness::uniform(4.0))
        .content(
            ListView::new()
                .selection_mode(ListViewSelectionMode::Single)
                .selected_index(None)
                .on_selection_changed(on_select)
                .collection_slot(ListViewSlot::Items, rows),
        )
}

/// A line of a list, with the selection drawn as WinUI draws its own: a
/// gray behind it and an accent bar at its left.
fn list_item(i: usize, selected: bool, content: impl Into<View>) -> View {
    let indicator = Border::new()
        .background(if selected {
            Brush::from(ThemeBrush::Accent)
        } else {
            Brush::from(Color::transparent())
        })
        .corner_radius(CornerRadius::uniform(1.5))
        .height(16.0)
        .vertical_alignment(VerticalAlignment::Center);
    let line = Border::new()
        .background(if selected {
            SELECTED_ROW
        } else {
            Color::transparent()
        })
        .corner_radius(CornerRadius::uniform(4.0))
        .padding(Thickness::new(4.0, 8.0, 4.0, 8.0))
        .content(
            Grid::new()
                .columns([GridLength::Pixel(3.0), GridLength::STAR])
                .column_spacing(9.0)
                .children((indicator, Border::new().grid_column(1).content(content))),
        );
    ListViewItem::new().tag(i.to_string()).content(line)
}

fn button(label: &str, msg: Msg, enabled: bool, context: &mut ViewContext<Settings>) -> View {
    Button::new()
        .is_enabled(enabled)
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .on_click(context.message(msg))
        .content(label.to_string())
}

fn heading(text: &str) -> TextBlock {
    TextBlock::new()
        .text(text.to_string())
        .font_size(16.0)
        .font_weight(FontWeight::SEMI_BOLD)
}

/// A label above a control.
fn field(label: &str, control: impl Into<View>) -> View {
    StackPanel::new()
        .spacing(6.0)
        .children((TextBlock::new().text(label.to_string()), control.into()))
}

/// How large keycaps are drawn.
#[derive(Clone, Copy)]
struct Caps {
    height: f64,
    min_width: f64,
    text_size: f64,
    radius: f64,
    /// Inside a cap, at either side of its text.
    padding: f64,
    /// Between caps, and around the plus between them.
    spacing: f64,
}

impl Caps {
    /// The recording dialog's, big enough to read at a glance.
    const LARGE: Caps = Caps {
        height: 56.0,
        min_width: 64.0,
        text_size: 18.0,
        radius: 6.0,
        padding: 10.0,
        spacing: 12.0,
    };
    /// A line of keys'.
    const SMALL: Caps = Caps {
        height: 28.0,
        min_width: 28.0,
        text_size: 13.0,
        radius: 4.0,
        padding: 8.0,
        spacing: 4.0,
    };
}

/// Keys drawn as keycaps in a row, accent colored as Windows' own settings
/// draw shortcuts. The caps are pictures, not buttons.
fn keycaps(caps: Vec<Keycap>, size: Caps) -> View {
    let caps = caps.into_iter().enumerate().map(|(i, cap)| {
        if cap == Keycap::Plus {
            let plus: View = TextBlock::new()
                .text("+")
                .font_size(size.text_size)
                .vertical_alignment(VerticalAlignment::Center)
                .into();
            return (i, plus);
        }
        let face: View = match cap {
            Keycap::Arrow(arrow) => arrow_face(arrow, size.text_size),
            _ => TextBlock::new()
                .text(cap.label().unwrap_or_default())
                .font_size(size.text_size)
                .foreground(ON_ACCENT)
                .horizontal_alignment(HorizontalAlignment::Center)
                .vertical_alignment(VerticalAlignment::Center)
                .into(),
        };
        let cap: View = Border::new()
            .background(ThemeBrush::Accent)
            .corner_radius(CornerRadius::uniform(size.radius))
            .padding(Thickness::xy(size.padding, 0.0))
            .height(size.height)
            .min_width(size.min_width)
            .content(face);
        (i, cap)
    });
    StackPanel::new()
        .orientation(Orientation::Horizontal)
        .spacing(size.spacing)
        .horizontal_alignment(HorizontalAlignment::Left)
        .vertical_alignment(VerticalAlignment::Center)
        .keyed_children(caps)
}

/// Behind a selected line in a list: a gray that reads on both themes'
/// backgrounds, as WinUI's own does.
const SELECTED_ROW: Color = Color {
    a: 0x26,
    r: 0x80,
    g: 0x80,
    b: 0x80,
};

/// Text on an accent-colored cap: the window's base background, which is
/// the opposite of the accent fill in both themes.
const ON_ACCENT: ThemeBrush = ThemeBrush::SolidBackground;

/// The colors of a button whose action cannot be undone: WinUI's
/// SystemFillColorCritical of the light theme behind white text, in both
/// themes. Lighter on hover and when pressed, as an accent button is.
fn critical_button() -> ResourceOverrides {
    let fill = Color::rgb(0xC4, 0x2B, 0x1C);
    let text = Color::rgb(0xFF, 0xFF, 0xFF);
    let fill_at = |a| Color { a, ..fill };
    ResourceOverrides::new()
        .set("ButtonBackground", fill)
        .set("ButtonBackgroundPointerOver", fill_at(0xE6))
        .set("ButtonBackgroundPressed", fill_at(0xCC))
        .set("ButtonForeground", text)
        .set("ButtonForegroundPointerOver", text)
        .set("ButtonForegroundPressed", Color { a: 0xB3, ..text })
        .set("ButtonBorderBrush", Color::transparent())
        .set("ButtonBorderBrushPointerOver", Color::transparent())
        .set("ButtonBorderBrushPressed", Color::transparent())
}

/// An arrow key's cap face: an arrow as tall as the text on the other caps,
/// drawn as three strokes so that it takes the same color as that text.
fn arrow_face(arrow: Arrow, text_size: f64) -> View {
    let size = text_size * 0.8;
    let thickness = text_size / 9.0;
    let (half, head) = (size / 2.0, size * 0.35);
    let (cx, cy) = (size / 2.0, size / 2.0);
    // For an arrow pointing right: the tip, the two ends of the head, and
    // the tail. The shaft stops short of the tip so that its flat end does
    // not show past the head.
    let points = [
        (half, 0.0),
        (half - head, -head),
        (half - head, head),
        (-half, 0.0),
        (half - thickness, 0.0),
    ];
    let turn = |(x, y): (f64, f64)| match arrow {
        Arrow::Right => (cx + x, cy + y),
        Arrow::Left => (cx - x, cy + y),
        Arrow::Down => (cx + y, cy + x),
        Arrow::Up => (cx + y, cy - x),
    };
    let [tip, a, b, tail, shaft_end] = points.map(turn);
    let stroke = |from: (f64, f64), to: (f64, f64)| {
        Line::new()
            .x1(from.0)
            .y1(from.1)
            .x2(to.0)
            .y2(to.1)
            .stroke(ON_ACCENT)
            .stroke_thickness(thickness)
    };
    Canvas::new()
        .width(size)
        .height(size)
        .horizontal_alignment(HorizontalAlignment::Center)
        .vertical_alignment(VerticalAlignment::Center)
        .children((stroke(a, tip), stroke(b, tip), stroke(tail, shaft_end)))
}

fn close(context: &ComponentContext<Settings>) {
    CLOSING.set(true);
    let _ = context.window().request_close();
}

/// Does to the window what Reactor has no way to say. It gets the icon of
/// the notification area on its title bar and taskbar button, and its close
/// button goes through [`Msg::Cancel`], so that closing with unsaved changes
/// asks first. Reactor does not hand out its window, so this finds it among
/// the thread's windows by the class WinUI 3 gives a desktop window. Should
/// that fail, the window keeps the default icon and the close button simply
/// closes.
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
    WINDOW.set(Some(hwnd));
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

/// The settings window as a number that can cross to the thread a file
/// dialog runs on, or 0 before `adopt_window` has found it.
fn window_number() -> isize {
    WINDOW.get().map_or(0, |hwnd| hwnd.0 as isize)
}
