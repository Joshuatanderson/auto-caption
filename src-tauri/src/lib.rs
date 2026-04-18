mod commands;
mod pipeline;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::extract_audio,
            commands::transcribe,
            commands::generate_ass,
            commands::burn_captions,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
