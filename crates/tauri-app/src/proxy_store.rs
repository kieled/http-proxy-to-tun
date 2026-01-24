use crate::error::{AppError, Result};
use keyring::Entry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

const SERVICE_NAME: &str = "http-tun-desktop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedProxy {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ProxyData {
    proxies: Vec<SavedProxy>,
}

pub struct ProxyStore {
    config_path: PathBuf,
    proxies: Vec<SavedProxy>,
}

impl ProxyStore {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| AppError::internal("Could not find config directory"))?
            .join("http-tun");

        fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("proxies.json");
        let proxies = if config_path.exists() {
            let data: ProxyData = serde_json::from_str(&fs::read_to_string(&config_path)?)?;
            data.proxies
        } else {
            Vec::new()
        };

        Ok(Self {
            config_path,
            proxies,
        })
    }

    fn save(&self) -> Result<()> {
        let data = ProxyData {
            proxies: self.proxies.clone(),
        };
        let json = serde_json::to_string_pretty(&data)?;
        fs::write(&self.config_path, json)?;
        Ok(())
    }

    fn keyring_entry(&self, proxy_id: &str) -> std::result::Result<Entry, keyring::Error> {
        Entry::new(SERVICE_NAME, proxy_id)
    }

    pub fn list(&self) -> Vec<SavedProxy> {
        let mut proxies = self.proxies.clone();
        proxies.sort_by_key(|p| p.order);
        proxies
    }

    pub fn get(&self, id: &str) -> Option<SavedProxy> {
        self.proxies.iter().find(|p| p.id == id).cloned()
    }

    pub fn get_password(&self, proxy_id: &str) -> Result<Option<String>> {
        let entry = self.keyring_entry(proxy_id)?;
        match entry.get_password() {
            Ok(pwd) => Ok(Some(pwd)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn add(
        &mut self,
        name: String,
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<SavedProxy> {
        // Check for duplicates
        if self.is_duplicate(&host, port, username.as_deref(), None) {
            return Err(AppError::duplicate(format!(
                "A proxy with host {}:{} already exists",
                host, port
            )));
        }

        let id = Uuid::new_v4().to_string();
        let order = self.proxies.len() as i32;

        let proxy = SavedProxy {
            id: id.clone(),
            name,
            host,
            port,
            username,
            order,
        };

        self.proxies.push(proxy.clone());
        self.save()?;

        // Store password in keyring if provided
        if let Some(pwd) = password {
            let entry = self.keyring_entry(&id)?;
            entry.set_password(&pwd)?;
        }

        Ok(proxy)
    }

    pub fn update(
        &mut self,
        id: &str,
        name: Option<String>,
        host: Option<String>,
        port: Option<u16>,
        username: Option<Option<String>>,
        password: Option<Option<String>>,
    ) -> Result<SavedProxy> {
        // First, find the proxy and get values we need for duplicate check
        let idx = self
            .proxies
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| AppError::not_found("Proxy"))?;

        let proxy = &self.proxies[idx];

        // Check for duplicates if host/port/username changed
        let new_host = host.as_ref().unwrap_or(&proxy.host).clone();
        let new_port = port.unwrap_or(proxy.port);
        let new_username = match &username {
            Some(u) => u.clone(),
            None => proxy.username.clone(),
        };

        if self.is_duplicate(&new_host, new_port, new_username.as_deref(), Some(id)) {
            return Err(AppError::duplicate(format!(
                "A proxy with host {}:{} already exists",
                new_host, new_port
            )));
        }

        // Now mutate the proxy
        let proxy = &mut self.proxies[idx];
        if let Some(n) = name {
            proxy.name = n;
        }
        if let Some(h) = host {
            proxy.host = h;
        }
        if let Some(p) = port {
            proxy.port = p;
        }
        if let Some(u) = username {
            proxy.username = u;
        }

        let updated = proxy.clone();
        self.save()?;

        // Update password in keyring
        if let Some(pwd_opt) = password {
            let entry = self.keyring_entry(id)?;
            match pwd_opt {
                Some(pwd) => entry.set_password(&pwd)?,
                None => {
                    let _ = entry.delete_credential();
                }
            }
        }

        Ok(updated)
    }

    pub fn delete(&mut self, id: &str) -> Result<()> {
        let idx = self
            .proxies
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| AppError::not_found("Proxy"))?;

        self.proxies.remove(idx);
        self.save()?;

        // Remove password from keyring
        if let Ok(entry) = self.keyring_entry(id) {
            let _ = entry.delete_credential();
        }

        Ok(())
    }

    pub fn reorder(&mut self, ids: Vec<String>) -> Result<()> {
        for (order, id) in ids.iter().enumerate() {
            if let Some(proxy) = self.proxies.iter_mut().find(|p| &p.id == id) {
                proxy.order = order as i32;
            }
        }
        self.save()?;
        Ok(())
    }

    fn is_duplicate(&self, host: &str, port: u16, username: Option<&str>, exclude_id: Option<&str>) -> bool {
        self.proxies.iter().any(|p| {
            let same_connection = p.host == host && p.port == port && p.username.as_deref() == username;
            let is_different_proxy = exclude_id.is_none_or(|id| p.id != id);
            same_connection && is_different_proxy
        })
    }
}

impl Default for ProxyStore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            config_path: PathBuf::new(),
            proxies: Vec::new(),
        })
    }
}
