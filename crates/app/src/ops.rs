use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;

use proxyvpn_firewall::{FirewallBackendKind, FirewallConfig};
use proxyvpn_mark::{MarkBackendKind, MarkConfig, remove_mark_rules_best_effort};
use proxyvpn_netlink::Netlink;
use proxyvpn_state::{FirewallState, State, StateStore};
use proxyvpn_util::CommandRunner;

#[allow(async_fn_in_trait)]
pub trait NetlinkOps: Send + Sync {
    async fn ipv4_addrs(&self) -> Result<Vec<Ipv4Addr>>;
    async fn add_default_route_to_table(
        &self,
        tun_name: String,
        tun_ip: Ipv4Addr,
        table: u32,
    ) -> Result<()>;
    async fn existing_rule_prefs(&self) -> Result<HashSet<u32>>;
    async fn add_rule_fwmark_table(&self, pref: u32, table: u32, mark: u32) -> Result<()>;
    async fn add_rule_to_ip(&self, pref: u32, ip: Ipv4Addr, table: u32) -> Result<()>;
    async fn delete_rule_pref(&self, pref: u32) -> Result<()>;
    async fn delete_routes_in_table(&self, table: u32) -> Result<()>;
}

impl NetlinkOps for Arc<Netlink> {
    async fn ipv4_addrs(&self) -> Result<Vec<Ipv4Addr>> {
        self.as_ref().ipv4_addrs().await
    }

    async fn add_default_route_to_table(
        &self,
        tun_name: String,
        tun_ip: Ipv4Addr,
        table: u32,
    ) -> Result<()> {
        self.as_ref()
            .add_default_route_to_table(&tun_name, tun_ip, table)
            .await
    }

    async fn existing_rule_prefs(&self) -> Result<HashSet<u32>> {
        self.as_ref().existing_rule_prefs().await
    }

    async fn add_rule_fwmark_table(&self, pref: u32, table: u32, mark: u32) -> Result<()> {
        self.as_ref().add_rule_fwmark_table(pref, table, mark).await
    }

    async fn add_rule_to_ip(&self, pref: u32, ip: Ipv4Addr, table: u32) -> Result<()> {
        self.as_ref().add_rule_to_ip(pref, ip, table).await
    }

    async fn delete_rule_pref(&self, pref: u32) -> Result<()> {
        self.as_ref().delete_rule_pref(pref).await
    }

    async fn delete_routes_in_table(&self, table: u32) -> Result<()> {
        self.as_ref().delete_routes_in_table(table).await
    }
}

pub trait FirewallOps: Send + Sync {
    fn apply(&self, cfg: &FirewallConfig) -> Result<FirewallState>;
    fn remove_from_state(&self, state: &FirewallState) -> Result<()>;
}

pub struct RealFirewall {
    backend: FirewallBackendKind,
    runner: CommandRunner,
}

impl RealFirewall {
    pub fn new(backend: FirewallBackendKind, runner: CommandRunner) -> Self {
        Self { backend, runner }
    }
}

impl FirewallOps for RealFirewall {
    fn apply(&self, cfg: &FirewallConfig) -> Result<FirewallState> {
        self.backend.apply(cfg, &self.runner)
    }

    fn remove_from_state(&self, state: &FirewallState) -> Result<()> {
        self.backend.remove(state, &self.runner)
    }
}

pub trait MarkOps: Send + Sync {
    fn apply(&self, mark: u32, exclude_ips: Vec<std::net::IpAddr>) -> Result<()>;
    fn remove_best_effort(&self) -> Result<()>;
}

pub struct RealMark {
    backend: MarkBackendKind,
    runner: CommandRunner,
}

impl RealMark {
    pub fn new(backend: MarkBackendKind, runner: CommandRunner) -> Self {
        Self { backend, runner }
    }
}

impl MarkOps for RealMark {
    fn apply(&self, mark: u32, exclude_ips: Vec<std::net::IpAddr>) -> Result<()> {
        self.backend
            .apply(&MarkConfig { mark, exclude_ips }, &self.runner)
    }

    fn remove_best_effort(&self) -> Result<()> {
        remove_mark_rules_best_effort(&self.runner)
    }
}

pub trait StateStoreOps: Send + Sync {
    fn state_dir(&self) -> PathBuf;
    fn lock_path(&self) -> PathBuf;
    fn write_state(&self, state: &State) -> Result<()>;
    fn remove_state_files(&self, keep_logs: bool) -> Result<()>;
}

impl StateStoreOps for StateStore {
    fn state_dir(&self) -> PathBuf {
        self.state_dir.clone()
    }

    fn lock_path(&self) -> PathBuf {
        self.lock_path.clone()
    }

    fn write_state(&self, state: &State) -> Result<()> {
        StateStore::write_state(self, state)
    }

    fn remove_state_files(&self, keep_logs: bool) -> Result<()> {
        StateStore::remove_state_files(self, keep_logs)
    }
}
