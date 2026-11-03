mod auth;
mod card;
mod commands;
mod credentials;
mod error;
mod gmail;
mod links;
mod mime;
mod sanitize;

use tokio::sync::{oneshot, Mutex};

pub struct AppState {
    gmail: gmail::Gmail,
    /// Gives up the sign-in that is waiting for the browser, if any.
    cancel_sign_in: Mutex<Option<oneshot::Sender<()>>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(links::guard())
        .manage(AppState {
            gmail: gmail::Gmail::new(),
            cancel_sign_in: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_session,
            commands::sign_in,
            commands::cancel_sign_in,
            commands::sign_out,
            commands::list_inbox,
            commands::list_labels,
            commands::create_label,
            commands::archive,
            commands::apply_label,
            commands::set_aside,
            commands::set_starred,
            commands::open_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running mailcrew");
}
