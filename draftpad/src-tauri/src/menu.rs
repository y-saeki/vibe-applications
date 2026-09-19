//! Native menus: the menu bar and the right-click menu.
//!
//! The menu bar is macOS only; Windows has no menu bar, and handles the same
//! shortcuts in the webview. The right-click menu built here is macOS's too.
//! WKWebView will not let its own menu be trimmed, so on macOS the page
//! suppresses it (`src/context-menu.ts`) and asks for this one instead, while
//! Windows keeps the webview's menu and trims it in place (`context_menu.rs`).
//! Nothing stops this menu from being put up anywhere; only macOS asks.
//!
//! Menu items with an application-level meaning are forwarded to the webview
//! as a `menu` event carrying the item id, and `src/commands.ts` maps the id
//! onto its command table. Both menus here use predefined items for cut, copy,
//! paste and select all; the menu bar has to, because WKWebView only routes
//! Cmd+C/V/X/A to the page when such items exist. Undo and redo are forwarded
//! items instead: the predefined ones drive WKWebView's own undo manager,
//! which knows nothing about the editor's history. "Paste as Plain Text" is
//! forwarded too: the predefined paste item carries Cmd+V and takes no
//! accelerator of its own.

use serde::Deserialize;
use tauri::menu::{ContextMenu, MenuBuilder, MenuItemBuilder};
use tauri::{AppHandle, Emitter, Manager, Window};

#[cfg(target_os = "macos")]
use tauri::menu::{AboutMetadata, SubmenuBuilder};

const FORWARDED_IDS: [&str; 9] = [
    "preferences",
    "quit",
    "close",
    "undo",
    "redo",
    "paste_plain",
    "find",
    "increase_font_size",
    "decrease_font_size",
];

/// Carries every menu item [`FORWARDED_IDS`] names to the webview. Both menus
/// go through it, so it is installed on every platform and only once.
pub fn forward_events(app: &AppHandle) {
    app.on_menu_event(|app, event| {
        let id = event.id().0.as_str();
        if FORWARDED_IDS.contains(&id) {
            if let Err(err) = app.emit("menu", id) {
                eprintln!("draftpad: failed to forward menu event {id}: {err}");
            }
        }
        #[cfg(debug_assertions)]
        if id == "devtools" {
            if let Some(window) = app.get_webview_window("main") {
                if window.is_devtools_open() {
                    window.close_devtools();
                } else {
                    window.open_devtools();
                }
            }
        }
    });
}

/// What the page knows and the menu cannot work out for itself, as
/// `src/context-menu.ts` sends it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextState {
    can_undo: bool,
    can_redo: bool,
    can_search: bool,
}

/// Puts draftpad's right-click menu up at the pointer, in place of the one the
/// webview would have opened.
///
/// The four editing items are predefined, so they carry the platform's own
/// wording and reach whatever holds the caret — the draft, the search panel's
/// fields, the preferences panel's — without this side having to know which.
/// They also stay enabled throughout, which is all a predefined item allows;
/// each one simply does nothing when there is no selection to act on.
///
/// The call only returns once the menu is dismissed: a native menu runs a modal
/// loop of its own.
pub fn show_context(window: &Window, state: &ContextState) -> tauri::Result<()> {
    let app = window.app_handle();
    let item = |id: &str, text: &str, enabled: bool| {
        MenuItemBuilder::with_id(id, text)
            .enabled(enabled)
            .build(app)
    };
    let menu = MenuBuilder::new(app)
        .item(&item("undo", "元に戻す", state.can_undo)?)
        .item(&item("redo", "やり直す", state.can_redo)?)
        .separator()
        .cut()
        .copy()
        .paste()
        .separator()
        .select_all()
        .separator()
        .item(&item("find", "検索・置換", state.can_search)?)
        .build()?;
    menu.popup(window.clone())
}

#[cfg(target_os = "macos")]
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let item = |id: &str, text: &str, accelerator: &str| {
        MenuItemBuilder::with_id(id, text)
            .accelerator(accelerator)
            .build(app)
    };

    let about = AboutMetadata {
        name: Some("draftpad".into()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };
    let app_menu = SubmenuBuilder::new(app, "draftpad")
        .about(Some(about))
        .separator()
        .item(&item("preferences", "Preferences…", "CmdOrCtrl+,")?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .item(&item("quit", "Quit draftpad", "CmdOrCtrl+Q")?)
        .build()?;

    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&item("undo", "Undo", "CmdOrCtrl+Z")?)
        .item(&item("redo", "Redo", "CmdOrCtrl+Shift+Z")?)
        .separator()
        .cut()
        .copy()
        .paste()
        .item(&item(
            "paste_plain",
            "Paste as Plain Text",
            "CmdOrCtrl+Shift+V",
        )?)
        .select_all()
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&item("close", "Close", "CmdOrCtrl+W")?)
        .fullscreen();
    #[cfg(debug_assertions)]
    let view = view.item(&item(
        "devtools",
        "Toggle Developer Tools",
        "Alt+CmdOrCtrl+I",
    )?);
    let view = view.build()?;

    let text = SubmenuBuilder::new(app, "Text")
        .item(&item(
            "increase_font_size",
            "Increase Font Size",
            "CmdOrCtrl+=",
        )?)
        .item(&item(
            "decrease_font_size",
            "Decrease Font Size",
            "CmdOrCtrl+-",
        )?)
        .build()?;

    let menu = MenuBuilder::new(app)
        .items(&[&app_menu, &edit, &view, &text])
        .build()?;
    app.set_menu(menu)?;
    Ok(())
}
