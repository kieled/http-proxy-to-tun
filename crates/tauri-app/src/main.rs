// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use http_tun_desktop::commands;
use http_tun_desktop::state::AppState;
use http_tun_desktop::tray;
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize application state
            let state = AppState::new().expect("Failed to initialize app state");
            app.manage(state);

            // Create system tray
            tray::create_tray(app.handle())?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Proxy commands
            commands::get_proxies,
            commands::get_proxy,
            commands::add_proxy,
            commands::update_proxy,
            commands::delete_proxy,
            commands::reorder_proxies,
            // Connection commands
            commands::get_connection_status,
            commands::connect,
            commands::disconnect,
            // Settings commands
            commands::get_settings,
            commands::update_settings,
            commands::set_theme,
            // Utility commands
            commands::check_privileges_command,
            commands::can_elevate_privileges,
            commands::set_capabilities,
            // Network info commands
            commands::refresh_public_ip,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
