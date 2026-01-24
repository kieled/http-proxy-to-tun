use crate::connection::ConnectionManager;
use crate::proxy_store::ProxyStore;
use crate::settings::SettingsStore;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct AppState {
    pub connection: Arc<RwLock<ConnectionManager>>,
    pub proxies: Arc<RwLock<ProxyStore>>,
    pub settings: Arc<RwLock<SettingsStore>>,
}

impl AppState {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            connection: Arc::new(RwLock::new(ConnectionManager::new())),
            proxies: Arc::new(RwLock::new(ProxyStore::new()?)),
            settings: Arc::new(RwLock::new(SettingsStore::new()?)),
        })
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connection: Arc::new(RwLock::new(ConnectionManager::new())),
            proxies: Arc::new(RwLock::new(ProxyStore::default())),
            settings: Arc::new(RwLock::new(SettingsStore::default())),
        }
    }
}
