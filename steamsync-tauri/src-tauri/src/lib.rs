//! steamsync-tauri: native Tauri 2 GUI for adding non-Steam game launchers
//! (Epic Games Store, itch.io, Xbox, legendary) to Steam as shortcuts.

mod api;
mod commands;
mod error;
mod launchers;
mod steam;
mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::detect_games,
            commands::apply_changes,
            commands::auto_detect_steam_path,
            commands::fetch_art_previews,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
