//! The right-click menu on Windows.
//!
//! WebView2 draws its own, and it is worth keeping: it looks the way the rest
//! of Windows does, its wording comes from the OS rather than from here, and
//! its editing items are already wired to the page. So this does not replace
//! that menu — it only decides which of its items may appear, and adds the one
//! command draftpad has that WebView2 has no item for.
//!
//! macOS offers no such hook. `menu.rs` says what happens there instead.

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2ContextMenuRequestedEventArgs, ICoreWebView2Controller, ICoreWebView2Environment9,
    ICoreWebView2_11, ICoreWebView2_2, COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
    COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
};
use webview2_com::{
    CoTaskMemPWSTR, ContextMenuRequestedEventHandler, CustomItemSelectedEventHandler,
};
use windows::core::{Interface, Result, BOOL, HSTRING, PWSTR};
use windows::Win32::System::Com::IStream;

/// The command with no WebView2 item of its own. The id is the one
/// `src/commands.ts` knows, and `menu::forward_events` carries it to the page;
/// the label comes from `menu.rs`, so that both menus say the same thing.
const FIND_ID: &str = "find";

/// Decides whether WebView2 may keep one of its own items, by the name it
/// gives it.
///
/// A list of what to keep rather than of what to drop: an item a later Edge
/// adds — as the emoji picker and the writing helpers were added — stays out
/// on its own, instead of turning up here until someone notices.
fn is_kept(name: &str) -> bool {
    // Right-clicking is the only way into the developer tools on Windows,
    // where there is no menu bar to put them on.
    #[cfg(debug_assertions)]
    if name == "inspectElement" {
        return true;
    }
    matches!(
        name,
        "undo" | "redo" | "cut" | "copy" | "paste" | "selectAll"
    )
}

/// Takes over the context menu on `window`'s webview. The work happens on the
/// webview thread, so it may finish after this call returns.
pub fn install(window: &WebviewWindow) {
    let app = window.app_handle().clone();
    let dispatched = window.with_webview(move |webview| {
        if let Err(err) = install_on(&webview.controller(), app) {
            eprintln!("draftpad: failed to take over the WebView2 context menu: {err}");
        }
    });
    if let Err(err) = dispatched {
        eprintln!("draftpad: failed to reach the WebView2 controller: {err}");
    }
}

fn install_on(controller: &ICoreWebView2Controller, app: AppHandle) -> Result<()> {
    unsafe {
        let webview = controller.CoreWebView2()?;
        // ICoreWebView2_11 raises the event and ICoreWebView2Environment9 is
        // what builds an item of our own; an older runtime that implements
        // neither fails the cast and keeps its whole menu.
        let events: ICoreWebView2_11 = webview.cast()?;
        let environment: ICoreWebView2Environment9 =
            webview.cast::<ICoreWebView2_2>()?.Environment()?.cast()?;
        let mut token = 0i64;
        events.add_ContextMenuRequested(
            &ContextMenuRequestedEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else { return Ok(()) };
                if let Err(err) = build(&args, &environment, &app) {
                    eprintln!("draftpad: failed to trim the context menu: {err}");
                }
                Ok(())
            })),
            &mut token,
        )
    }
}

/// Drops what [`is_kept`] does not name and puts draftpad's own item at the
/// end, leaving `Handled` alone so that WebView2 goes on to draw the menu it
/// would have drawn, holding what is left of it.
///
/// Where there is nothing to edit there is no menu at all: `Handled` with no
/// command selected is how a host says it is showing none of its own.
fn build(
    args: &ICoreWebView2ContextMenuRequestedEventArgs,
    environment: &ICoreWebView2Environment9,
    app: &AppHandle,
) -> Result<()> {
    unsafe {
        // The draft and the panels' text fields are the whole of what this
        // menu acts on. Over the bars, a button or a dropdown it would
        // have nothing to offer.
        let mut editable = BOOL::default();
        args.ContextMenuTarget()?.IsEditable(&mut editable)?;
        if !editable.as_bool() {
            return args.SetHandled(true);
        }

        let items = args.MenuItems()?;
        let mut count = 0u32;
        items.Count(&mut count)?;
        // Backwards, so a removal does not shift the indexes still to come.
        for index in (0..count).rev() {
            let item = items.GetValueAtIndex(index)?;
            let mut name = PWSTR::null();
            item.Name(&mut name)?;
            if !is_kept(&CoTaskMemPWSTR::from(name).to_string()) {
                items.RemoveValueAtIndex(index)?;
            }
        }

        items.Count(&mut count)?;
        let separator = environment.CreateContextMenuItem(
            &HSTRING::new(),
            None::<&IStream>,
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_SEPARATOR,
        )?;
        items.InsertValueAtIndex(count, &separator)?;

        let find = environment.CreateContextMenuItem(
            &HSTRING::from(crate::menu::FIND),
            None::<&IStream>,
            COREWEBVIEW2_CONTEXT_MENU_ITEM_KIND_COMMAND,
        )?;
        let app = app.clone();
        let mut token = 0i64;
        find.add_CustomItemSelected(
            &CustomItemSelectedEventHandler::create(Box::new(move |_, _| {
                if let Err(err) = app.emit("menu", FIND_ID) {
                    eprintln!("draftpad: failed to forward menu event {FIND_ID}: {err}");
                }
                Ok(())
            })),
            &mut token,
        )?;
        items.InsertValueAtIndex(count + 1, &find)?;
        Ok(())
    }
}
