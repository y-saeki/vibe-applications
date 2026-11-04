mod commands;
mod links;

use mailcrew_core::Core;
use tauri::Manager;

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
            app.manage(Core::new(app.path().app_config_dir()?));
            Ok(())
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
