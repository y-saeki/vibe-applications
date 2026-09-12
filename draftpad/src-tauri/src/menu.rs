//! Native application menu (macOS only). Windows has no menu bar; the same
//! shortcuts are handled in the webview there.
//!
//! Menu items with an application-level meaning are forwarded to the webview
//! as a `menu` event carrying the item id, and `src/commands.ts` maps the id
//! onto its command table. The Edit menu uses predefined items for cut, copy,
//! paste and select all because WKWebView only routes Cmd+C/V/X/A to the page
//! when such items exist. Undo and redo are forwarded items instead: the
//! predefined ones drive WKWebView's own undo manager, which knows nothing
//! about the editor's history.

use tauri::menu::{AboutMetadata, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager};

const FORWARDED_IDS: [&str; 7] = [
    "preferences",
    "quit",
    "close",
    "undo",
    "redo",
    "increase_font_size",
    "decrease_font_size",
];

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
    Ok(())
}
