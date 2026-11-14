#[cfg(target_os = "macos")]
mod fullscreen;
mod google;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use google::DeckError;
use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, Emitter, Manager, Url, Webview, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

const SIGN_IN_LABEL: &str = "google-sign-in";
/// Event the main window hears when the sign-in window is done with: `"done"`
/// once it reached the deck, `"closed"` when the user closed it first.
const SIGN_IN_EVENT: &str = "google-sign-in";

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            load_google_slides,
            open_google_sign_in,
            set_slide_fullscreen
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            if let Some(main) = app.get_webview_window("main") {
                fullscreen::remember_web_view(&main);
            }
            #[cfg(not(target_os = "macos"))]
            let _ = app;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running pecha-player");
}

/// Downloads the Google Slides deck the link points at, as PDF bytes.
///
/// Async (and the download on a blocking thread) because reading cookies
/// deadlocks on Windows when done from a synchronous command.
#[tauri::command]
async fn load_google_slides(
    webview: Webview,
    url: String,
) -> Result<tauri::ipc::Response, DeckError> {
    let id = google::parse_deck_id(&url)?;
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        google::download(&id, |target| cookie_header(&webview, target))
    })
    .await
    .map_err(|e| DeckError::Network {
        detail: e.to_string(),
    })??;
    Ok(tauri::ipc::Response::new(bytes))
}

/// The `Cookie` header the webview's cookie store holds for `url`. Every
/// window shares one store, so this sees what the sign-in window left there.
fn cookie_header(webview: &Webview, url: &Url) -> String {
    match webview.cookies_for_url(url.clone()) {
        Ok(cookies) => cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; "),
        Err(err) => {
            eprintln!("pecha-player: could not read cookies: {err}");
            String::new()
        }
    }
}

/// Opens a window on Google's sign-in page that continues to the deck.
///
/// The window shows Google's own pages and is given no access to the app's
/// commands: no capability names it. Async because creating a window from a
/// synchronous command deadlocks on Windows.
#[tauri::command]
async fn open_google_sign_in(app: AppHandle, url: String) -> Result<(), DeckError> {
    let id = google::parse_deck_id(&url)?;
    if let Some(existing) = app.get_webview_window(SIGN_IN_LABEL) {
        let _ = existing.set_focus();
        return Ok(());
    }

    // Set once the deck has loaded, so that closing the window afterwards is
    // not reported as the user giving up.
    let reached = Arc::new(AtomicBool::new(false));
    let reached_on_load = reached.clone();
    let deck_id = id.clone();

    let mut builder = WebviewWindowBuilder::new(
        &app,
        SIGN_IN_LABEL,
        WebviewUrl::External(google::sign_in_url(&id)),
    )
    .title("Google にログイン")
    .inner_size(480.0, 640.0)
    .min_inner_size(360.0, 480.0)
    .center()
    // Sign-in may pass through the user's own identity provider (Google
    // Workspace with SSO), so any https page is allowed; nothing else is.
    .on_navigation(|url| matches!(url.scheme(), "https" | "about" | "blob"))
    .on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Finished
            && google::is_deck_page(payload.url(), &deck_id)
        {
            reached_on_load.store(true, Ordering::SeqCst);
            if let Err(err) = window.app_handle().emit_to("main", SIGN_IN_EVENT, "done") {
                eprintln!("pecha-player: could not report the sign-in: {err}");
            }
            let _ = window.close();
        }
    });
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main).map_err(|e| DeckError::Network {
            detail: e.to_string(),
        })?;
    }
    let window = builder.build().map_err(|e| DeckError::Network {
        detail: e.to_string(),
    })?;

    let handle = app.clone();
    window.on_window_event(move |event| {
        if matches!(event, WindowEvent::Destroyed) && !reached.load(Ordering::SeqCst) {
            let _ = handle.emit_to("main", SIGN_IN_EVENT, "closed");
        }
    });
    Ok(())
}

/// Lets the slide fill the whole screen, or brings the window back: on macOS
/// with no title bar, menu bar or Dock (`src/fullscreen.rs`), elsewhere the
/// ordinary fullscreen.
#[tauri::command]
async fn set_slide_fullscreen(window: WebviewWindow, on: bool) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let result = fullscreen::set(&window, on);
    #[cfg(not(target_os = "macos"))]
    let result = window.set_fullscreen(on);
    result.map_err(|e| e.to_string())
}
