mod accounts;
mod attachments;
mod auth;
mod authres;
mod bimi;
mod card;
mod commands;
mod credentials;
mod error;
mod gmail;
mod image_senders;
mod links;
mod mime;
mod preferences;
mod sanitize;
mod trackpad;
mod views;

use tokio::sync::{oneshot, Mutex};

pub struct AppState {
    accounts: accounts::Accounts,
    /// Gives up the sign-in that is waiting for the browser, if any.
    cancel_sign_in: Mutex<Option<oneshot::Sender<()>>>,
    brands: bimi::Brands,
    /// Read from disk on first use.
    preferences: Mutex<Option<preferences::Preferences>>,
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(links::guard())
        .setup(|app| {
            // What was opened last time is not needed any more. A file a
            // viewer still holds open stays, and goes next time.
            if let Ok(dir) = commands::opened_dir(app.handle()) {
                let _ = std::fs::remove_dir_all(dir);
            }
            trackpad::watch(app.handle());
            Ok(())
        })
        .manage(AppState {
            accounts: accounts::Accounts::new(),
            cancel_sign_in: Mutex::new(None),
            brands: bimi::Brands::new(),
            preferences: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_session,
            commands::sign_in,
            commands::cancel_sign_in,
            commands::sign_out,
            commands::list_inbox,
            commands::get_message,
            commands::get_brand,
            commands::open_attachment,
            commands::save_attachment,
            commands::get_preferences,
            commands::save_preferences,
            commands::list_labels,
            commands::create_label,
            commands::archive,
            commands::apply_label,
            commands::set_aside,
            commands::set_starred,
            commands::list_views,
            commands::save_views,
            commands::list_image_senders,
            commands::save_image_senders,
            commands::open_link,
        ])
        .run(tauri::generate_context!())
        .expect("error while running mailcrew");
}
