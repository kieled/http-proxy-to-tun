use crate::error::Result;
use crate::proxy_store::SavedProxy;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{oneshot, Mutex};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    Disconnecting,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionState {
    pub status: ConnectionStatus,
    pub proxy_id: Option<String>,
    pub proxy_name: Option<String>,
    pub connected_since: Option<i64>,
    pub duration_secs: u64,
    pub error_message: Option<String>,
    pub public_ip: Option<String>,
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            proxy_id: None,
            proxy_name: None,
            connected_since: None,
            duration_secs: 0,
            error_message: None,
            public_ip: None,
        }
    }
}

struct ConnectionHandle {
    shutdown_tx: oneshot::Sender<()>,
}

pub struct ConnectionManager {
    state: ConnectionState,
    handle: Option<ConnectionHandle>,
    connect_time: Option<Instant>,
    is_running: Arc<AtomicBool>,
    timer_shutdown: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::default(),
            handle: None,
            connect_time: None,
            is_running: Arc::new(AtomicBool::new(false)),
            timer_shutdown: Arc::new(Mutex::new(None)),
        }
    }

    pub fn state(&self) -> ConnectionState {
        let mut state = self.state.clone();
        if let Some(connect_time) = self.connect_time {
            state.duration_secs = connect_time.elapsed().as_secs();
        }
        state
    }

    pub fn status(&self) -> ConnectionStatus {
        self.state.status
    }

    pub fn is_connected(&self) -> bool {
        self.state.status == ConnectionStatus::Connected
    }

    pub fn set_connecting(&mut self, proxy: &SavedProxy) {
        self.state.status = ConnectionStatus::Connecting;
        self.state.proxy_id = Some(proxy.id.clone());
        self.state.proxy_name = Some(proxy.name.clone());
        self.state.error_message = None;
    }

    pub fn set_connected(&mut self) {
        use std::time::{SystemTime, UNIX_EPOCH};
        self.state.status = ConnectionStatus::Connected;
        self.state.connected_since = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
        );
        self.connect_time = Some(Instant::now());
        self.is_running.store(true, Ordering::SeqCst);
    }

    pub fn set_disconnecting(&mut self) {
        self.state.status = ConnectionStatus::Disconnecting;
    }

    pub fn set_disconnected(&mut self) {
        self.state.status = ConnectionStatus::Disconnected;
        self.state.proxy_id = None;
        self.state.proxy_name = None;
        self.state.connected_since = None;
        self.state.duration_secs = 0;
        self.connect_time = None;
        self.is_running.store(false, Ordering::SeqCst);
        self.handle = None;
    }

    pub fn set_error(&mut self, message: String) {
        self.state.status = ConnectionStatus::Error;
        self.state.error_message = Some(message);
        self.is_running.store(false, Ordering::SeqCst);
        self.handle = None;
    }

    pub fn set_public_ip(&mut self, ip: Option<String>) {
        self.state.public_ip = ip;
    }

    pub fn set_shutdown_handle(&mut self, shutdown_tx: oneshot::Sender<()>) {
        self.handle = Some(ConnectionHandle { shutdown_tx });
    }

    pub fn take_shutdown_handle(&mut self) -> Option<oneshot::Sender<()>> {
        self.handle.take().map(|h| h.shutdown_tx)
    }

    pub fn is_running(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.is_running)
    }

    pub fn timer_shutdown(&self) -> Arc<Mutex<Option<oneshot::Sender<()>>>> {
        Arc::clone(&self.timer_shutdown)
    }

    pub fn current_proxy_id(&self) -> Option<String> {
        self.state.proxy_id.clone()
    }
}

impl Default for ConnectionManager {
    fn default() -> Self {
        Self::new()
    }
}

pub fn check_privileges() -> Result<bool> {
    // Check if we have CAP_NET_ADMIN capability
    let uid = unsafe { libc::getuid() };
    if uid == 0 {
        return Ok(true);
    }

    // Try to read capabilities from /proc/self/status
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("CapEff:") {
                if let Some(cap_hex) = line.split_whitespace().nth(1) {
                    if let Ok(caps) = u64::from_str_radix(cap_hex, 16) {
                        // CAP_NET_ADMIN is bit 12
                        if caps & (1 << 12) != 0 {
                            return Ok(true);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
}

pub fn has_pkexec() -> bool {
    std::process::Command::new("which")
        .arg("pkexec")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
