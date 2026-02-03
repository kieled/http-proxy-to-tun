//! System tray functionality for ProxyVPN
//!
//! Provides system tray icon with menu for quick access when minimized.

use std::sync::mpsc::{self, Receiver, Sender};

use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{
    menu::MenuId, Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

/// Menu item IDs
const MENU_SHOW: &str = "show";
const MENU_CONNECT: &str = "connect";
const MENU_DISCONNECT: &str = "disconnect";
const MENU_QUIT: &str = "quit";

/// Events from the tray to the application
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// User clicked "Show" or double-clicked the tray icon
    Show,
    /// User clicked "Connect"
    Connect,
    /// User clicked "Disconnect"
    Disconnect,
    /// User clicked "Quit"
    Quit,
}

/// Manages the system tray icon and menu
pub struct SystemTray {
    tray_icon: Option<TrayIcon>,
    #[allow(dead_code)]
    menu_show: MenuItem,
    menu_connect: MenuItem,
    menu_disconnect: MenuItem,
}

impl Drop for SystemTray {
    fn drop(&mut self) {
        // Clear the global event handlers
        MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
        TrayIconEvent::set_event_handler(None::<fn(TrayIconEvent)>);

        // Drop the tray icon
        self.tray_icon.take();

        tracing::debug!("SystemTray dropped");
    }
}

impl SystemTray {
    /// Create a new system tray with the given icon
    pub fn new(event_tx: Sender<TrayEvent>) -> anyhow::Result<Self> {
        // Create menu items
        let menu_show = MenuItem::with_id(MenuId::new(MENU_SHOW), "Show", true, None);
        let menu_connect = MenuItem::with_id(MenuId::new(MENU_CONNECT), "Connect", true, None);
        let menu_disconnect = MenuItem::with_id(MenuId::new(MENU_DISCONNECT), "Disconnect", false, None);
        let menu_quit = MenuItem::with_id(MenuId::new(MENU_QUIT), "Quit", true, None);

        // Build menu
        let menu = Menu::new();
        menu.append(&menu_show)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&menu_connect)?;
        menu.append(&menu_disconnect)?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&menu_quit)?;

        // Load icon
        let icon = load_icon()?;

        // Create tray icon
        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ProxyVPN - Disconnected")
            .with_icon(icon)
            .build()?;

        // Set up menu event handler
        let event_tx_menu = event_tx.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let tray_event = match event.id.0.as_str() {
                MENU_SHOW => Some(TrayEvent::Show),
                MENU_CONNECT => Some(TrayEvent::Connect),
                MENU_DISCONNECT => Some(TrayEvent::Disconnect),
                MENU_QUIT => Some(TrayEvent::Quit),
                _ => None,
            };

            if let Some(e) = tray_event {
                let _ = event_tx_menu.send(e);
            }
        }));

        // Set up tray icon event handler (for click/double-click)
        // Use Click for single-click behavior which is more natural on Linux
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            match event {
                TrayIconEvent::Click { button: tray_icon::MouseButton::Left, .. } |
                TrayIconEvent::DoubleClick { button: tray_icon::MouseButton::Left, .. } => {
                    let _ = event_tx.send(TrayEvent::Show);
                }
                _ => {}
            }
        }));

        Ok(Self {
            tray_icon: Some(tray_icon),
            menu_show,
            menu_connect,
            menu_disconnect,
        })
    }

    /// Update the tray tooltip based on connection status
    #[allow(dead_code)]
    pub fn set_tooltip(&self, tooltip: &str) {
        // Note: tray-icon doesn't support changing tooltip after creation
        // This is a limitation we'll work around by recreating if needed
        tracing::debug!("Tray tooltip update requested: {}", tooltip);
    }

    /// Update menu items based on connection status
    pub fn set_connected(&self, connected: bool) {
        self.menu_connect.set_enabled(!connected);
        self.menu_disconnect.set_enabled(connected);
    }
}

/// Load the tray icon from embedded resources or generate a simple one
fn load_icon() -> anyhow::Result<Icon> {
    // Try to load from file first
    let icon_paths = [
        // Check relative to executable
        "assets/icon.png",
        // Check in config dir
    ];

    for path in icon_paths {
        if let Ok(icon) = load_icon_from_file(path) {
            return Ok(icon);
        }
    }

    // Fall back to a simple generated icon (green circle for VPN)
    Ok(create_default_icon())
}

fn load_icon_from_file(path: &str) -> anyhow::Result<Icon> {
    let img = image::open(path)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    let icon = Icon::from_rgba(rgba.into_raw(), width, height)?;
    Ok(icon)
}

/// Create a simple default icon (32x32 green circle)
fn create_default_icon() -> Icon {
    const SIZE: u32 = 32;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];

    let center = SIZE as f32 / 2.0;
    let radius = center - 2.0;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * SIZE + x) * 4) as usize;

            if dist <= radius {
                // Green color (matching our accent color)
                rgba[idx] = 74;      // R
                rgba[idx + 1] = 185; // G
                rgba[idx + 2] = 128; // B
                rgba[idx + 3] = 255; // A
            } else if dist <= radius + 1.0 {
                // Anti-aliased edge
                let alpha = ((radius + 1.0 - dist) * 255.0) as u8;
                rgba[idx] = 74;
                rgba[idx + 1] = 185;
                rgba[idx + 2] = 128;
                rgba[idx + 3] = alpha;
            }
            // else: transparent (already 0)
        }
    }

    Icon::from_rgba(rgba, SIZE, SIZE).expect("Failed to create default icon")
}

/// Create a channel for tray events
pub fn create_tray_channel() -> (Sender<TrayEvent>, Receiver<TrayEvent>) {
    mpsc::channel()
}
