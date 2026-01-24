use crate::connection::ConnectionState;
use crate::proxy_store::SavedProxy;
use serde::Serialize;

pub const EVENT_CONNECTION_STATUS: &str = "connection-status";
pub const EVENT_CONNECTION_TIME: &str = "connection-time";
pub const EVENT_PROXIES_CHANGED: &str = "proxies-changed";

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionStatusEvent {
    pub state: ConnectionState,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectionTimeEvent {
    pub duration_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProxiesChangedEvent {
    pub proxies: Vec<SavedProxy>,
}
