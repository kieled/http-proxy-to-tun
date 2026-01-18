use std::fs;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::firewall::FirewallState;
use crate::util::{set_permissions_0600, set_permissions_0700};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SysctlState {
    pub ipv4_forward: Option<String>,
    pub ipv6_forward: Option<String>,
    pub changed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SingBoxState {
    pub pid: Option<i32>,
    pub config_path: PathBuf,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RouteBypassRule {
    pub pref: u32,
    pub ip: IpAddr,
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
    #[serde(default)]
    pub dns_bypass_rules: Vec<RouteBypassRule>,
    #[serde(default)]
    pub proxy_bypass_rules: Vec<RouteBypassRule>,
    pub routes_before: Vec<String>,
    pub firewall: Option<FirewallState>,
    pub sysctl: SysctlState,
    pub sing_box: SingBoxState,
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
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.lock_path)
            .with_context(|| "lock file exists (is proxyvpn already running?)")?;
        set_permissions_0600(&self.lock_path)?;
        Ok(file)
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
        if !keep_logs {
            let _ = fs::remove_file(self.state_dir.join("sing-box.stdout.log"));
            let _ = fs::remove_file(self.state_dir.join("sing-box.stderr.log"));
        }
        let _ = fs::remove_file(self.state_dir.join("sing-box.json"));
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
        version: 1,
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
        dns_bypass_rules: Vec::new(),
        proxy_bypass_rules: Vec::new(),
        routes_before: Vec::new(),
        firewall: None,
        sysctl: SysctlState {
            ipv4_forward: None,
            ipv6_forward: None,
            changed: false,
        },
        sing_box: SingBoxState {
            pid: None,
            config_path: params.state_dir.join("sing-box.json"),
            stdout_path: params.state_dir.join("sing-box.stdout.log"),
            stderr_path: params.state_dir.join("sing-box.stderr.log"),
        },
    }
}

pub fn write_text_file_0600(path: &Path, contents: &str) -> Result<()> {
    fs::write(path, contents)?;
    set_permissions_0600(path)?;
    Ok(())
}

pub fn open_log_file_0600(path: &Path) -> Result<fs::File> {
    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    set_permissions_0600(path)?;
    Ok(file)
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
        });
        store.write_state(&state).unwrap();
        let loaded = store.read_state().unwrap();
        assert_eq!(loaded.tun_name, "tun0");
        assert_eq!(loaded.proxy_port, 8080);
        assert_eq!(loaded.proxy_ips.len(), 1);
        let _ = store.remove_state_files(true);
    }
}
