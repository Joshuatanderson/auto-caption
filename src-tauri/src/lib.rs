mod commands;
mod db;
mod pipeline;

use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("captioner.db");
            let conn = db::init(db_path)?;
            app.manage(db::DbState(Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::extract_audio,
            commands::transcribe,
            commands::generate_ass,
            commands::burn_captions,
            db::get_themes,
            db::get_current_theme,
            db::set_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
