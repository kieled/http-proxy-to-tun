use crate::connection::{check_privileges, has_pkexec, ConnectionState, ConnectionStatus};
use crate::error::{AppError, Result};
use crate::events::{
    ConnectionStatusEvent, ConnectionTimeEvent, ProxiesChangedEvent, EVENT_CONNECTION_STATUS,
    EVENT_CONNECTION_TIME, EVENT_PROXIES_CHANGED,
};
use crate::proxy_store::SavedProxy;
use crate::settings::{AppSettings, Theme};
use crate::state::AppState;
use crate::tray::update_tray_tooltip;
use crate::vpn::{run_vpn, VpnParams};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::oneshot;

// ============================================================================
// Proxy Commands
// ============================================================================

#[tauri::command]
pub async fn get_proxies(state: State<'_, AppState>) -> Result<Vec<SavedProxy>> {
    let store = state.proxies.read().await;
    Ok(store.list())
}

#[tauri::command]
pub async fn get_proxy(state: State<'_, AppState>, id: String) -> Result<Option<SavedProxy>> {
    let store = state.proxies.read().await;
    Ok(store.get(&id))
}

#[tauri::command]
pub async fn add_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    host: String,
    port: u16,
    username: Option<String>,
    password: Option<String>,
) -> Result<SavedProxy> {
    let mut store = state.proxies.write().await;
    let proxy = store.add(name, host, port, username, password)?;

    // Emit proxies changed event
    let proxies = store.list();
    let _ = app.emit(EVENT_PROXIES_CHANGED, ProxiesChangedEvent { proxies });

    Ok(proxy)
}

#[tauri::command]
pub async fn update_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    host: Option<String>,
    port: Option<u16>,
    username: Option<Option<String>>,
    password: Option<Option<String>>,
) -> Result<SavedProxy> {
    let mut store = state.proxies.write().await;
    let proxy = store.update(&id, name, host, port, username, password)?;

    // Emit proxies changed event
    let proxies = store.list();
    let _ = app.emit(EVENT_PROXIES_CHANGED, ProxiesChangedEvent { proxies });

    Ok(proxy)
}

#[tauri::command]
pub async fn delete_proxy(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<()> {
    // Check if this proxy is currently connected
    {
        let conn = state.connection.read().await;
        if conn.current_proxy_id() == Some(id.clone()) && conn.is_connected() {
            return Err(AppError::validation(
                "Cannot delete a proxy that is currently connected",
            ));
        }
    }

    let mut store = state.proxies.write().await;
    store.delete(&id)?;

    // Emit proxies changed event
    let proxies = store.list();
    let _ = app.emit(EVENT_PROXIES_CHANGED, ProxiesChangedEvent { proxies });

    Ok(())
}

#[tauri::command]
pub async fn reorder_proxies(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<()> {
    let mut store = state.proxies.write().await;
    store.reorder(ids)?;

    // Emit proxies changed event
    let proxies = store.list();
    let _ = app.emit(EVENT_PROXIES_CHANGED, ProxiesChangedEvent { proxies });

    Ok(())
}

// ============================================================================
// Connection Commands
// ============================================================================

#[tauri::command]
pub async fn get_connection_status(state: State<'_, AppState>) -> Result<ConnectionState> {
    let conn = state.connection.read().await;
    Ok(conn.state())
}

#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    proxy_id: String,
) -> Result<()> {
    // Check privileges first
    check_privileges()?;

    // Get proxy details
    let (proxy, password) = {
        let store = state.proxies.read().await;
        let proxy = store
            .get(&proxy_id)
            .ok_or_else(|| AppError::not_found("Proxy"))?;
        let password = store.get_password(&proxy_id)?;
        (proxy, password)
    };

    // Check if already connected
    {
        let conn = state.connection.read().await;
        if conn.status() == ConnectionStatus::Connecting
            || conn.status() == ConnectionStatus::Connected
        {
            return Err(AppError::validation(
                "Already connected or connecting. Disconnect first.",
            ));
        }
    }

    // Update state to connecting
    {
        let mut conn = state.connection.write().await;
        conn.set_connecting(&proxy);
    }

    // Emit status update
    emit_status(&app, &state).await;

    // Create shutdown channel
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    // Store shutdown handle
    {
        let mut conn = state.connection.write().await;
        conn.set_shutdown_handle(shutdown_tx);
    }

    // Save last proxy
    {
        let mut settings = state.settings.write().await;
        let _ = settings.set_last_proxy(Some(proxy_id.clone()));
    }

    // Get connection manager handles
    let is_running = {
        let conn = state.connection.read().await;
        conn.is_running()
    };

    // Spawn connection task
    let app_handle = app.clone();
    let state_clone = app.state::<AppState>().inner().clone();
    let proxy_clone = proxy.clone();

    tokio::spawn(async move {
        let result = run_connection(
            &proxy_clone,
            password.as_deref(),
            &state_clone,
            shutdown_rx,
        )
        .await;

        match result {
            Ok(()) => {
                let mut conn = state_clone.connection.write().await;
                conn.set_disconnected();
            }
            Err(e) => {
                let mut conn = state_clone.connection.write().await;
                conn.set_error(e.message);
            }
        }

        is_running.store(false, Ordering::SeqCst);
        emit_status_with_state(&app_handle, &state_clone).await;
    });

    // Start timer task
    start_timer_task(app.clone(), state.inner().clone()).await;

    // Mark as connected (the actual connection happens in the spawned task)
    {
        let mut conn = state.connection.write().await;
        conn.set_connected();
    }

    emit_status(&app, &state).await;

    // Fetch public IP in background and emit updated status
    let app_for_ip = app.clone();
    let state_for_ip = app.state::<AppState>().inner().clone();
    tokio::spawn(async move {
        let ip = fetch_public_ip().await;
        {
            let mut conn = state_for_ip.connection.write().await;
            conn.set_public_ip(ip);
        }
        emit_status_with_state(&app_for_ip, &state_for_ip).await;
    });

    Ok(())
}

#[tauri::command]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    // Get shutdown handle
    let shutdown_tx = {
        let mut conn = state.connection.write().await;
        if conn.status() != ConnectionStatus::Connected
            && conn.status() != ConnectionStatus::Connecting
        {
            return Err(AppError::validation("Not connected"));
        }
        conn.set_disconnecting();
        conn.take_shutdown_handle()
    };

    emit_status(&app, &state).await;

    // Send shutdown signal
    if let Some(tx) = shutdown_tx {
        let _ = tx.send(());
    }

    // Stop timer
    {
        let conn = state.connection.read().await;
        let timer = conn.timer_shutdown();
        let mut guard = timer.lock().await;
        if let Some(tx) = guard.take() {
            let _ = tx.send(());
        }
    }

    // Wait a bit for cleanup
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Update state
    {
        let mut conn = state.connection.write().await;
        conn.set_disconnected();
    }

    emit_status(&app, &state).await;

    // Fetch public IP in background and emit updated status
    let app_for_ip = app.clone();
    let state_for_ip = app.state::<AppState>().inner().clone();
    tokio::spawn(async move {
        let ip = fetch_public_ip().await;
        {
            let mut conn = state_for_ip.connection.write().await;
            conn.set_public_ip(ip);
        }
        emit_status_with_state(&app_for_ip, &state_for_ip).await;
    });

    Ok(())
}

// ============================================================================
// Settings Commands
// ============================================================================

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings> {
    let settings = state.settings.read().await;
    Ok(settings.get())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings> {
    let mut store = state.settings.write().await;
    store.update(settings)
}

#[tauri::command]
pub async fn set_theme(state: State<'_, AppState>, theme: Theme) -> Result<()> {
    let mut settings = state.settings.write().await;
    settings.set_theme(theme)
}

// ============================================================================
// Privilege Check Commands
// ============================================================================

#[tauri::command]
pub fn check_privileges_command() -> Result<bool> {
    check_privileges()
}

#[tauri::command]
pub fn can_elevate_privileges() -> bool {
    has_pkexec()
}

#[tauri::command]
pub async fn set_capabilities() -> Result<()> {
    use std::process::Command;

    // Get the path to the current executable
    let exe_path = std::env::current_exe()
        .map_err(|e| AppError::internal(format!("Failed to get executable path: {}", e)))?;

    // Use pkexec to run setcap
    let output = Command::new("pkexec")
        .args(["setcap", "cap_net_admin+ep"])
        .arg(&exe_path)
        .output()
        .map_err(|e| AppError::internal(format!("Failed to run pkexec: {}", e)))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(AppError::permission(format!("Failed to set capabilities: {}", stderr)))
    }
}

// ============================================================================
// Network Info Commands
// ============================================================================

#[tauri::command]
pub async fn refresh_public_ip(app: AppHandle, state: State<'_, AppState>) -> Result<Option<String>> {
    let ip = fetch_public_ip().await;

    // Update state
    {
        let mut conn = state.connection.write().await;
        conn.set_public_ip(ip.clone());
    }

    // Emit updated status
    emit_status(&app, &state).await;

    Ok(ip)
}

async fn fetch_public_ip() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;

    let response = client.get("https://ifconfig.me/ip").send().await.ok()?;

    if !response.status().is_success() {
        return None;
    }

    let text = response.text().await.ok()?;
    Some(text.trim().to_string())
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn emit_status(app: &AppHandle, state: &State<'_, AppState>) {
    let conn = state.connection.read().await;
    let conn_state = conn.state();
    let _ = app.emit(
        EVENT_CONNECTION_STATUS,
        ConnectionStatusEvent {
            state: conn_state.clone(),
        },
    );
    // Update tray tooltip
    let _ = update_tray_tooltip(app, conn.status(), conn_state.proxy_name.as_deref());
}

async fn emit_status_with_state(app: &AppHandle, state: &AppState) {
    let conn = state.connection.read().await;
    let conn_state = conn.state();
    let _ = app.emit(
        EVENT_CONNECTION_STATUS,
        ConnectionStatusEvent {
            state: conn_state.clone(),
        },
    );
    // Update tray tooltip
    let _ = update_tray_tooltip(app, conn.status(), conn_state.proxy_name.as_deref());
}

async fn start_timer_task(app: AppHandle, state: AppState) {
    let (timer_tx, mut timer_rx) = oneshot::channel();

    // Store timer shutdown
    {
        let conn = state.connection.read().await;
        let timer = conn.timer_shutdown();
        let mut guard = timer.lock().await;
        *guard = Some(timer_tx);
    }

    let is_running = {
        let conn = state.connection.read().await;
        conn.is_running()
    };

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !is_running.load(Ordering::SeqCst) {
                        break;
                    }
                    let conn = state.connection.read().await;
                    let duration = conn.state().duration_secs;
                    let _ = app.emit(EVENT_CONNECTION_TIME, ConnectionTimeEvent { duration_secs: duration });
                }
                _ = &mut timer_rx => {
                    break;
                }
            }
        }
    });
}

async fn run_connection(
    proxy: &SavedProxy,
    password: Option<&str>,
    state: &AppState,
    shutdown_rx: oneshot::Receiver<()>,
) -> Result<()> {
    // Get settings for killswitch
    let settings = {
        let s = state.settings.read().await;
        s.get()
    };

    // Build VPN parameters
    let username = proxy.username.clone().unwrap_or_default();
    let password = password.unwrap_or_default().to_string();

    let params = VpnParams {
        host: proxy.host.clone(),
        port: proxy.port,
        username,
        password,
        killswitch: settings.killswitch,
    };

    // Run the VPN connection
    run_vpn(params, shutdown_rx)
        .await
        .map_err(|e| AppError::internal(format!("VPN error: {}", e)))?;

    Ok(())
}

// Clone implementation for AppState to use in spawned tasks
impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            connection: self.connection.clone(),
            proxies: self.proxies.clone(),
            settings: self.settings.clone(),
        }
    }
}
