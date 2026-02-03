#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod bridge;
mod icons;
mod theme;
#[cfg(feature = "tray")]
mod tray;
mod views;

use app::{AppSettings, ProxyVpnApp};

fn main() -> iced::Result {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("proxyvpn=info".parse().unwrap())
                .add_directive("iced=warn".parse().unwrap()),
        )
        .init();

    tracing::info!("Starting ProxyVPN GUI");

    // Initialize GTK for system tray on Linux
    #[cfg(all(target_os = "linux", feature = "tray"))]
    {
        if let Err(e) = gtk::init() {
            tracing::warn!("Failed to initialize GTK for tray: {}", e);
        }
    }

    // Load settings
    let settings = load_settings().unwrap_or_default();

    // Use daemon mode so the app doesn't exit when all windows close (for tray support)
    iced::daemon(move || ProxyVpnApp::new(settings.clone()), ProxyVpnApp::update, ProxyVpnApp::view)
        .title(ProxyVpnApp::title)
        .subscription(ProxyVpnApp::subscription)
        .theme(ProxyVpnApp::theme)
        .font(include_bytes!("../assets/fonts/fa-solid-900.ttf").as_slice())
        .font(include_bytes!("../assets/fonts/fa-brands-400.ttf").as_slice())
        .run()
}

fn load_settings() -> Option<AppSettings> {
    let config_dir = dirs::config_dir()?.join("proxyvpn");
    let settings_path = config_dir.join("settings.json");

    std::fs::read_to_string(settings_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}
