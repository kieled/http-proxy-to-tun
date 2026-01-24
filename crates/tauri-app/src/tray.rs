use crate::connection::ConnectionStatus;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Manager, Runtime,
};

// Embed the icon at compile time
const TRAY_ICON: &[u8] = include_bytes!("../icons/32x32.png");

pub fn create_tray<R: Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let show_i = MenuItem::with_id(app, "show", "Show Window", true, None::<&str>)?;
    let hide_i = MenuItem::with_id(app, "hide", "Hide Window", true, None::<&str>)?;
    let separator1 = MenuItem::with_id(app, "sep1", "---", false, None::<&str>)?;
    let connect_i = MenuItem::with_id(app, "connect", "Connect", true, None::<&str>)?;
    let disconnect_i = MenuItem::with_id(app, "disconnect", "Disconnect", false, None::<&str>)?;
    let separator2 = MenuItem::with_id(app, "sep2", "---", false, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &show_i,
            &hide_i,
            &separator1,
            &connect_i,
            &disconnect_i,
            &separator2,
            &quit_i,
        ],
    )?;

    // Load tray icon
    let icon = Image::from_bytes(TRAY_ICON)
        .expect("Failed to load tray icon");

    let tray = TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .tooltip("HTTP Tunnel - Disconnected")
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "hide" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            "connect" => {
                // Trigger connect via frontend
                let _ = app.emit("tray-connect", ());
            }
            "disconnect" => {
                // Trigger disconnect via frontend
                let _ = app.emit("tray-disconnect", ());
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    // Store tray reference in app state for later updates
    app.manage(TrayState(std::sync::Mutex::new(Some(tray))));

    Ok(())
}

/// Wrapper to store tray icon reference
pub struct TrayState<R: Runtime>(pub std::sync::Mutex<Option<tauri::tray::TrayIcon<R>>>);

pub fn update_tray_tooltip<R: Runtime>(
    app: &tauri::AppHandle<R>,
    status: ConnectionStatus,
    proxy_name: Option<&str>,
) -> tauri::Result<()> {
    if let Some(tray_state) = app.try_state::<TrayState<R>>() {
        let guard = tray_state.0.lock().unwrap();
        if let Some(tray) = guard.as_ref() {
            let tooltip = match status {
                ConnectionStatus::Connected => {
                    if let Some(name) = proxy_name {
                        format!("HTTP Tunnel - Connected to {}", name)
                    } else {
                        "HTTP Tunnel - Connected".to_string()
                    }
                }
                ConnectionStatus::Connecting => "HTTP Tunnel - Connecting...".to_string(),
                ConnectionStatus::Disconnecting => "HTTP Tunnel - Disconnecting...".to_string(),
                ConnectionStatus::Error => "HTTP Tunnel - Error".to_string(),
                ConnectionStatus::Disconnected => "HTTP Tunnel - Disconnected".to_string(),
            };
            let _ = tray.set_tooltip(Some(&tooltip));
        }
    }
    Ok(())
}
