mod iptables;
mod nft;

use std::net::IpAddr;

use anyhow::Result;

use proxyvpn_state::FirewallState;
use proxyvpn_util::CommandRunner;

pub struct FirewallConfig<'a> {
    pub tun_name: &'a str,
    pub proxy_ips: &'a [IpAddr],
    pub proxy_port: u16,
    pub dns_ips: &'a [IpAddr],
    pub allow_udp_dns: bool,
    pub proxy_mark: u32,
}

pub trait FirewallBackend {
    fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState>;
    fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()>;
}

pub struct NftBackend {
    pub table: String,
    pub chain: String,
}

impl FirewallBackend for NftBackend {
    fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState> {
        let table = &self.table;
        let chain = &self.chain;
        if let Err(err) = nft::apply_native(cfg, table, chain) {
            if proxyvpn_util::is_root() && nft::find_nft_binary() {
                nft::apply_cmd(cfg, table, chain, runner)?;
            } else {
                return Err(err);
            }
        }
        Ok(FirewallState::Nft {
            table: table.clone(),
            chain: chain.clone(),
        })
    }

    fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()> {
        if let FirewallState::Nft { table, .. } = state {
            match nft::delete_table(table) {
                Ok(()) => eprintln!("firewall: native nft delete '{}' succeeded", table),
                Err(err) => {
                    eprintln!("firewall: native nft delete '{}' failed: {}", table, err);
                    if proxyvpn_util::is_root() && nft::find_nft_binary() {
                        let _ = runner.run_capture_allow_fail("nft", &["delete", "table", "inet", table]);
                    } else {
                        return Err(err);
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct IptablesBackend {
    pub chain: String,
}

impl FirewallBackend for IptablesBackend {
    fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState> {
        iptables::apply(cfg, &self.chain, runner)?;
        Ok(FirewallState::Iptables {
            chain: self.chain.clone(),
        })
    }

    fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()> {
        if let FirewallState::Iptables { chain } = state {
            iptables::remove(chain, runner)?;
        }
        Ok(())
    }
}

pub enum FirewallBackendKind {
    Nft(NftBackend),
    Iptables(IptablesBackend),
}

impl FirewallBackendKind {
    pub fn apply(&self, cfg: &FirewallConfig, runner: &CommandRunner) -> Result<FirewallState> {
        match self {
            FirewallBackendKind::Nft(backend) => backend.apply(cfg, runner),
            FirewallBackendKind::Iptables(backend) => backend.apply(cfg, runner),
        }
    }

    pub fn remove(&self, state: &FirewallState, runner: &CommandRunner) -> Result<()> {
        match self {
            FirewallBackendKind::Nft(backend) => backend.remove(state, runner),
            FirewallBackendKind::Iptables(backend) => backend.remove(state, runner),
        }
    }

    pub fn remove_best_effort(&self, runner: &CommandRunner) -> Result<()> {
        // Always try to clean up nft table (even if using iptables backend)
        // This handles cases where a previous run used nft but current backend is iptables
        match nft::delete_table("proxyvpn") {
            Ok(()) => eprintln!("firewall: native nft delete succeeded"),
            Err(e) => eprintln!("firewall: native nft delete failed: {e}"),
        }

        // Also try via nft command (in case native failed - needs sudo or setcap on nft binary)
        if proxyvpn_util::is_root() && nft::find_nft_binary() {
            let _ = runner.run_capture_allow_fail("nft", &["delete", "table", "inet", "proxyvpn"]);
        }

        // Also try iptables cleanup
        if proxyvpn_util::is_root() && proxyvpn_util::find_in_path("iptables").is_some() {
            let _ = iptables::remove("PROXYVPN", runner);
        }

        Ok(())
    }
}
