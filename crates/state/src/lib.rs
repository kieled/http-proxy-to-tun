use std::fs;
use std::io::{Read as _, Write as _};
use std::net::IpAddr;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use proxyvpn_util::{set_permissions_0600, set_permissions_0700};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteBypassRule {
    pub pref: u32,
    pub ip: IpAddr,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "backend", rename_all = "snake_case")]
pub enum FirewallState {
    Nft { table: String, chain: String },
    Iptables { chain: String },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct State {
    pub version: u32,
    pub created_at: String,
    pub state_dir: PathBuf,
    pub lock_path: PathBuf,
    pub tun_name: String,
    pub tun_cidr: String,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_ips: Vec<IpAddr>,
    pub dns: Option<IpAddr>,
    pub killswitch: bool,
    pub keep_logs: bool,
    pub proxy_table: u32,
    #[serde(default)]
    pub dns_bypass_rules: Vec<RouteBypassRule>,
    #[serde(default)]
    pub proxy_bypass_rules: Vec<RouteBypassRule>,
    pub tcp_rule_pref: Option<u32>,
    pub firewall: Option<FirewallState>,
}

#[derive(Debug, Clone)]
pub struct NewStateParams {
    pub state_dir: PathBuf,
    pub lock_path: PathBuf,
    pub tun_name: String,
    pub tun_cidr: String,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_ips: Vec<IpAddr>,
    pub dns: Option<IpAddr>,
    pub killswitch: bool,
    pub keep_logs: bool,
    pub proxy_table: u32,
}

#[derive(Clone)]
pub struct StateStore {
    pub state_dir: PathBuf,
    pub state_path: PathBuf,
    pub lock_path: PathBuf,
}

impl StateStore {
    pub fn new(state_dir: PathBuf) -> Self {
        let state_path = state_dir.join("state.json");
        let lock_path = state_dir.join("lock");
        Self {
            state_dir,
            state_path,
            lock_path,
        }
    }

    pub fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.state_dir)?;
        set_permissions_0700(&self.state_dir)?;
        Ok(())
    }

    pub fn create_lock(&self) -> Result<fs::File> {
        // Try to clean up stale lock first
        if self.lock_path.exists() && self.is_lock_stale()? {
            eprintln!("removing stale lock file from crashed instance");
            let _ = fs::remove_file(&self.lock_path);
        }

        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.lock_path)
            .with_context(|| "lock file exists (is proxyvpn already running?)")?;
        set_permissions_0600(&self.lock_path)?;

        // Write our PID to the lock file
        let pid = std::process::id();
        writeln!(file, "{}", pid).context("failed to write pid to lock")?;
        file.sync_all()?;
        Ok(file)
    }

    /// Check if the lock file is stale (process that created it no longer exists)
    pub fn is_lock_stale(&self) -> Result<bool> {
        let mut contents = String::new();
        let mut file = match fs::File::open(&self.lock_path) {
            Ok(f) => f,
            Err(_) => return Ok(true), // Can't open = stale
        };
        if file.read_to_string(&mut contents).is_err() {
            return Ok(true); // Can't read = stale
        }

        let pid: u32 = match contents.trim().parse() {
            Ok(p) => p,
            Err(_) => return Ok(true), // Can't parse = old format, assume stale
        };

        // Check if process is still running by checking /proc/<pid>
        let proc_path = format!("/proc/{}", pid);
        Ok(!std::path::Path::new(&proc_path).exists())
    }

    /// Force remove lock file (for recovery after crash)
    pub fn force_remove_lock(&self) -> Result<()> {
        if self.lock_path.exists() {
            fs::remove_file(&self.lock_path)?;
        }
        Ok(())
    }

    pub fn write_state(&self, state: &State) -> Result<()> {
        let data = serde_json::to_vec_pretty(state)?;
        fs::write(&self.state_path, data)?;
        set_permissions_0600(&self.state_path)?;
        Ok(())
    }

    pub fn read_state(&self) -> Result<State> {
        let data = fs::read(&self.state_path)
            .with_context(|| format!("state file not found: {}", self.state_path.display()))?;
        Ok(serde_json::from_slice(&data)?)
    }

    pub fn remove_state_files(&self, keep_logs: bool) -> Result<()> {
        let _ = fs::remove_file(&self.lock_path);
        if keep_logs {
            return Ok(());
        }
        let _ = fs::remove_file(&self.state_path);
        let _ = fs::remove_dir(&self.state_dir);
        Ok(())
    }
}

pub fn new_state_template(params: NewStateParams) -> State {
    let now = OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    State {
        version: 3,
        created_at: now,
        state_dir: params.state_dir.clone(),
        lock_path: params.lock_path.clone(),
        tun_name: params.tun_name,
        tun_cidr: params.tun_cidr,
        proxy_host: params.proxy_host,
        proxy_port: params.proxy_port,
        proxy_ips: params.proxy_ips,
        dns: params.dns,
        killswitch: params.killswitch,
        keep_logs: params.keep_logs,
        proxy_table: params.proxy_table,
        dns_bypass_rules: Vec::new(),
        proxy_bypass_rules: Vec::new(),
        tcp_rule_pref: None,
        firewall: None,
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(prefix: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("{prefix}-{now}"))
    }

    #[test]
    fn state_roundtrip() {
        let state_dir = temp_path("proxyvpn-state");
        let store = StateStore::new(state_dir.clone());
        store.ensure_dir().unwrap();
        let state = new_state_template(NewStateParams {
            state_dir: store.state_dir.clone(),
            lock_path: store.lock_path.clone(),
            tun_name: "tun0".to_string(),
            tun_cidr: "172.19.0.1/30".to_string(),
            proxy_host: "proxy.example.com".to_string(),
            proxy_port: 8080,
            proxy_ips: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))],
            dns: None,
            killswitch: true,
            keep_logs: false,
            proxy_table: 100,
        });
        store.write_state(&state).unwrap();
        let loaded = store.read_state().unwrap();
        assert_eq!(loaded.tun_name, "tun0");
        assert_eq!(loaded.proxy_port, 8080);
        assert_eq!(loaded.proxy_ips.len(), 1);
        let _ = store.remove_state_files(true);
    }

    #[test]
    fn keep_logs_preserves_state_file() {
        let state_dir = temp_path("proxyvpn-state-keep");
        let store = StateStore::new(state_dir.clone());
        store.ensure_dir().unwrap();
        let state = new_state_template(NewStateParams {
            state_dir: store.state_dir.clone(),
            lock_path: store.lock_path.clone(),
            tun_name: "tun0".to_string(),
            tun_cidr: "172.19.0.1/30".to_string(),
            proxy_host: "proxy.example.com".to_string(),
            proxy_port: 8080,
            proxy_ips: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))],
            dns: None,
            killswitch: true,
            keep_logs: true,
            proxy_table: 100,
        });
        store.write_state(&state).unwrap();
        let _ = store.create_lock().unwrap();
        store.remove_state_files(true).unwrap();
        assert!(store.state_path.exists());
        assert!(store.state_dir.exists());
        let _ = store.remove_state_files(false);
    }
}
