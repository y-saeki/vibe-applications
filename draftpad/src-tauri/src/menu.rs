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
//! which knows nothing about the editor's history. "Paste as plain text" is
//! forwarded too: the predefined paste item carries Cmd+V and takes no
//! accelerator of its own.
//!
//! "閉じる" (Cmd+W) is forwarded as well: what it closes is decided in the
//! page — the pane with the caret while the compare pane is open, otherwise
//! the window.
//!
//! The accelerators written here are the defaults. The page moves them to
//! whatever keys the preferences panel has given those commands
//! ([`set_shortcuts`]), once it has started and again after every change.
//! The predefined items keep theirs: muda gives them no way to change it.

use serde::Deserialize;
use tauri::menu::{ContextMenu, MenuBuilder, MenuItemBuilder};
use tauri::{AppHandle, Emitter, Manager, Window};

#[cfg(target_os = "macos")]
use std::collections::HashMap;

#[cfg(target_os = "macos")]
use tauri::menu::{AboutMetadata, MenuItem, SubmenuBuilder};

// Wording the menu bar and the right-click menu share, in one place so that
// they cannot drift apart. The predefined items need it spelled out too: muda
// gives them a hardcoded English string rather than the platform's own.
const UNDO: &str = "元に戻す";
const REDO: &str = "やり直す";
const CUT: &str = "切り取り";
const COPY: &str = "コピー";
const PASTE: &str = "貼り付け";
const SELECT_ALL: &str = "すべてを選択";
/// Shared with `context_menu.rs`, whose menu offers the same command, and the
/// last item draftpad puts in the menu bar's Edit menu — `edit_menu::trim`
/// takes off whatever AppKit adds after it.
pub(crate) const FIND: &str = "検索・置換";

/// The Edit menu's title, which is how `edit_menu::trim` finds it again.
#[cfg(target_os = "macos")]
pub(crate) const EDIT_TITLE: &str = "編集";

const FORWARDED_IDS: [&str; 8] = [
    "preferences",
    "quit",
    "close",
    "undo",
    "redo",
    "paste_plain",
    "find",
    "open_compare",
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
/// The four editing items are predefined, so they reach whatever holds the
/// caret — the draft, the search panel's fields, the preferences panel's —
/// without this side having to know which. They also stay enabled throughout,
/// which is all a predefined item allows; each one simply does nothing when
/// there is no selection to act on.
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
        .item(&item("undo", UNDO, state.can_undo)?)
        .item(&item("redo", REDO, state.can_redo)?)
        .separator()
        .cut_with_text(CUT)
        .copy_with_text(COPY)
        .paste_with_text(PASTE)
        .separator()
        .select_all_with_text(SELECT_ALL)
        .separator()
        .item(&item("find", FIND, state.can_search)?)
        .build()?;
    menu.popup(window.clone())
}

/// The menu bar's items whose key the preferences panel can move, by id —
/// the same ids `src/shortcuts.ts` lists them under.
#[cfg(target_os = "macos")]
pub struct Shortcuts(HashMap<&'static str, MenuItem<tauri::Wry>>);

/// The ids of [`Shortcuts`].
#[cfg(target_os = "macos")]
const SHORTCUT_IDS: [&str; 6] = [
    "preferences",
    "quit",
    "paste_plain",
    "find",
    "open_compare",
    "close",
];

/// Puts each item [`Shortcuts`] holds on the accelerator the page sent for
/// it, or on none. An id the page did not send is left as it is, and one this
/// side does not know is ignored.
#[cfg(target_os = "macos")]
pub fn set_shortcuts(app: &AppHandle, shortcuts: &HashMap<String, Option<String>>) {
    let Some(items) = app.try_state::<Shortcuts>() else {
        return;
    };
    for (id, accelerator) in shortcuts {
        let Some(item) = items.0.get(id.as_str()) else {
            continue;
        };
        if let Err(err) = item.set_accelerator(accelerator.as_deref()) {
            eprintln!("draftpad: failed to set the key of menu item {id}: {err}");
        }
    }
}

#[cfg(target_os = "macos")]
pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let mut shortcuts = HashMap::new();
    let mut item = |id: &str, text: &str, accelerator: &str| {
        let built = MenuItemBuilder::with_id(id, text)
            .accelerator(accelerator)
            .build(app)?;
        if let Some(known) = SHORTCUT_IDS.iter().find(|known| **known == id) {
            shortcuts.insert(*known, built.clone());
        }
        Ok::<_, tauri::Error>(built)
    };

    let about = AboutMetadata {
        name: Some("draftpad".into()),
        version: Some(app.package_info().version.to_string()),
        ..Default::default()
    };
    let app_menu = SubmenuBuilder::new(app, "draftpad")
        .about_with_text("draftpad について", Some(about))
        .separator()
        .item(&item("preferences", "環境設定…", "CmdOrCtrl+,")?)
        .separator()
        .services_with_text("サービス")
        .separator()
        .hide_with_text("draftpad を隠す")
        .hide_others_with_text("ほかを隠す")
        .show_all_with_text("すべてを表示")
        .separator()
        .item(&item("quit", "draftpad を終了", "CmdOrCtrl+Q")?)
        .build()?;

    let edit = SubmenuBuilder::new(app, EDIT_TITLE)
        .item(&item("undo", UNDO, "CmdOrCtrl+Z")?)
        .item(&item("redo", REDO, "CmdOrCtrl+Shift+Z")?)
        .separator()
        .cut_with_text(CUT)
        .copy_with_text(COPY)
        .paste_with_text(PASTE)
        .item(&item(
            "paste_plain",
            "プレーンテキストとして貼り付け",
            "CmdOrCtrl+Shift+V",
        )?)
        .select_all_with_text(SELECT_ALL)
        .separator()
        .item(&item("find", FIND, "CmdOrCtrl+F")?)
        .build()?;

    // Cmd+\ is the key VS Code splits its editor with. It only opens the
    // compare pane; Cmd+W is what closes one, so no key is added for that.
    let view = SubmenuBuilder::new(app, "表示")
        .item(&item(
            "open_compare",
            "テキストを比較する",
            "CmdOrCtrl+Backslash",
        )?)
        .separator()
        .item(&item("close", "閉じる", "CmdOrCtrl+W")?)
        .fullscreen_with_text("フルスクリーンを切り替え");
    #[cfg(debug_assertions)]
    let view = view.item(&item(
        "devtools",
        "開発者ツールを切り替え",
        "Alt+CmdOrCtrl+I",
    )?);
    let view = view.build()?;

    let menu = MenuBuilder::new(app)
        .items(&[&app_menu, &edit, &view])
        .build()?;
    app.set_menu(menu)?;
    app.manage(Shortcuts(shortcuts));
    Ok(())
}
