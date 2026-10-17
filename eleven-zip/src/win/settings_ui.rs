//! The settings window, in WinUI 3: what eleven-zip does after extracting, its
//! place in Explorer, and the 7-Zip it uses. It is what eleven-zip.exe opens when
//! started on its own, from the Start menu or at the end of the installer.
//!
//! A switch takes effect the moment it is flipped: the config is saved and
//! the registry rewritten straight away, as Windows' own settings do, so the
//! window has no 保存 button.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, FindWindowExW, GetClassNameW, GetWindowTextW, ICON_BIG, ICON_SMALL,
    IsIconic, SW_RESTORE, SW_SHOWNORMAL, SendMessageW, SetForegroundWindow, ShowWindow, WM_SETICON,
};
use windows::core::{BOOL, HSTRING, PCWSTR, w};
use windows_reactor::*;

use super::sevenzip::{self, Checked};
use super::{config_path, error_box, icon, now, registry, store};
use crate::config::Config;
use crate::release::{self, Version};
use crate::title;

const CLIENT: (f64, f64) = (640.0, 640.0);
const MIN_CLIENT: (f64, f64) = (480.0, 360.0);
const WINUI_WINDOW_CLASS: &str = "WinUIDesktopWin32WindowClass";
/// Windows' default-apps settings, at the page for the application that
/// `Software\RegisteredApplications` lists under this name.
const DEFAULT_APPS: &str = "ms-settings:defaultapps?registeredAppUser=eleven-zip";

/// Runs the settings window until it closes, or brings forward the one that
/// is already open.
pub fn run() {
    if bring_forward_existing() {
        return;
    }
    let config = match Config::load(&config_path()) {
        Ok(config) => config,
        Err(e) => {
            error_box(&e.to_string());
            return;
        }
    };
    // Again on every start, so that the registry points at this executable
    // even after eleven-zip has been installed somewhere else.
    let error = registry::register(config.context_menu).err();
    let input = Input {
        config,
        version: store().current().map(|i| i.version),
        error,
    };
    if let Err(e) = App::run_component::<Settings>(input) {
        error_box(&format!("設定を開けませんでした。\n{e}"));
    }
}

#[derive(Clone, PartialEq)]
struct Input {
    config: Config,
    version: Option<Version>,
    error: Option<String>,
}

struct Settings {
    config: Config,
    /// The 7-Zip in use, `None` until one is downloaded.
    version: Option<Version>,
    update: Update,
    /// Shown above everything when a change could not be saved.
    error: Option<String>,
}

#[derive(Clone, PartialEq)]
enum Update {
    Idle,
    Working,
    Done(Checked),
    Failed(String),
}

#[derive(Clone)]
enum Msg {
    OpenInExplorer(bool),
    UnwrapSingleItem(bool),
    TrashArchive(bool),
    ContextMenu(bool),
    AutoUpdate(bool),
    DefaultApps,
    CheckNow,
    Checked(Result<Checked, String>, Option<Version>),
    DismissError,
}

impl Component for Settings {
    type Input = Input;
    type Message = Msg;

    fn create(input: &Input, context: &ComponentContext<Self>) -> Self {
        let mut settings = Settings {
            config: input.config.clone(),
            version: input.version,
            update: Update::Idle,
            error: input.error.clone(),
        };
        // 7-Zip is fetched as soon as the window first opens (the installer
        // opens it), so that the first extraction does not wait for it.
        let due = input.config.auto_update && release::is_due(store().last_check(), now());
        if input.version.is_none() || due {
            settings.check(context);
        }
        settings
    }

    fn update(&mut self, msg: Msg, context: &ComponentContext<Self>) {
        match msg {
            Msg::OpenInExplorer(on) => {
                self.change(|c| c.open_in_explorer = on);
            }
            Msg::UnwrapSingleItem(on) => {
                self.change(|c| c.unwrap_single_item = on);
            }
            Msg::TrashArchive(on) => {
                self.change(|c| c.trash_archive = on);
            }
            Msg::ContextMenu(on) => {
                if self.change(|c| c.context_menu = on) {
                    self.error = registry::register(on).err();
                }
            }
            Msg::AutoUpdate(on) => {
                self.change(|c| c.auto_update = on);
            }
            Msg::DefaultApps => unsafe {
                ShellExecuteW(
                    None,
                    w!("open"),
                    &HSTRING::from(DEFAULT_APPS),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            },
            Msg::CheckNow => self.check(context),
            Msg::Checked(result, version) => {
                self.version = version;
                self.update = match result {
                    Ok(checked) => Update::Done(checked),
                    Err(e) => Update::Failed(e),
                };
            }
            Msg::DismissError => self.error = None,
        }
    }

    fn view(&self, _input: &Input, context: &mut ViewContext<Self>) -> View {
        context.window_title(title::window_title(env!("CARGO_PKG_VERSION")));
        context.window_visuals(
            WindowVisuals::new()
                .backdrop(WindowBackdrop::Mica)
                .client_size(CLIENT.0, CLIENT.1)
                .constraints(WindowConstraints {
                    min_width: Some(MIN_CLIENT.0),
                    min_height: Some(MIN_CLIENT.1),
                    ..Default::default()
                }),
        );
        context.use_effect("adopt-window", (), || {
            adopt_window();
            None
        });

        let error = InfoBar::new()
            .severity(InfoBarSeverity::Error)
            .title("設定を変更できませんでした")
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

        let version = self.version_card(context);
        let c = &self.config;
        let toggle = |on: bool, msg: fn(bool) -> Msg| {
            ToggleSwitch::new()
                .is_on(on)
                .on_toggled(context.callback(msg))
        };
        let sections = StackPanel::new().spacing(4.0).children((
            heading("展開"),
            card(
                "展開したらエクスプローラーで開く",
                None,
                toggle(c.open_in_explorer, Msg::OpenInExplorer),
            ),
            card(
                "中身が 1 つだけなら、フォルダーを作らずに展開する",
                None,
                toggle(c.unwrap_single_item, Msg::UnwrapSingleItem),
            ),
            card(
                "展開したら書庫をごみ箱に入れる",
                None,
                toggle(c.trash_archive, Msg::TrashArchive),
            ),
            heading("エクスプローラー"),
            card(
                "右クリックメニューに「eleven-zip で展開」を表示する",
                None,
                toggle(c.context_menu, Msg::ContextMenu),
            ),
            card(
                "既定のアプリ",
                None,
                Button::new()
                    .on_click(context.message(Msg::DefaultApps))
                    .content("Windows の設定を開く"),
            ),
            heading("7-Zip"),
            version,
            card(
                "自動で更新する",
                None,
                toggle(c.auto_update, Msg::AutoUpdate),
            ),
        ));

        Grid::new()
            .rows([GridLength::Auto, GridLength::STAR])
            .margin(Thickness::new(24.0, 16.0, 24.0, 24.0))
            .children((
                error,
                Border::new()
                    .grid_row(1)
                    .content(ScrollViewer::new().content(sections)),
            ))
    }
}

impl Settings {
    /// Applies a change to the config and saves it. False when nothing
    /// changed (WinUI reports a switch set to what it was) or saving failed.
    fn change(&mut self, edit: impl FnOnce(&mut Config)) -> bool {
        let mut config = self.config.clone();
        edit(&mut config);
        if config == self.config {
            return false;
        }
        match config.save(&config_path()) {
            Ok(()) => {
                self.config = config;
                self.error = None;
                true
            }
            Err(e) => {
                self.error = Some(e.to_string());
                false
            }
        }
    }

    /// Looks for a new 7-Zip, and installs it, off the window's thread.
    fn check(&mut self, context: &ComponentContext<Self>) {
        if self.update == Update::Working {
            return;
        }
        self.update = Update::Working;
        context.spawn_background(|_| {
            let store = store();
            let result = sevenzip::update(&store);
            Msg::Checked(result, store.current().map(|i| i.version))
        });
    }

    /// The version in use, what the last check found, and the button that
    /// checks again.
    fn version_card(&self, context: &mut ViewContext<Self>) -> View {
        let version = match self.version {
            Some(v) => format!("バージョン {v}"),
            None => "未取得".to_string(),
        };
        let (status, failed) = match &self.update {
            Update::Idle => (None, false),
            Update::Working => (Some("確認しています…".to_string()), false),
            Update::Done(Checked::UpToDate(_)) => (Some("最新です".to_string()), false),
            Update::Done(Checked::Updated(v)) => (Some(format!("{v} に更新しました")), false),
            Update::Done(Checked::Busy) => {
                (Some("別の eleven-zip が更新しています".to_string()), false)
            }
            Update::Failed(e) => (Some(e.clone()), true),
        };
        let working = self.update == Update::Working;
        let action = StackPanel::new()
            .orientation(Orientation::Horizontal)
            .spacing(12.0)
            .children((
                ProgressRing::new()
                    .is_active(working)
                    .width(20.0)
                    .height(20.0)
                    .opacity(if working { 1.0 } else { 0.0 })
                    .vertical_alignment(VerticalAlignment::Center),
                Button::new()
                    .is_enabled(!working)
                    .on_click(context.message(Msg::CheckNow))
                    .content("更新を確認"),
            ));
        card(&version, status.map(|s| (s, failed)), action)
    }
}

/// A section's title above its cards.
fn heading(text: &str) -> View {
    TextBlock::new()
        .text(text.to_string())
        .font_weight(FontWeight::SEMI_BOLD)
        .margin(Thickness::new(0.0, 20.0, 0.0, 4.0))
        .into()
}

/// One setting as Windows' own settings draw it: a card with the label at
/// the left, an optional second line under it (red when `true`), and the
/// control at the right.
fn card(label: &str, detail: Option<(String, bool)>, control: impl Into<View>) -> View {
    let detail: View = match detail {
        Some((text, failed)) => {
            let line = TextBlock::new()
                .text(text)
                .font_size(12.0)
                .text_wrapping(TextWrapping::Wrap);
            if failed {
                line.foreground(ThemeBrush::SystemCritical).into()
            } else {
                line.opacity(0.7).into()
            }
        }
        None => View::empty(),
    };
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::uniform(1.0))
        .corner_radius(CornerRadius::uniform(8.0))
        .padding(Thickness::xy(16.0, 12.0))
        .min_height(64.0)
        .content(
            Grid::new()
                .columns([GridLength::STAR, GridLength::Auto])
                .column_spacing(16.0)
                .children((
                    StackPanel::new()
                        .spacing(2.0)
                        .vertical_alignment(VerticalAlignment::Center)
                        .children((
                            TextBlock::new()
                                .text(label.to_string())
                                .text_wrapping(TextWrapping::Wrap),
                            detail,
                        )),
                    Border::new()
                        .grid_column(1)
                        .vertical_alignment(VerticalAlignment::Center)
                        .content(control),
                )),
        )
}

/// Starting eleven-zip with no file while its settings window is open brings that
/// window forward instead of opening a second one. The window is found by its
/// title, which may be another version's.
fn bring_forward_existing() -> bool {
    let class = HSTRING::from(WINUI_WINDOW_CLASS);
    let mut after = None;
    let hwnd = loop {
        let Ok(hwnd) = (unsafe { FindWindowExW(None, after, &class, PCWSTR::null()) }) else {
            return false;
        };
        if hwnd.is_invalid() {
            return false;
        }
        let mut text = [0u16; 256];
        let len = unsafe { GetWindowTextW(hwnd, &mut text) }.max(0) as usize;
        if title::is_settings(&String::from_utf16_lossy(&text[..len])) {
            break hwnd;
        }
        after = Some(hwnd);
    };
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
    }
    true
}

/// Gives the window eleven-zip's icon on its title bar and taskbar button, which
/// Reactor has no way to say. Reactor does not hand out its window, so this
/// finds it among the thread's windows by the class WinUI 3 gives a desktop
/// window; should that fail, the window keeps the default icon.
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
}
