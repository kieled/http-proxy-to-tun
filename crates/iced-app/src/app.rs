use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use crate::icons::{codes as fa, icon};
use iced::widget::{button, column, container, row, text, Space};
use iced::window;
use iced::{Alignment, Background, Border, Element, Length, Padding, Subscription, Task, Theme};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};

use crate::bridge::{BackendCommand, BackendEvent, ConnectArgs};
use crate::theme::colors::dark;
use crate::theme::styles;
#[cfg(feature = "tray")]
use crate::tray::{create_tray_channel, SystemTray, TrayEvent};
use crate::views::{logs_view, main_view, proxies_view, settings_view};


/// A saved proxy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedProxy {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    #[serde(skip)] // Password stored in keyring, not JSON
    pub password: Option<String>,
    pub tun_name: String,
    pub tun_cidr: String,
}

impl SavedProxy {
    pub fn new(name: String, host: String, port: u16) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            host,
            port,
            username: None,
            password: None,
            tun_name: "tun0".to_string(),
            tun_cidr: "10.255.255.1/30".to_string(),
        }
    }

    /// Load password from keyring
    pub fn load_password(&mut self) {
        match keyring::Entry::new("proxyvpn", &self.id) {
            Ok(entry) => match entry.get_password() {
                Ok(password) => {
                    tracing::debug!("Loaded password from keyring for proxy {}", self.id);
                    self.password = Some(password);
                }
                Err(e) => {
                    tracing::debug!("No password in keyring for proxy {}: {}", self.id, e);
                }
            },
            Err(e) => {
                tracing::warn!("Failed to access keyring for proxy {}: {}", self.id, e);
            }
        }
    }

    /// Save password to keyring
    pub fn save_password(&self) {
        let Some(ref password) = self.password else {
            tracing::debug!("No password to save for proxy {}", self.id);
            return;
        };
        match keyring::Entry::new("proxyvpn", &self.id) {
            Ok(entry) => match entry.set_password(password) {
                Ok(()) => {
                    tracing::debug!("Saved password to keyring for proxy {}", self.id);
                }
                Err(e) => {
                    tracing::error!("Failed to save password to keyring for proxy {}: {}", self.id, e);
                }
            },
            Err(e) => {
                tracing::error!("Failed to access keyring for proxy {}: {}", self.id, e);
            }
        }
    }

    /// Delete password from keyring
    pub fn delete_password(&self) {
        if let Ok(entry) = keyring::Entry::new("proxyvpn", &self.id) {
            let _ = entry.delete_credential();
        }
    }
}

/// Application settings persisted to disk
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub minimize_to_tray: bool,
    #[serde(default)]
    pub start_minimized: bool,
    #[serde(default)]
    pub killswitch_enabled: bool,
    #[serde(default)]
    pub theme_mode: ThemeMode,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    #[default]
    Dark,
    Light,
    System,
}

/// Current view in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum View {
    #[default]
    Main,
    Proxies,
    Settings,
    Logs,
}

/// Connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

/// Log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Error => "ERROR",
        }
    }
}

/// Form state for adding/editing a proxy
#[derive(Debug, Clone, Default)]
pub struct ProxyFormState {
    pub editing_id: Option<String>,
    pub name: String,
    pub host: String,
    pub port: String,
    pub username: String,
    pub password: String,
    pub tun_name: String,
    pub tun_cidr: String,
}

impl ProxyFormState {
    pub fn new() -> Self {
        Self {
            editing_id: None,
            name: String::new(),
            host: String::new(),
            port: "8080".to_string(),
            username: String::new(),
            password: String::new(),
            tun_name: "tun0".to_string(),
            tun_cidr: "10.255.255.1/30".to_string(),
        }
    }

    pub fn from_proxy(proxy: &SavedProxy) -> Self {
        Self {
            editing_id: Some(proxy.id.clone()),
            name: proxy.name.clone(),
            host: proxy.host.clone(),
            port: proxy.port.to_string(),
            username: proxy.username.clone().unwrap_or_default(),
            password: proxy.password.clone().unwrap_or_default(),
            tun_name: proxy.tun_name.clone(),
            tun_cidr: proxy.tun_cidr.clone(),
        }
    }
}

/// Main application state
pub struct ProxyVpnApp {
    pub current_view: View,
    pub connection_status: ConnectionStatus,
    pub connection_error: Option<String>,
    pub connected_at: Option<std::time::Instant>,
    pub public_ip: Option<String>,

    // Proxy management
    pub proxies: Vec<SavedProxy>,
    pub selected_proxy_id: Option<String>,
    pub proxy_form: ProxyFormState,
    pub show_proxy_form: bool,

    // Settings
    pub settings: AppSettings,

    // Backend
    pub backend_tx: Option<mpsc::Sender<BackendCommand>>,
    pub backend_event_rx: Option<Arc<Mutex<mpsc::Receiver<BackendEvent>>>>,

    // Logs (VecDeque for O(1) removal from front)
    pub logs: VecDeque<LogEntry>,

    // System tray
    #[cfg(feature = "tray")]
    pub tray: Option<SystemTray>,
    #[cfg(feature = "tray")]
    pub tray_event_rx: Option<std::sync::mpsc::Receiver<TrayEvent>>,
    pub should_exit: bool,

    // Track if window is hidden to tray (closed but app still running)
    #[cfg(feature = "tray")]
    pub hidden_to_tray: bool,
}

/// Messages for the application
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    SwitchView(View),

    // Connection
    Connect,
    Disconnect,
    ConnectionError(String),

    // Proxy management
    SelectProxy(String),
    ShowProxyForm,
    EditProxy(String),
    HideProxyForm,
    DeleteProxy(String),
    SaveProxy,

    // Proxy form fields
    ProxyNameChanged(String),
    ProxyHostChanged(String),
    ProxyPortChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    TunNameChanged(String),
    TunCidrChanged(String),

    // Settings
    ToggleKillswitch(bool),
    ToggleMinimizeToTray(bool),
    ToggleStartMinimized(bool),
    ThemeModeChanged(ThemeMode),

    // Backend
    BackendReady(mpsc::Sender<BackendCommand>, Arc<Mutex<mpsc::Receiver<BackendEvent>>>),
    BackendEvent(BackendEvent),

    // Logs
    ClearLogs,

    // IP
    #[allow(dead_code)]
    FetchPublicIp,
    PublicIpResult(Option<String>),

    // Window events
    WindowCloseRequested(window::Id),
    ShowWindow,
    ExitApp,

    // Tray events
    #[cfg(feature = "tray")]
    TrayEvent(TrayEvent),

    // Timer
    Tick,
    /// Fast GTK event pump for responsive tray menu
    #[cfg(feature = "tray")]
    GtkPump,
    None,
}

impl ProxyVpnApp {
    pub fn new(settings: AppSettings) -> (Self, Task<Message>) {
        let mut proxies = load_proxies();
        // Load passwords from keyring
        for proxy in &mut proxies {
            proxy.load_password();
        }
        let selected_proxy_id = proxies.first().map(|p| p.id.clone());

        // Create system tray on startup if minimize_to_tray is enabled
        #[cfg(feature = "tray")]
        let (tray, tray_event_rx): (Option<SystemTray>, Option<std::sync::mpsc::Receiver<TrayEvent>>) =
            if settings.minimize_to_tray {
                let (tx, rx) = create_tray_channel();
                match SystemTray::new(tx) {
                    Ok(tray) => {
                        tracing::info!("System tray created on startup");
                        (Some(tray), Some(rx))
                    }
                    Err(e) => {
                        tracing::error!("Failed to create system tray on startup: {}", e);
                        (None, None)
                    }
                }
            } else {
                (None, None)
            };

        let app = Self {
            current_view: View::Main,
            connection_status: ConnectionStatus::Disconnected,
            connection_error: None,
            connected_at: None,
            public_ip: None,
            proxies,
            selected_proxy_id,
            proxy_form: ProxyFormState::new(),
            show_proxy_form: false,
            settings,
            backend_tx: None,
            backend_event_rx: None,
            logs: {
                let mut logs = VecDeque::with_capacity(512);
                logs.push_back(LogEntry {
                    timestamp: current_timestamp(),
                    level: LogLevel::Info,
                    message: "Application started".to_string(),
                });
                logs
            },
            #[cfg(feature = "tray")]
            tray,
            #[cfg(feature = "tray")]
            tray_event_rx,
            should_exit: false,
            #[cfg(feature = "tray")]
            hidden_to_tray: false,
        };

        // Start backend initialization
        let backend_task = Task::perform(crate::bridge::start_backend(), |result| match result {
            Ok((tx, rx)) => Message::BackendReady(tx, Arc::new(Mutex::new(rx))),
            Err(e) => Message::ConnectionError(e.to_string()),
        });

        // Open the initial window (daemon mode doesn't do this automatically)
        let window_settings = window::Settings {
            size: iced::Size::new(480.0, 640.0),
            exit_on_close_request: false,
            ..Default::default()
        };
        let (_id, window_task) = window::open(window_settings);

        // Combine tasks
        let init_task = Task::batch([
            backend_task,
            window_task.map(|_| Message::None),
        ]);

        (app, init_task)
    }

    #[allow(dead_code)]
    pub fn has_selected_proxy(&self) -> bool {
        self.selected_proxy_id.is_some() && !self.proxies.is_empty()
    }

    pub fn get_selected_proxy(&self) -> Option<&SavedProxy> {
        self.selected_proxy_id
            .as_ref()
            .and_then(|id| self.proxies.iter().find(|p| &p.id == id))
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SwitchView(view) => {
                self.current_view = view;
                Task::none()
            }

            Message::Connect => self.handle_connect(),
            Message::Disconnect => self.handle_disconnect(),

            Message::ConnectionError(error) => {
                self.connection_status = ConnectionStatus::Error;
                self.connection_error = Some(error.clone());
                self.add_log(LogLevel::Error, &format!("Connection error: {}", error));
                Task::none()
            }

            // Proxy management
            Message::SelectProxy(id) => {
                self.selected_proxy_id = Some(id);
                Task::none()
            }

            Message::ShowProxyForm => {
                self.proxy_form = ProxyFormState::new();
                self.show_proxy_form = true;
                Task::none()
            }

            Message::EditProxy(id) => {
                if let Some(proxy) = self.proxies.iter().find(|p| p.id == id) {
                    self.proxy_form = ProxyFormState::from_proxy(proxy);
                    self.show_proxy_form = true;
                }
                Task::none()
            }

            Message::HideProxyForm => {
                self.show_proxy_form = false;
                Task::none()
            }

            Message::DeleteProxy(id) => {
                // Delete password from keyring
                if let Some(proxy) = self.proxies.iter().find(|p| p.id == id) {
                    proxy.delete_password();
                }
                self.proxies.retain(|p| p.id != id);
                if self.selected_proxy_id.as_ref() == Some(&id) {
                    self.selected_proxy_id = self.proxies.first().map(|p| p.id.clone());
                }
                save_proxies(&self.proxies);
                Task::none()
            }

            Message::SaveProxy => {
                if let Ok(port) = self.proxy_form.port.parse::<u16>() {
                    if let Some(edit_id) = &self.proxy_form.editing_id {
                        // Update existing proxy
                        if let Some(proxy) = self.proxies.iter_mut().find(|p| &p.id == edit_id) {
                            proxy.name = self.proxy_form.name.clone();
                            proxy.host = self.proxy_form.host.clone();
                            proxy.port = port;
                            proxy.username = if self.proxy_form.username.is_empty() {
                                None
                            } else {
                                Some(self.proxy_form.username.clone())
                            };
                            proxy.password = if self.proxy_form.password.is_empty() {
                                None
                            } else {
                                Some(self.proxy_form.password.clone())
                            };
                            proxy.tun_name = self.proxy_form.tun_name.clone();
                            proxy.tun_cidr = self.proxy_form.tun_cidr.clone();
                            // Save password to keyring
                            proxy.save_password();
                        }
                    } else {
                        // Create new proxy
                        let mut proxy = SavedProxy::new(
                            self.proxy_form.name.clone(),
                            self.proxy_form.host.clone(),
                            port,
                        );
                        proxy.username = if self.proxy_form.username.is_empty() {
                            None
                        } else {
                            Some(self.proxy_form.username.clone())
                        };
                        proxy.password = if self.proxy_form.password.is_empty() {
                            None
                        } else {
                            Some(self.proxy_form.password.clone())
                        };
                        proxy.tun_name = self.proxy_form.tun_name.clone();
                        proxy.tun_cidr = self.proxy_form.tun_cidr.clone();
                        // Save password to keyring
                        proxy.save_password();

                        if self.proxies.is_empty() {
                            self.selected_proxy_id = Some(proxy.id.clone());
                        }
                        self.proxies.push(proxy);
                    }
                    save_proxies(&self.proxies);
                    self.show_proxy_form = false;
                }
                Task::none()
            }

            // Proxy form fields
            Message::ProxyNameChanged(name) => {
                self.proxy_form.name = name;
                Task::none()
            }
            Message::ProxyHostChanged(host) => {
                self.proxy_form.host = host;
                Task::none()
            }
            Message::ProxyPortChanged(port) => {
                self.proxy_form.port = port;
                Task::none()
            }
            Message::UsernameChanged(username) => {
                self.proxy_form.username = username;
                Task::none()
            }
            Message::PasswordChanged(password) => {
                self.proxy_form.password = password;
                Task::none()
            }
            Message::TunNameChanged(name) => {
                self.proxy_form.tun_name = name;
                Task::none()
            }
            Message::TunCidrChanged(cidr) => {
                self.proxy_form.tun_cidr = cidr;
                Task::none()
            }

            // Settings
            Message::ToggleKillswitch(enabled) => {
                self.settings.killswitch_enabled = enabled;
                self.save_settings();
                Task::none()
            }
            Message::ToggleMinimizeToTray(enabled) => {
                self.settings.minimize_to_tray = enabled;
                self.save_settings();
                Task::none()
            }
            Message::ToggleStartMinimized(enabled) => {
                self.settings.start_minimized = enabled;
                self.save_settings();
                Task::none()
            }
            Message::ThemeModeChanged(mode) => {
                self.settings.theme_mode = mode;
                self.save_settings();
                Task::none()
            }

            // Backend
            Message::BackendReady(tx, rx) => {
                self.backend_tx = Some(tx);
                self.backend_event_rx = Some(rx);
                self.add_log(LogLevel::Info, "Backend initialized");
                Task::none()
            }

            Message::BackendEvent(event) => {
                match event {
                    BackendEvent::Connected => {
                        self.connection_status = ConnectionStatus::Connected;
                        self.connected_at = Some(std::time::Instant::now());
                        self.connection_error = None;
                        self.add_log(LogLevel::Info, "VPN connected");
                        // Fetch public IP after connection
                        return Task::perform(fetch_public_ip(), Message::PublicIpResult);
                    }
                    BackendEvent::Disconnected => {
                        self.connection_status = ConnectionStatus::Disconnected;
                        self.connected_at = None;
                        self.public_ip = None;
                        self.add_log(LogLevel::Info, "VPN disconnected");
                    }
                    BackendEvent::Error(error) => {
                        self.connection_status = ConnectionStatus::Error;
                        self.connection_error = Some(error.clone());
                        self.public_ip = None;
                        self.add_log(LogLevel::Error, &format!("VPN error: {}", error));
                    }
                }
                Task::none()
            }

            // Logs
            Message::ClearLogs => {
                self.logs.clear();
                self.add_log(LogLevel::Info, "Logs cleared");
                Task::none()
            }

            // IP
            Message::FetchPublicIp => Task::perform(fetch_public_ip(), Message::PublicIpResult),

            Message::PublicIpResult(ip) => {
                if let Some(ref ip_addr) = ip {
                    self.add_log(LogLevel::Info, &format!("Public IP: {}", ip_addr));
                }
                self.public_ip = ip;
                Task::none()
            }

            // Window events
            Message::WindowCloseRequested(id) => {
                #[cfg(feature = "tray")]
                if self.settings.minimize_to_tray {
                    // Create tray icon on first close-to-tray (stays visible for app lifetime)
                    if self.tray.is_none() {
                        let (tx, rx) = create_tray_channel();
                        match SystemTray::new(tx) {
                            Ok(tray) => {
                                tracing::info!("System tray created");
                                self.tray = Some(tray);
                                self.tray_event_rx = Some(rx);

                                // Pump GTK events to make tray icon appear immediately
                                #[cfg(target_os = "linux")]
                                {
                                    for _ in 0..5 {
                                        while gtk::events_pending() {
                                            gtk::main_iteration_do(false);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to create system tray: {}", e);
                                // Fall through to exit the app
                                self.should_exit = true;
                                return iced::exit();
                            }
                        }
                    }

                    // Close window but keep app running in tray
                    self.add_log(LogLevel::Info, "Closed to tray");
                    self.hidden_to_tray = true;
                    return window::close(id);
                }
                // Actually exit the app
                self.should_exit = true;
                iced::exit()
            }

            Message::ShowWindow => {
                // Open a new window when restoring from tray
                #[cfg(feature = "tray")]
                if self.hidden_to_tray {
                    self.add_log(LogLevel::Info, "Restored from tray");
                    self.hidden_to_tray = false;

                    // Note: We keep the tray icon visible even when window is open.
                    // set_visible(false) doesn't work reliably on Linux with libayatana-appindicator.
                    // The tray provides quick access to connect/disconnect even when window is open.
                    tracing::info!("Window restored from tray (tray remains visible)");

                    let settings = window::Settings {
                        size: iced::Size::new(480.0, 640.0),
                        exit_on_close_request: false,
                        ..Default::default()
                    };
                    let (id, open_task) = window::open(settings);
                    tracing::info!("Opening new window with id: {:?}", id);
                    return open_task.map(|_| Message::None);
                }
                Task::none()
            }

            Message::ExitApp => {
                self.should_exit = true;
                iced::exit()
            }

            // Tray events
            #[cfg(feature = "tray")]
            Message::TrayEvent(event) => match event {
                TrayEvent::Show => self.update(Message::ShowWindow),
                TrayEvent::Connect => self.update(Message::Connect),
                TrayEvent::Disconnect => self.update(Message::Disconnect),
                TrayEvent::Quit => self.update(Message::ExitApp),
            },

            Message::Tick => {
                // Update tray menu based on connection status
                #[cfg(feature = "tray")]
                if let Some(ref tray) = self.tray {
                    let connected = self.connection_status == ConnectionStatus::Connected;
                    tray.set_connected(connected);
                }

                // Poll tray events
                #[cfg(feature = "tray")]
                if let Some(task) = self.poll_tray_events() {
                    return task;
                }

                // Poll backend events
                if let Some(task) = self.poll_backend_events() {
                    return task;
                }

                Task::none()
            }

            // Fast GTK event pump for responsive tray menu (runs ~60fps when tray is visible)
            #[cfg(feature = "tray")]
            Message::GtkPump => {
                #[cfg(target_os = "linux")]
                {
                    while gtk::events_pending() {
                        gtk::main_iteration_do(false);
                    }
                }

                // Also poll tray events during fast pump
                if let Some(task) = self.poll_tray_events() {
                    return task;
                }

                Task::none()
            }
            Message::None => Task::none(),
        }
    }

    pub fn title(&self, _window: window::Id) -> String {
        "ProxyVPN".to_string()
    }

    pub fn view(&self, _window: window::Id) -> Element<'_, Message> {
        let content: Element<'_, Message> = match self.current_view {
            View::Main => main_view::view(self),
            View::Proxies => proxies_view::view(self),
            View::Settings => settings_view::view(self),
            View::Logs => logs_view::view(self),
        };

        let nav_bar = self.view_nav_bar();

        let main_content = column![nav_bar, content]
            .spacing(0)
            .width(Length::Fill)
            .height(Length::Fill);

        container(main_content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(dark::BACKGROUND)),
                ..Default::default()
            })
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let timer = iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick);

        // Subscribe to window close events
        let window_events = window::close_requests().map(Message::WindowCloseRequested);

        // Note: Backend events are now polled during the Tick handler instead of using a subscription
        // This is because iced 0.14's Subscription::run_with requires fn pointers and can't capture state

        // Fast timer for GTK event pumping - needed whenever tray exists for menu responsiveness
        #[cfg(feature = "tray")]
        let gtk_pump = if self.tray.is_some() {
            iced::time::every(Duration::from_millis(16)).map(|_| Message::GtkPump)
        } else {
            Subscription::none()
        };

        #[cfg(feature = "tray")]
        return Subscription::batch([timer, window_events, gtk_pump]);

        #[cfg(not(feature = "tray"))]
        Subscription::batch([timer, window_events])
    }

    /// Poll tray events (called during tick)
    #[cfg(feature = "tray")]
    pub fn poll_tray_events(&mut self) -> Option<Task<Message>> {
        let rx = self.tray_event_rx.as_ref()?;

        match rx.try_recv() {
            Ok(event) => Some(self.update(Message::TrayEvent(event))),
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                // Channel disconnected - the tray was destroyed
                // We can't clear tray_event_rx here due to borrow, but that's okay
                // It will keep returning Disconnected which is fine
                tracing::debug!("Tray event channel disconnected");
                None
            }
        }
    }

    /// Poll backend events (called during tick)
    pub fn poll_backend_events(&mut self) -> Option<Task<Message>> {
        // Get the event first, then drop all borrows before calling update
        let event = self.backend_event_rx.as_ref().and_then(|rx| {
            rx.try_lock().ok().and_then(|mut guard| {
                match guard.try_recv() {
                    Ok(event) => Some(event),
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => None,
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        tracing::warn!("Backend event channel disconnected");
                        None
                    }
                }
            })
        });

        event.map(|e| self.update(Message::BackendEvent(e)))
    }

    pub fn theme(&self, _window: window::Id) -> Theme {
        match self.settings.theme_mode {
            ThemeMode::Dark => Theme::Dark,
            ThemeMode::Light => Theme::Light,
            ThemeMode::System => {
                if is_system_dark_mode() {
                    Theme::Dark
                } else {
                    Theme::Light
                }
            }
        }
    }

    fn view_nav_bar(&self) -> Element<'_, Message> {
        // Back button for sub-views, Settings button for main
        let left_content: Element<'_, Message> = if self.current_view == View::Main {
            button(
                row![
                    icon(fa::GEAR, 14.0),
                    Space::new().width(6),
                    text("Settings").size(13).color(dark::TEXT_SECONDARY),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::SwitchView(View::Settings))
            .padding(Padding::from([8, 12]))
            .style(styles::nav_button_style)
            .into()
        } else {
            button(
                row![
                    icon(fa::CHEVRON_LEFT, 12.0),
                    Space::new().width(6),
                    text("Back").size(13).color(dark::TEXT_SECONDARY),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::SwitchView(View::Main))
            .padding(Padding::from([8, 12]))
            .style(styles::nav_button_style)
            .into()
        };

        // Center content: proxy selector or view title
        let center_content: Element<'_, Message> = if self.current_view == View::Main {
            self.view_proxy_selector()
        } else {
            let title = match self.current_view {
                View::Proxies => "Manage Proxies",
                View::Settings => "Settings",
                View::Logs => "Logs",
                View::Main => "",
            };
            text(title).size(15).color(dark::TEXT).into()
        };

        // Logs button on main view
        let right_content: Element<'_, Message> = if self.current_view == View::Main {
            button(
                row![
                    icon(fa::SCROLL, 14.0),
                    Space::new().width(6),
                    text("Logs").size(13).color(dark::TEXT_SECONDARY),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::SwitchView(View::Logs))
            .padding(Padding::from([8, 12]))
            .style(styles::nav_button_style)
            .into()
        } else {
            Space::new().width(80).into()
        };

        container(
            row![
                left_content,
                Space::new().width(Length::Fill),
                center_content,
                Space::new().width(Length::Fill),
                right_content,
            ]
            .align_y(Alignment::Center)
            .padding(Padding::from([12, 16])),
        )
        .width(Length::Fill)
        .style(|_| container::Style {
            border: Border {
                color: dark::BORDER,
                width: 0.0,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
    }

    fn view_proxy_selector(&self) -> Element<'_, Message> {
        if self.proxies.is_empty() {
            button(
                row![
                    icon(fa::PLUS, 12.0),
                    Space::new().width(6),
                    text("Add Proxy").size(13).color(dark::TEXT_SECONDARY),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::SwitchView(View::Proxies))
            .padding(Padding::from([8, 14]))
            .style(styles::proxy_selector_style)
            .into()
        } else {
            let selected = self.get_selected_proxy();
            let display_text = selected
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Select Proxy".to_string());

            button(
                row![
                    text(display_text).size(14).color(dark::TEXT),
                    Space::new().width(8),
                    icon(fa::CHEVRON_DOWN, 10.0),
                ]
                .align_y(Alignment::Center),
            )
            .on_press(Message::SwitchView(View::Proxies))
            .padding(Padding::from([8, 14]))
            .style(styles::proxy_selector_style)
            .into()
        }
    }

    fn handle_connect(&mut self) -> Task<Message> {
        let proxy = match self.get_selected_proxy() {
            Some(p) => p.clone(),
            None => {
                self.connection_error = Some("No proxy selected".to_string());
                return Task::none();
            }
        };

        self.connection_status = ConnectionStatus::Connecting;
        self.connection_error = None;
        self.add_log(
            LogLevel::Info,
            &format!("Connecting to {}:{}", proxy.host, proxy.port),
        );

        if let Some(tx) = &self.backend_tx {
            let args = ConnectArgs {
                proxy_host: proxy.host,
                proxy_port: proxy.port,
                username: proxy.username,
                password: proxy.password,
                tun_name: proxy.tun_name,
                tun_cidr: proxy.tun_cidr,
                killswitch: self.settings.killswitch_enabled,
            };

            let tx = tx.clone();
            return Task::perform(
                async move {
                    let _ = tx.send(BackendCommand::Connect(args)).await;
                },
                |_| Message::None,
            );
        }

        Task::none()
    }

    fn handle_disconnect(&mut self) -> Task<Message> {
        self.connection_status = ConnectionStatus::Disconnecting;
        self.add_log(LogLevel::Info, "Disconnecting...");

        if let Some(tx) = &self.backend_tx {
            let tx = tx.clone();
            return Task::perform(
                async move {
                    let _ = tx.send(BackendCommand::Disconnect).await;
                },
                |_| Message::None,
            );
        }

        Task::none()
    }

    pub fn add_log(&mut self, level: LogLevel, message: &str) {
        self.logs.push_back(LogEntry {
            timestamp: current_timestamp(),
            level,
            message: message.to_string(),
        });

        // O(1) removal from front with VecDeque
        while self.logs.len() > 500 {
            self.logs.pop_front();
        }
    }

    fn save_settings(&self) {
        let Some(config_dir) = dirs::config_dir() else {
            tracing::warn!("Failed to get config directory for saving settings");
            return;
        };
        let proxyvpn_dir = config_dir.join("proxyvpn");
        if let Err(e) = std::fs::create_dir_all(&proxyvpn_dir) {
            tracing::error!("Failed to create config directory: {}", e);
            return;
        }
        let settings_path = proxyvpn_dir.join("settings.json");
        match serde_json::to_string_pretty(&self.settings) {
            Ok(content) => {
                if let Err(e) = std::fs::write(&settings_path, content) {
                    tracing::error!("Failed to write settings to {:?}: {}", settings_path, e);
                }
            }
            Err(e) => {
                tracing::error!("Failed to serialize settings: {}", e);
            }
        }
    }

    pub fn uptime_string(&self) -> Option<String> {
        self.connected_at.map(|start| {
            let elapsed = start.elapsed();
            let secs = elapsed.as_secs();
            let hours = secs / 3600;
            let mins = (secs % 3600) / 60;
            let secs = secs % 60;
            format!("{:02}:{:02}:{:02}", hours, mins, secs)
        })
    }
}

fn current_timestamp() -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_utc();
    format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second())
}

fn load_proxies() -> Vec<SavedProxy> {
    dirs::config_dir()
        .map(|d| d.join("proxyvpn").join("proxies.json"))
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default()
}

fn save_proxies(proxies: &[SavedProxy]) {
    let Some(config_dir) = dirs::config_dir() else {
        tracing::warn!("Failed to get config directory for saving proxies");
        return;
    };
    let proxyvpn_dir = config_dir.join("proxyvpn");
    if let Err(e) = std::fs::create_dir_all(&proxyvpn_dir) {
        tracing::error!("Failed to create config directory: {}", e);
        return;
    }
    let proxies_path = proxyvpn_dir.join("proxies.json");
    match serde_json::to_string_pretty(proxies) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&proxies_path, content) {
                tracing::error!("Failed to write proxies to {:?}: {}", proxies_path, e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to serialize proxies: {}", e);
        }
    }
}

/// Fetch public IP address from ipify API
async fn fetch_public_ip() -> Option<String> {
    // Use multiple services for redundancy
    let services = [
        "https://api.ipify.org",
        "https://ifconfig.me/ip",
        "https://icanhazip.com",
    ];

    for url in services {
        match reqwest::get(url).await {
            Ok(response) => {
                if let Ok(ip) = response.text().await {
                    let ip = ip.trim().to_string();
                    // Validate it looks like an IP
                    if ip.parse::<std::net::IpAddr>().is_ok() {
                        return Some(ip);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Failed to fetch IP from {}: {}", url, e);
            }
        }
    }

    tracing::warn!("Failed to fetch public IP from all services");
    None
}

#[cfg(target_os = "linux")]
fn is_system_dark_mode() -> bool {
    std::env::var("GTK_THEME")
        .map(|t| t.to_lowercase().contains("dark"))
        .unwrap_or(true)
}

#[cfg(target_os = "macos")]
fn is_system_dark_mode() -> bool {
    std::process::Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("Dark"))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_system_dark_mode() -> bool {
    true
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn is_system_dark_mode() -> bool {
    true
}

// ============ TESTS ============

#[cfg(test)]
mod tests {
    use super::*;

    /// Environment variable name for test proxy URL
    const TEST_PROXY_ENV: &str = "TEST_PROXY_URL";

    /// Parsed proxy configuration from URL
    #[derive(Debug)]
    struct TestProxyConfig {
        host: String,
        port: u16,
        username: String,
        password: String,
    }

    /// Parse proxy URL in format: http://<user>:<pass>@<ip>:<port>
    fn parse_proxy_url(url: &str) -> Option<TestProxyConfig> {
        let url = url.strip_prefix("http://").or_else(|| url.strip_prefix("https://"))?;

        // Split by @ to get credentials and host:port
        let (credentials, host_port) = url.rsplit_once('@')?;
        let (username, password) = credentials.split_once(':')?;
        let (host, port_str) = host_port.rsplit_once(':')?;
        let port = port_str.parse().ok()?;

        Some(TestProxyConfig {
            host: host.to_string(),
            port,
            username: username.to_string(),
            password: password.to_string(),
        })
    }

    /// Get test proxy config from environment variable.
    /// Returns None and logs a message if not set.
    fn get_test_proxy() -> Option<TestProxyConfig> {
        match std::env::var(TEST_PROXY_ENV) {
            Ok(url) => {
                match parse_proxy_url(&url) {
                    Some(config) => Some(config),
                    None => {
                        eprintln!("WARNING: {} is set but has invalid format. Expected: http://<user>:<pass>@<ip>:<port>", TEST_PROXY_ENV);
                        None
                    }
                }
            }
            Err(_) => {
                eprintln!("INFO: Test skipped - {} environment variable not set", TEST_PROXY_ENV);
                None
            }
        }
    }

    #[test]
    fn test_saved_proxy_new() {
        let proxy = SavedProxy::new("Test".to_string(), "proxy.example.com".to_string(), 8080);
        assert_eq!(proxy.name, "Test");
        assert_eq!(proxy.host, "proxy.example.com");
        assert_eq!(proxy.port, 8080);
        assert!(proxy.username.is_none());
        assert!(proxy.password.is_none());
        assert_eq!(proxy.tun_name, "tun0");
        assert_eq!(proxy.tun_cidr, "10.255.255.1/30");
        assert!(!proxy.id.is_empty());
    }

    #[test]
    fn test_proxy_form_state_new() {
        let form = ProxyFormState::new();
        assert!(form.editing_id.is_none());
        assert!(form.name.is_empty());
        assert!(form.host.is_empty());
        assert_eq!(form.port, "8080");
        assert!(form.username.is_empty());
        assert!(form.password.is_empty());
        assert_eq!(form.tun_name, "tun0");
        assert_eq!(form.tun_cidr, "10.255.255.1/30");
    }

    #[test]
    fn test_proxy_form_state_from_proxy() {
        let mut proxy = SavedProxy::new("Test".to_string(), "proxy.example.com".to_string(), 3128);
        proxy.username = Some("user".to_string());
        proxy.password = Some("pass".to_string());

        let form = ProxyFormState::from_proxy(&proxy);
        assert_eq!(form.editing_id, Some(proxy.id.clone()));
        assert_eq!(form.name, "Test");
        assert_eq!(form.host, "proxy.example.com");
        assert_eq!(form.port, "3128");
        assert_eq!(form.username, "user");
        assert_eq!(form.password, "pass");
    }

    #[test]
    fn test_app_settings_default() {
        let settings = AppSettings::default();
        assert!(!settings.minimize_to_tray);
        assert!(!settings.start_minimized);
        assert!(!settings.killswitch_enabled);
        assert_eq!(settings.theme_mode, ThemeMode::Dark);
    }

    #[test]
    fn test_theme_mode_default() {
        let mode = ThemeMode::default();
        assert_eq!(mode, ThemeMode::Dark);
    }

    #[test]
    fn test_connection_status_default() {
        let status = ConnectionStatus::default();
        assert_eq!(status, ConnectionStatus::Disconnected);
    }

    #[test]
    fn test_view_default() {
        let view = View::default();
        assert_eq!(view, View::Main);
    }

    #[test]
    fn test_log_level_as_str() {
        assert_eq!(LogLevel::Info.as_str(), "INFO");
        assert_eq!(LogLevel::Error.as_str(), "ERROR");
    }

    #[test]
    fn test_current_timestamp_format() {
        let ts = current_timestamp();
        // Should be HH:MM:SS format
        assert_eq!(ts.len(), 8);
        assert_eq!(&ts[2..3], ":");
        assert_eq!(&ts[5..6], ":");
    }

    #[test]
    fn test_proxy_serialization() {
        let proxy = SavedProxy::new("Test".to_string(), "host.com".to_string(), 8080);
        let json = serde_json::to_string(&proxy).unwrap();

        // Password should be skipped in serialization
        assert!(!json.contains("password"));

        // Other fields should be present
        assert!(json.contains("Test"));
        assert!(json.contains("host.com"));
        assert!(json.contains("8080"));
    }

    #[test]
    fn test_proxy_deserialization() {
        let json = r#"{
            "id": "test-id",
            "name": "Test Proxy",
            "host": "proxy.example.com",
            "port": 3128,
            "username": "user",
            "tun_name": "tun1",
            "tun_cidr": "10.0.0.1/24"
        }"#;

        let proxy: SavedProxy = serde_json::from_str(json).unwrap();
        assert_eq!(proxy.id, "test-id");
        assert_eq!(proxy.name, "Test Proxy");
        assert_eq!(proxy.host, "proxy.example.com");
        assert_eq!(proxy.port, 3128);
        assert_eq!(proxy.username, Some("user".to_string()));
        assert!(proxy.password.is_none()); // Skipped in deserialization
        assert_eq!(proxy.tun_name, "tun1");
        assert_eq!(proxy.tun_cidr, "10.0.0.1/24");
    }

    #[test]
    fn test_keyring_save_load_cycle() {
        // Create a proxy with a unique test ID to avoid conflicts
        let test_id = format!("test-keyring-{}", uuid::Uuid::new_v4());
        let mut proxy = SavedProxy {
            id: test_id.clone(),
            name: "Keyring Test".to_string(),
            host: "test.example.com".to_string(),
            port: 8080,
            username: Some("testuser".to_string()),
            password: Some("testsecretpassword".to_string()),
            tun_name: "tun0".to_string(),
            tun_cidr: "10.255.255.1/30".to_string(),
        };

        // Save password to keyring
        proxy.save_password();

        // Clear the password from struct
        proxy.password = None;
        assert!(proxy.password.is_none());

        // Load password from keyring
        proxy.load_password();

        // Verify password was loaded
        assert_eq!(proxy.password, Some("testsecretpassword".to_string()));

        // Clean up: delete from keyring
        proxy.delete_password();

        // Verify deletion
        proxy.password = None;
        proxy.load_password();
        assert!(proxy.password.is_none());
    }

    #[test]
    fn test_proxy_password_not_serialized() {
        let mut proxy = SavedProxy::new("Test".to_string(), "host.com".to_string(), 8080);
        proxy.password = Some("secret123".to_string());

        // Serialize
        let json = serde_json::to_string(&proxy).unwrap();

        // Password should NOT be in JSON
        assert!(!json.contains("secret123"));
        assert!(!json.contains("password"));

        // Deserialize
        let loaded: SavedProxy = serde_json::from_str(&json).unwrap();

        // Password should be None after deserialization
        assert!(loaded.password.is_none());
    }

    #[test]
    fn test_proxy_roundtrip_with_keyring() {
        // Skip test if proxy URL not set
        let Some(proxy_config) = get_test_proxy() else {
            return;
        };

        // Simulate saving and loading proxies like the app does
        let test_id = format!("test-roundtrip-{}", uuid::Uuid::new_v4());

        // Create proxy with password from environment
        let original = SavedProxy {
            id: test_id.clone(),
            name: "Roundtrip Test".to_string(),
            host: proxy_config.host,
            port: proxy_config.port,
            username: Some(proxy_config.username),
            password: Some(proxy_config.password.clone()),
            tun_name: "tun0".to_string(),
            tun_cidr: "10.255.255.1/30".to_string(),
        };

        // Save password to keyring (as app does when saving proxy)
        original.save_password();

        // Serialize to JSON (password is skipped)
        let json = serde_json::to_string(&original).unwrap();

        // Deserialize (simulating app restart)
        let mut loaded: SavedProxy = serde_json::from_str(&json).unwrap();

        // Password should be None after JSON load
        assert!(loaded.password.is_none());

        // Load password from keyring (as app does on startup)
        loaded.load_password();

        // Password should now be restored
        assert_eq!(loaded.password, Some(proxy_config.password));

        // Cleanup
        loaded.delete_password();
    }

    #[test]
    fn test_connect_args_has_password() {
        // Skip test if proxy URL not set
        let Some(proxy_config) = get_test_proxy() else {
            return;
        };

        use crate::bridge::ConnectArgs;

        let args = ConnectArgs {
            proxy_host: proxy_config.host,
            proxy_port: proxy_config.port,
            username: Some(proxy_config.username),
            password: Some(proxy_config.password.clone()),
            tun_name: "tun0".to_string(),
            tun_cidr: "10.255.255.1/30".to_string(),
            killswitch: false,
        };

        assert!(args.password.is_some());
        assert_eq!(args.password.unwrap(), proxy_config.password);
    }

    #[test]
    fn test_parse_proxy_url() {
        // Test valid URL
        let config = parse_proxy_url("http://user:pass@192.168.1.1:8080").unwrap();
        assert_eq!(config.host, "192.168.1.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.username, "user");
        assert_eq!(config.password, "pass");

        // Test with https
        let config = parse_proxy_url("https://admin:secret@proxy.example.com:3128").unwrap();
        assert_eq!(config.host, "proxy.example.com");
        assert_eq!(config.port, 3128);
        assert_eq!(config.username, "admin");
        assert_eq!(config.password, "secret");

        // Test invalid URLs
        assert!(parse_proxy_url("not-a-url").is_none());
        assert!(parse_proxy_url("http://user@host:8080").is_none()); // missing password
        assert!(parse_proxy_url("http://user:pass@host").is_none()); // missing port
    }
}
